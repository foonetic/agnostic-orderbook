use std::io::Write;
use std::ops::{Deref, DerefMut};
use std::str::FromStr;

use anchor_lang::prelude::*;
use anchor_lang::solana_program::log::sol_log_compute_units;
use borsh::BorshDeserialize;
use borsh::BorshSerialize;
use bytemuck::{try_from_bytes, try_from_bytes_mut};
use num_traits::FromPrimitive;

use aob::critbit::Slab;
use aob::error::AoError;
use aob::error::AoError::FailedToDeserialize;
use aob::orderbook::OrderBookState;
use aob::orderbook::OrderSummary;
use aob::params::NewOrderParams;
use aob::state::{AccountTag, EventQueueHeader, MARKET_STATE_LEN};
use aob::state::{EVENT_QUEUE_HEADER_LEN, EventQueue};
use aob::state::{SelfTradeBehavior, Side};
use aob::state::get_side_from_order_id;
use aob::utils::fp32_mul;
use aob::utils::round_price;

declare_id!("aaobKniTtDGvCZces7GH5UReLYP671bBkB96ahr9x3e");

#[program]
pub mod anchor_agnostic_orderbook {
    use super::*;

    pub fn create_market(
        ctx: Context<CreateMarket>,
        caller_authority: Pubkey,
        callback_info_len: u64,
        callback_id_len: u64,
        min_base_order_size: u64,
        tick_size: u64,
        cranker_reward: u64,
    ) -> ProgramResult {
        let market_state = &mut ctx.accounts.market;
        market_state.0 = aob::state::MarketState {
            tag: AccountTag::Market as u64,
            caller_authority: caller_authority.to_bytes(),
            event_queue: ctx.accounts.event_queue.key.to_bytes(),
            bids: ctx.accounts.bids.key.to_bytes(),
            asks: ctx.accounts.asks.key.to_bytes(),
            callback_info_len,
            callback_id_len,
            fee_budget: 0,
            initial_lamports: market_state.to_account_info().lamports(),
            min_base_order_size,
            tick_size,
            cranker_reward,
        };

        let event_queue_header = EventQueueHeader::initialize(callback_info_len as usize);
        event_queue_header
            .serialize(&mut (&mut ctx.accounts.event_queue.data.borrow_mut() as &mut [u8]))
            .unwrap();

        Slab::initialize(
            &ctx.accounts.bids.to_account_info(),
            &ctx.accounts.asks.to_account_info(),
            *ctx.accounts.market.to_account_info().key,
            callback_info_len as usize,
        );

        Ok(())
    }

    #[allow(clippy::too_many_arguments)]
    pub fn new_order(
        ctx: Context<NewOrder>,
        max_base_qty: u64,
        max_quote_qty: u64,
        limit_price: u64,
        side: u8,
        match_limit: u64,
        callback_info: Vec<u8>,
        post_only: bool,
        post_allowed: bool,
        self_trade_behavior: u8,
    ) -> ProgramResult {
        let market_state = &mut ctx.accounts.market;
        let side = Side::from_u8(side).ok_or(AoError::FailedToDeserialize)?;
        let self_trade_behavior =
            SelfTradeBehavior::from_u8(self_trade_behavior).ok_or(AoError::FailedToDeserialize)?;
        let limit_price = round_price(market_state.tick_size, limit_price, side);
        let callback_info_len = market_state.callback_info_len as usize;

        msg!("New Order: Creating order book");
        sol_log_compute_units();
        let mut order_book = OrderBookState::new(
            &ctx.accounts.bids,
            &ctx.accounts.asks,
            market_state.callback_info_len as usize,
            market_state.callback_id_len as usize,
        )?;
        sol_log_compute_units();

        if callback_info.len() != market_state.callback_info_len as usize {
            msg!("Invalid callback information");
            return Err(ProgramError::InvalidArgument);
        }

        msg!("New Order: Creating event queue");
        sol_log_compute_units();
        let header = {
            let mut event_queue_data: &[u8] =
                &ctx.accounts.event_queue.data.borrow()[0..EVENT_QUEUE_HEADER_LEN];
            EventQueueHeader::deserialize(&mut event_queue_data)
                .unwrap()
                .check()?
        };
        let mut event_queue =
            EventQueue::new_safe(header, &ctx.accounts.event_queue, callback_info_len)?;
        sol_log_compute_units();

        msg!("New Order: Creating new order");
        sol_log_compute_units();
        let order_summary = order_book.new_order(
            NewOrderParams {
                max_base_qty,
                max_quote_qty,
                limit_price,
                side,
                match_limit,
                callback_info,
                post_only,
                post_allowed,
                self_trade_behavior,
            },
            &mut event_queue,
            market_state.min_base_order_size,
        )?;
        sol_log_compute_units();
        msg!("Order summary : {:?}", order_summary);
        event_queue.write_to_register(order_summary);

        let mut event_queue_header_data: &mut [u8] =
            &mut ctx.accounts.event_queue.data.borrow_mut();
        event_queue
            .header
            .serialize(&mut event_queue_header_data)
            .unwrap();
        msg!("Committing changes");
        sol_log_compute_units();
        order_book.commit_changes();
        sol_log_compute_units();

        // Verify that fees were transfered. Fees are expected to be transfered by the caller
        // program in order to reduce the CPI call stack depth.
        if market_state.to_account_info().lamports() - market_state.initial_lamports
            < market_state
                .fee_budget
                .checked_add(market_state.cranker_reward)
                .unwrap()
        {
            msg!("Fees were not correctly payed during caller runtime.");
            return Err(AoError::FeeNotPayed.into());
        }
        market_state.fee_budget =
            market_state.to_account_info().lamports() - market_state.initial_lamports;
        order_book.release(&ctx.accounts.bids, &ctx.accounts.asks);

        Ok(())
    }

