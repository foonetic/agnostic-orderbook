use std::{cell::RefMut, convert::TryInto, io::Write, mem::size_of};

use anchor_lang::prelude::*;
use anchor_lang::solana_program::{
    account_info::AccountInfo, entrypoint::ProgramResult, msg, program_error::ProgramError,
};
use bonfida_utils::BorshSize;
use borsh::{BorshDeserialize, BorshSerialize};
use bytemuck::{try_from_bytes_mut, Pod};
use num_derive::{FromPrimitive, ToPrimitive};
use num_traits::{FromPrimitive, ToPrimitive};

use crate::aob::critbit::IoError;
pub use crate::aob::orderbook::{OrderSummary, ORDER_SUMMARY_SIZE};
#[cfg(feature = "no-entrypoint")]
pub use crate::aob::utils::get_spread;

#[derive(AnchorSerialize, AnchorDeserialize, Copy, Clone, Debug, PartialEq)]
// #[account]
#[allow(missing_docs)]
#[repr(u8)]
pub enum AccountTag {
    Uninitialized,
    Market,
    EventQueue,
    Bids,
    Asks,
}

/// TODO this is done for the sake of Anchor-space sizing, but it's probably conflating.
///
/// Anchor uses `T::default()` to derive `space`, but really, there should be a separate `Space`
/// trait.
impl Default for AccountTag {
    fn default() -> Self {
        AccountTag::Uninitialized
    }
}

#[derive(
    BorshDeserialize,
    BorshSerialize,
    Clone,
    Copy,
    PartialEq,
    FromPrimitive,
    ToPrimitive,
    Debug,
    BorshSize,
)]
#[repr(u8)]
#[allow(missing_docs)]
pub enum Side {
    Bid,
    Ask,
}

impl Side {
    /// Helper function to get the opposite side.
    pub fn opposite(&self) -> Self {
        match self {
            Side::Bid => Side::Ask,
            Side::Ask => Side::Bid,
        }
    }
}

/// Describes what happens when two order with identical callback informations are matched together
#[derive(
    BorshDeserialize, BorshSerialize, Clone, PartialEq, FromPrimitive, ToPrimitive, BorshSize,
)]
#[repr(u8)]
pub enum SelfTradeBehavior {
    /// The orders are matched together
    DecrementTake,
    /// The order on the provide side is cancelled. Matching for the current order continues and essentially bypasses
    /// the self-provided order.
    CancelProvide,
    /// The entire transaction fails and the program returns an error.
    AbortTransaction,
}

/// The orderbook market's central state
/// TODO zero-copy for Anchor
#[account(zero_copy)]
#[derive(Debug, Default)]
#[repr(C, packed)]
pub struct MarketState {
    /// Identifies the account as a [`MarketState`] object.
    pub tag: u64,
    /// The required signer for all market operations.
    pub caller_authority: [u8; 32],
    /// The public key of the orderbook's event queue account
    pub event_queue: [u8; 32],
    /// The public key of the orderbook's bids account
    pub bids: [u8; 32],
    /// The public key of the orderbook's asks account
    pub asks: [u8; 32],
    /// The length of an order actor's callback identifier.
    pub callback_id_len: u64,
    /// The length of an order's callback metadata.
    pub callback_info_len: u64,
    /// The current budget of fees that have been collected.
    /// Cranker rewards are taken from this. This value allows
    /// for a verification that the fee was payed in the caller program
    /// runtime while not having to add a CPI call to the serum-core.
    pub fee_budget: u64,
    /// The amount of lamports the market account was created with.
    pub initial_lamports: u64,
    /// The minimum order size that can be inserted into the orderbook after matching.
    pub min_base_order_size: u64,
    /// Tick size (FP32)
    pub tick_size: u64,
    /// Cranker reward (in lamports)
    pub cranker_reward: u64,
}

/// Expected size in bytes of MarketState
pub const MARKET_STATE_LEN: usize = size_of::<MarketState>();

impl MarketState {
    #[allow(missing_docs)]
    pub fn get<'a, 'b: 'a>(
        account_info: &'a AccountInfo<'b>,
    ) -> Result<RefMut<'a, Self>, ProgramError> {
        let a = Self::get_unchecked(account_info);
        if a.tag != AccountTag::Market as u64 {
            return Err(ProgramError::InvalidAccountData);
        };
        Ok(a)
    }

    #[allow(missing_docs)]
    pub fn get_unchecked<'a, 'b: 'a>(account_info: &'a AccountInfo<'b>) -> RefMut<'a, Self> {
        let a = RefMut::map(account_info.data.borrow_mut(), |s| {
            try_from_bytes_mut::<Self>(&mut s[0..MARKET_STATE_LEN]).unwrap()
        });
        a
    }
}

