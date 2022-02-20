use borsh::{BorshDeserialize, BorshSerialize};
use bonfida_utils::BorshSize;

use crate::aob::state::{SelfTradeBehavior, Side};

#[derive(BorshDeserialize, BorshSerialize, BorshSize)]
/**
The required arguments for a create_market instruction.
 */
pub struct CreateMarketParams {
    /// The caller authority will be the required signer for all market instructions.
    ///
    /// In practice, it will almost always be a program-derived address..
    pub caller_authority: [u8; 32],
    /// Callback information can be used by the caller to attach specific information to all orders.
    ///
    /// An example of this would be to store a public key to uniquely identify the owner of a particular order.
    /// This example would thus require a value of 32
    pub callback_info_len: u64,
    /// The prefix length of callback information which is used to identify self-trading
    pub callback_id_len: u64,
    /// The minimum order size that can be inserted into the orderbook after matching.
    pub min_base_order_size: u64,
    /// Enables the limiting of price precision on the orderbook (price ticks)
    pub tick_size: u64,
    /// Fixed fee for every new order operation. A higher fee increases incentives for cranking.
    pub cranker_reward: u64,
}

#[derive(BorshDeserialize, BorshSerialize, Clone, BorshSize)]
/**
The required arguments for a new_order instruction.
 */
pub struct NewOrderParams {
    /// The maximum quantity of base to be traded.
    pub max_base_qty: u64,
    /// The maximum quantity of quote to be traded.
    pub max_quote_qty: u64,
    /// The limit price of the order. This value is understood as a 32-bit fixed point number.
    pub limit_price: u64,
    /// The order's side.
    pub side: Side,
    /// The maximum number of orders to match against before performing a partial fill.
    ///
    /// It is then possible for a caller program to detect a partial fill by reading the [`OrderSummary`][`crate::orderbook::OrderSummary`]
    /// in the event queue register.
    pub match_limit: u64,
    /// The callback information is used to attach metadata to an order. This callback information will be transmitted back through the event queue.
    ///
    /// The size of this vector should not exceed the current market's [`callback_info_len`][`MarketState::callback_info_len`].
    pub callback_info: [u8; 32],
    /// The order will not be matched against the orderbook and will be direcly written into it.
    ///
    /// The operation will fail if the order's limit_price crosses the spread.
    pub post_only: bool,
    /// The order will be matched against the orderbook, but what remains will not be written as a new order into the orderbook.
    pub post_allowed: bool,
    /// Describes what would happen if this order was matched against an order with an equal `callback_info` field.
    pub self_trade_behavior: SelfTradeBehavior,
}

#[derive(BorshDeserialize, BorshSerialize, Clone, BorshSize)]
/**
The required arguments for a cancel_order instruction.
 */
pub struct CancelOrderParams {
    /// The order id is a unique identifier for a particular order
    pub order_id: u128,
}

#[derive(BorshDeserialize, BorshSerialize, Clone, BorshSize)]
/**
The required arguments for a consume_events instruction.
 */
pub struct ConsumeEventsParams {
    /// Depending on applications, it might be optimal to process several events at a time
    pub number_of_entries_to_consume: u64,
}

#[derive(BorshDeserialize, BorshSerialize, BorshSize)]
/**
The required arguments for a close_market instruction.
 */
pub struct CloseMarketParams {}