    pub fn cancel_order(ctx: Context<CancelOrder>, order_id: u128) -> ProgramResult {
        let market_state = &mut ctx.accounts.market;
        let callback_info_len = market_state.callback_info_len as usize;

        let mut order_book = OrderBookState::new(
            &ctx.accounts.bids,
            &ctx.accounts.asks,
            market_state.callback_info_len as usize,
            market_state.callback_id_len as usize,
        )?;

        let header = {
            let mut event_queue_data: &[u8] =
                &ctx.accounts.event_queue.data.borrow()[0..EVENT_QUEUE_HEADER_LEN];
            EventQueueHeader::deserialize(&mut event_queue_data).unwrap()
        };
        let event_queue =
            EventQueue::new_safe(header, &ctx.accounts.event_queue, callback_info_len)?;

        let slab = order_book.get_tree(get_side_from_order_id(order_id));
        let node = slab.remove_by_key(order_id).ok_or(AoError::OrderNotFound)?;
        let leaf_node = node.as_leaf().unwrap();
        let total_base_qty = leaf_node.base_quantity;
        let total_quote_qty = fp32_mul(leaf_node.base_quantity, leaf_node.price());

        let order_summary = OrderSummary {
            posted_order_id: None,
            total_base_qty,
            total_quote_qty,
            total_base_qty_posted: 0,
        };

        event_queue.write_to_register(order_summary);

        order_book.commit_changes();
        order_book.release(&ctx.accounts.bids, &ctx.accounts.asks);

        Ok(())
    }

    pub fn consume_events(
        ctx: Context<ConsumeEvents>,
        number_of_entries_to_consume: u64,
    ) -> ProgramResult {
        let market_state = &mut ctx.accounts.market;

        let header = {
            let mut event_queue_data: &[u8] =
                &ctx.accounts.event_queue.data.borrow()[0..EVENT_QUEUE_HEADER_LEN];
            EventQueueHeader::deserialize(&mut event_queue_data).unwrap()
        };
        let mut event_queue = EventQueue::new_safe(
            header,
            &ctx.accounts.event_queue,
            market_state.callback_info_len as usize,
        )?;

        // Reward payout
        let capped_number_of_entries_consumed =
            std::cmp::min(event_queue.header.count, number_of_entries_to_consume);
        let reward = (market_state.fee_budget * capped_number_of_entries_consumed)
            .checked_div(event_queue.header.count)
            .ok_or(AoError::NoOperations)
            .unwrap();
        market_state.fee_budget -= reward;
        let market_account = ctx.accounts.market.to_account_info();
        **market_account.try_borrow_mut_lamports()? -= reward;
        let reward_target_account = ctx.accounts.reward_target.to_account_info();
        **reward_target_account.try_borrow_mut_lamports()? += reward;

        // Pop Events
        event_queue.pop_n(number_of_entries_to_consume);
        let mut event_queue_data: &mut [u8] = &mut ctx.accounts.event_queue.data.borrow_mut();
        event_queue.header.serialize(&mut event_queue_data).unwrap();

        msg!(
            "Number of events consumed: {:?}",
            capped_number_of_entries_consumed
        );

        Ok(())
    }