/// Events are the primary output of the asset agnostic orderbook
#[derive(Copy, Clone, Debug)]
// #[derive(AnchorSerialize, AnchorDeserialize, Copy, Clone, Debug)]
pub enum Event {
    /// Would rather use Option but Anchor IDL can't seem to properly parse Option<Event>
    /// and makes assumptions about `Default`s
    None,
    /// A fill event describes a match between a taker order and a maker order
    Fill {
        #[allow(missing_docs)]
        taker_side: Side,
        /// The order id of the maker order
        maker_order_id: u128,
        /// The total quote size of the transaction
        quote_size: u64,
        /// The total base size of the transaction
        base_size: u64,
        /// The callback information for the maker
        maker_callback_info: [u8; 32],
        /// The callback information for the taker
        taker_callback_info: [u8; 32],
    },
    /// An out event describes an order which has been taken out of the orderbook
    Out {
        #[allow(missing_docs)]
        side: Side,
        #[allow(missing_docs)]
        order_id: u128,
        #[allow(missing_docs)]
        base_size: u64,
        #[allow(missing_docs)]
        delete: bool,
        #[allow(missing_docs)]
        callback_info: [u8; 32],
    },
}

impl Default for Event {
    fn default() -> Self {
        Event::None
    }
}

/// Event queue
#[account(zero_copy)]
#[derive(Debug, Default)]
pub struct EventQueue {
    pub head: u64,
    pub count: u64,
    pub seq_num: u64,
    pub callback_info_len: u64,
    pub buffer: [Event; 8],
}

impl EventQueue {
    pub fn new(callback_info_len: u64) -> Self {
        Self {
            head: 0,
            count: 0,
            seq_num: 0,
            callback_info_len,
            buffer: [Event::None; 8],
        }
    }

    pub fn empty(&self) -> bool {
        self.count == 0
    }

    pub fn full(&self) -> bool {
        self.count as usize == self.buffer.len()
    }

    /// Appends an `Event` to the back of the collection
    ///
    /// Returns back the `Event` if the vector is full
    pub fn push_back(&mut self, event: Event) -> Result<(), Event> {
        if self.full() {
            return Err(event);
        }
        let slot = ((self.head + self.count) as usize) % self.buffer.len();
        self.buffer[slot as usize] = event;
        self.head += 1;
        self.count += 1;
        self.seq_num += 1;
        // msg!("PUSH BACK {:?}", event);
        Ok(())
    }

    /// Removes the `Event` from the front of the deque and returns it, or `None` if it's empty
    pub fn pop_front(&mut self) -> Option<Event> {
        if self.empty() {
            return None;
        }
        let value = self.buffer[self.head as usize];
        self.count -= 1;
        self.head = (self.head + 1) % self.buffer.len() as u64;
        Some(value)
    }

    pub(crate) fn gen_order_id(&mut self, limit_price: u64, side: Side) -> u128 {
        let seq_num = self.seq_num + 1;
        let upper = (limit_price as u128) << 64;
        let lower = match side {
            Side::Bid => !seq_num,
            Side::Ask => seq_num,
        };
        upper | (lower as u128)
    }
}

