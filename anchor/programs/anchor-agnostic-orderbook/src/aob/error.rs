use anchor_lang::prelude::*;

#[error_code]
pub enum ErrorCode {
    #[msg("This account is already initialized")]
    AlreadyInitialized,
    #[msg("An invalid bids account has been provided.")]
    WrongBidsAccount,
    #[msg("An invalid asks account has been provided.")]
    WrongAsksAccount,
    #[msg("An invalid event queue account has been provided.")]
    WrongEventQueueAccount,
    #[msg("An invalid caller authority account has been provided.")]
    WrongCallerAuthority,
    #[msg("The event queue is full.")]
    EventQueueFull,
    #[msg("The order could not be found.")]
    OrderNotFound,
    #[msg("The order would self trade.")]
    WouldSelfTrade,
    #[msg("The market's memory is full.")]
    SlabOutOfSpace,
    #[msg("The due fee was not payed.")]
    FeeNotPayed,
    #[msg("This instruction is a No-op.")]
    NoOperations,
    #[msg("The market is still active")]
    MarketStillActive,
    #[msg("The base quantity must be > 0")]
    InvalidBaseQuantity,
    #[msg("The event queue should be owned by the AO program")]
    WrongEventQueueOwner,
    #[msg("The bids account should be owned by the AO program")]
    WrongBidsOwner,
    #[msg("The asks account should be owned by the AO program")]
    WrongAsksOwner,
    #[msg("The market account should be owned by the AO program")]
    WrongMarketOwner,
    #[msg("The MSRM token account should be owned by the cranker")]
    WrongMsrmOwner,
    #[msg("An invalid MSRM mint has been provided")]
    WrongMsrmMint,
    #[msg("The MSRM token account does not have enough balances")]
    WrongMsrmBalance,
    #[msg("Illegal MSRM token account owner")]
    IllegalMsrmOwner,
    #[msg("Wrong account tag")]
    WrongAccountTag,
    #[msg("Failed to deserialize")]
    FailedToDeserialize,
}