    pub fn close_market(ctx: Context<CloseMarket>) -> ProgramResult {
        let market_state = &mut ctx.accounts.market;

        // Check if there are still orders in the book
        let orderbook_state = OrderBookState::new(
            &ctx.accounts.bids,
            &ctx.accounts.asks,
            market_state.callback_info_len as usize,
            market_state.callback_id_len as usize,
        )
        .unwrap();
        if !orderbook_state.is_empty() {
            msg!("The orderbook must be empty");
            return Err(ProgramError::from(AoError::MarketStillActive));
        }

        // Check if all events have been processed
        let header = {
            let mut event_queue_data: &[u8] =
                &ctx.accounts.event_queue.data.borrow()[0..EVENT_QUEUE_HEADER_LEN];
            EventQueueHeader::deserialize(&mut event_queue_data).unwrap()
        };
        if header.count != 0 {
            msg!("The event queue needs to be empty");
            return Err(ProgramError::from(AoError::MarketStillActive));
        }

        market_state.tag = AccountTag::Uninitialized as u64;
        let market = ctx.accounts.market.to_account_info();
        let event_queue = ctx.accounts.event_queue.to_account_info();
        let bids = ctx.accounts.bids.to_account_info();
        let asks = ctx.accounts.asks.to_account_info();
        let lamports_target_account = ctx.accounts.lamports_target_account.to_account_info();

        let mut market_lamports = market.try_borrow_mut_lamports()?;
        let mut event_queue_lamports = event_queue.try_borrow_mut_lamports()?;
        let mut bids_lamports = bids.try_borrow_mut_lamports()?;
        let mut asks_lamports = asks.try_borrow_mut_lamports()?;
        let mut target_lamports = lamports_target_account.try_borrow_mut_lamports()?;

        **target_lamports +=
            **market_lamports + **bids_lamports + **asks_lamports + **event_queue_lamports;

        **market_lamports = 0;
        **bids_lamports = 0;
        **asks_lamports = 0;
        **event_queue_lamports = 0;

        orderbook_state.release(&ctx.accounts.bids, &ctx.accounts.asks);

        Ok(())
    }
}

/// TODO zero-copy. this currently delegates to Borsh
///
/// This is to solve the problem of:
/// How can I implement Anchor traits on a type `T` without modifying `T` itself? (i.e. attaching
/// `derive` macros)
///
/// The "wrapper type" pattern is done in Anchor's own codebase.
///
/// See how Anchor does this pattern in their `spl` wrappers here:
/// https://github.com/project-serum/anchor/blob/master/spl/src/token.rs#L306
///
/// See this PR comment for more details and thinking about the trade-offs (vs. modifying orderbook
/// data structures directly):
/// https://github.com/foonetic/agnostic-orderbook/pull/10#issuecomment-1038329572
///
/// Important! This isn't generally possible, because of Rust's orphan rule
#[derive(Clone, Default)]
pub struct MarketState(aob::state::MarketState);

impl MarketState {
    pub const LEN: usize = MARKET_STATE_LEN;
}