// impl<T> EventQueue<T> {
//     pub(crate) fn gen_order_id(&mut self, limit_price: u64, side: Side) -> u128 {
//         let seq_num = self.gen_seq_num();
//         let upper = (limit_price as u128) << 64;
//         let lower = match side {
//             Side::Bid => !seq_num,
//             Side::Ask => seq_num,
//         };
//         upper | (lower as u128)
//     }
//
//     fn gen_seq_num(&mut self) -> u64 {
//         let seq_num = self.header.seq_num;
//         self.header.seq_num += 1;
//         seq_num
//     }
//
//     pub(crate) fn get_buf_len(&self) -> usize {
//         self.buffer.len() - EventQueueHeader::LEN - REGISTER_SIZE
//     }
//
//     pub(crate) fn full(&self) -> bool {
//         self.header.count as usize == (self.get_buf_len() / (self.header.event_size as usize))
//     }
//
//     pub(crate) fn push_back(&mut self, event: Event) -> Result<(), Event> {
//         if self.full() {
//             return Err(event);
//         }
//         let offset = EventQueueHeader::LEN
//             + (REGISTER_SIZE)
//             + (((self.header.head + self.header.count * self.header.event_size) as usize)
//                 % self.get_buf_len());
//         let mut queue_event_data =
//             &mut self.buffer[offset..offset + (self.header.event_size as usize)];
//         event.serialize(&mut queue_event_data).unwrap();
//
//         self.header.count += 1;
//         self.header.seq_num += 1;
//
//         Ok(())
//     }
//
//     /// Retrieves the event at position index in the queue.
//     pub fn peek_at(&self, index: u64) -> Option<Event> {
//         if self.header.count <= index {
//             return None;
//         }
//
//         let header_offset = EventQueueHeader::LEN + REGISTER_SIZE;
//         let offset = ((self
//             .header
//             .head
//             .checked_add(index)
//             .unwrap()
//             .checked_mul(self.header.event_size)
//             .unwrap()) as usize
//             % self.get_buf_len())
//             + header_offset;
//         let mut event_data = &self.buffer[offset..offset + (self.header.event_size as usize)];
//         Some(Event::deserialize(&mut event_data, self.callback_info_len as usize))
//     }
//
//     /// Pop n entries from the event queue
//     pub fn pop_n(&mut self, number_of_entries_to_pop: u64) {
//         let capped_number_of_entries_to_pop =
//             std::cmp::min(self.header.count, number_of_entries_to_pop);
//         self.header.count -= capped_number_of_entries_to_pop;
//         self.header.head = (self.header.head
//             + capped_number_of_entries_to_pop * self.header.event_size)
//             % self.get_buf_len() as u64;
//     }
//
//     pub fn write_to_register<T: BorshSerialize + BorshDeserialize>(&mut self, object: T) {
//         let mut register =
//             &mut self.buffer[EventQueueHeader::LEN..EventQueueHeader::LEN + (REGISTER_SIZE)];
//         Register::Some(object).serialize(&mut register).unwrap();
//     }
//
//     pub fn clear_register(&mut self) {
//         let mut register =
//             &mut self.buffer[EventQueueHeader::LEN..EventQueueHeader::LEN + (REGISTER_SIZE)];
//         Register::<u8>::None.serialize(&mut register).unwrap();
//     }
//
//     /// This method is used to deserialize the event queue's register
//     ///
//     /// The nature of the serialized object should be deductible from caller context
//     pub fn read_register<T: BorshSerialize + BorshDeserialize>(
//         &self,
//     ) -> Result<Register<T>, IoError> {
//         let mut register =
//             &self.buffer[EventQueueHeader::LEN..EventQueueHeader::LEN + (REGISTER_SIZE)];
//         Register::deserialize(&mut register)
//     }
//
//     /// Returns an iterator over all the queue's events
//     #[cfg(feature = "no-entrypoint")]
//     pub fn iter<'b>(&'b self) -> QueueIterator<'a, 'b> {
//         QueueIterator {
//             queue_header: &self.header,
//             buffer: Rc::clone(&self.buffer),
//             current_index: self.header.head as usize,
//             callback_info_len: self.callback_info_len,
//             buffer_length: self.get_buf_len(),
//             header_offset: EventQueueHeader::LEN + REGISTER_SIZE,
//             remaining: self.header.count,
//         }
//     }
// }
//
// /// This method is used to deserialize the event queue's register
// /// without constructing an EventQueue instance
// ///
// /// The nature of the serialized object should be deductible from caller context
// pub fn read_register<T: BorshSerialize + BorshDeserialize>(
//     event_q_acc: &AccountInfo,
// ) -> Result<Register<T>, IoError> {
//     let mut register =
//         &event_q_acc.data.borrow()[EventQueueHeader::LEN..EventQueueHeader::LEN + REGISTER_SIZE];
//     Register::deserialize(&mut register)
// }
//
// #[cfg(feature = "no-entrypoint")]
// impl<'a, 'b> IntoIterator for &'b EventQueue<'a> {
//     type Item = Event;
//
//     type IntoIter = QueueIterator<'a, 'b>;
//
//     fn into_iter(self) -> Self::IntoIter {
//         self.iter()
//     }
// }
// #[cfg(feature = "no-entrypoint")]
// /// Utility struct for iterating over a queue
// pub struct QueueIterator<'a, 'b> {
//     queue_header: &'b EventQueueHeader,
//     buffer: Rc<RefCell<&'a mut [u8]>>, //The whole account data
//     current_index: usize,
//     callback_info_len: usize,
//     buffer_length: usize,
//     header_offset: usize,
//     remaining: u64,
// }
//
// #[cfg(feature = "no-entrypoint")]
// impl<'a, 'b> Iterator for QueueIterator<'a, 'b> {
//     type Item = Event;
//
//     fn next(&mut self) -> Option<Self::Item> {
//         if self.remaining == 0 {
//             return None;
//         }
//         let result = Event::deserialize(
//             &mut &self.buffer.borrow()[self.header_offset + self.current_index..],
//             self.callback_info_len,
//         );
//         self.current_index =
//             (self.current_index + self.queue_header.event_size as usize) % self.buffer_length;
//         self.remaining -= 1;
//         Some(result)
//     }
// }

/// This byte flag is set for order_ids with side Bid, and unset for side Ask
pub const ORDER_ID_SIDE_FLAG: u128 = 1 << 63;

/// This helper function deduces an order's side from its order_id
pub fn get_side_from_order_id(order_id: u128) -> Side {
    if ORDER_ID_SIDE_FLAG & order_id != 0 {
        Side::Bid
    } else {
        Side::Ask
    }
}