impl Deref for MarketState {
    type Target = aob::state::MarketState;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl DerefMut for MarketState {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

// AccountDeserialize delegates to AnchorDeserialize (which delegates to Borsh)
impl AccountDeserialize for MarketState {
    fn try_deserialize_unchecked(buf: &mut &[u8]) -> Result<Self, ProgramError> {
        AnchorDeserialize::deserialize(buf).map_err(|e| ProgramError::InvalidAccountData)
    }
}

impl AccountSerialize for MarketState {
    fn try_serialize<W: Write>(&self, _writer: &mut W) -> Result<(), ProgramError> {
        self.serialize(_writer).map_err(|e| ProgramError::BorshIoError(e.to_string()))
    }
}

impl AnchorSerialize for MarketState {
    fn serialize<W: Write>(&self, writer: &mut W) -> std::io::Result<()> {
        self.0.serialize(writer)
    }
}

impl AnchorDeserialize for MarketState {
    fn deserialize(buf: &mut &[u8]) -> std::io::Result<Self> {
        aob::state::MarketState::deserialize(buf).map(|market_state| MarketState(market_state))
    }
}

impl Owner for MarketState {
    fn owner() -> Pubkey {
        crate::id()
    }
}

#[derive(Accounts)]
pub struct CreateMarket<'info> {
    // TODO PDAs?
    #[account(init, payer = payer)]
    pub market: Account<'info, MarketState>,
    #[account(init, payer = payer, space = 10240)]
    pub event_queue: AccountInfo<'info>,
    // TODO it would be nicer to parameterize with the actual types `T`
    // and let Anchor do the serde boilerplate, checks etc. for us
    // but `Slab` does not implement `Clone` (see comment on `Slab` struct)
    #[account(init, payer = payer, space = 10240)]
    pub bids: AccountInfo<'info>,
    #[account(init, payer = payer, space = 10240)]
    pub asks: AccountInfo<'info>,
    #[account(mut)]
    pub payer: Signer<'info>,
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct NewOrder<'info> {
    #[account(mut)]
    pub market: Account<'info, MarketState>,
    #[account(mut)]
    pub event_queue: AccountInfo<'info>,
    #[account(mut)]
    pub bids: AccountInfo<'info>,
    #[account(mut)]
    pub asks: AccountInfo<'info>,
    #[account(mut)]
    pub authority: Signer<'info>,
}

#[derive(Accounts)]
pub struct CancelOrder<'info> {
    #[account(mut)]
    pub market: Account<'info, MarketState>,
    #[account(mut)]
    pub event_queue: AccountInfo<'info>,
    #[account(mut)]
    pub bids: AccountInfo<'info>,
    #[account(mut)]
    pub asks: AccountInfo<'info>,
    #[account(mut)]
    pub authority: Signer<'info>,
}

#[derive(Accounts)]
pub struct ConsumeEvents<'info> {
    pub market: Account<'info, MarketState>,
    #[account(mut)]
    pub event_queue: AccountInfo<'info>,
    #[account(mut)]
    pub authority: Signer<'info>,
    pub reward_target: AccountInfo<'info>,
}

#[derive(Accounts)]
pub struct CloseMarket<'info> {
    #[account(mut)]
    pub market: Account<'info, MarketState>,
    #[account(mut)]
    pub event_queue: AccountInfo<'info>,
    #[account(mut)]
    pub bids: AccountInfo<'info>,
    #[account(mut)]
    pub asks: AccountInfo<'info>,
    #[account(mut)]
    pub authority: Signer<'info>,
    #[account(mut)]
    pub lamports_target_account: Signer<'info>,
}

// #[account(zero_copy)]
// #[derive(Debug, Default)]
// #[repr(transparent)]
// pub struct MarketState {
//     market_state: aob::state::MarketState,
// }
//
// impl Deref for MarketState {
//     type Target = aob::state::MarketState;
//
//     fn deref(&self) -> &Self::Target {
//         &self.0
//     }
// }
//
// impl DerefMut for MarketState {
//     fn deref_mut(&mut self) -> &mut Self::Target {
//         &mut self.0
//     }
// }
//
// impl AccountDeserialize for &mut MarketState {
//     fn try_deserialize_unchecked<'a, 'b>(
//         buf: &'a mut &'b [u8],
//     ) -> Result<&'b mut Self, PodCastError> {
//         try_from_bytes_mut::<MarketState>(&mut buf[0..aob::state::MarketState::LEN])
//     }
// }
//
// impl Owner for MarketState {
//     fn owner() -> Pubkey {
//         ID
//     }
// }
//
// impl Discriminator for MarketState {
//     fn discriminator() -> [u8; 8] {
//         [1, 2, 3, 4, 5, 6, 7, 8]  // TODO right way to do this
//     }
// }
//
// impl ZeroCopy for MarketState {}
