use std::array::TryFromSliceError;
use num_derive::FromPrimitive;
use num_traits::FromPrimitive;
use anchor_lang::solana_program::program_error::PrintProgramError;
use anchor_lang::solana_program::{decode_error::DecodeError, msg, program_error::ProgramError};
use thiserror::Error;

pub type AoResult<T = ()> = Result<T, AoError>;

//TODO clean-up
#[derive(Clone, Debug, Error, FromPrimitive)]
pub enum AoError {
    #[error("This account is already initialized")]
    AlreadyInitialized,
    #[error("An invalid bids account has been provided.")]
    WrongBidsAccount,
    #[error("An invalid asks account has been provided.")]
    WrongAsksAccount,
    #[error("An invalid event queue account has been provided.")]
    WrongEventQueueAccount,
    #[error("An invalid caller authority account has been provided.")]
    WrongCallerAuthority,
    #[error("The event queue is full.")]
    EventQueueFull,
    #[error("The order could not be found.")]
    OrderNotFound,
    #[error("The order would self trade.")]
    WouldSelfTrade,
    #[error("The market's memory is full.")]
    SlabOutOfSpace,
    #[error("The due fee was not payed.")]
    FeeNotPayed,
    #[error("This instruction is a No-op.")]
    NoOperations,
    #[error("The market is still active")]
    MarketStillActive,
    #[error("The base quantity must be > 0")]
    InvalidBaseQuantity,
    #[error("The event queue should be owned by the AO program")]
    WrongEventQueueOwner,
    #[error("The bids account should be owned by the AO program")]
    WrongBidsOwner,
    #[error("The asks account should be owned by the AO program")]
    WrongAsksOwner,
    #[error("The market account should be owned by the AO program")]
    WrongMarketOwner,
    #[error("The MSRM token account should be owned by the cranker")]
    WrongMsrmOwner,
    #[error("An invalid MSRM mint has been provided")]
    WrongMsrmMint,
    #[error("The MSRM token account does not have enough balances")]
    WrongMsrmBalance,
    #[error("Illegal MSRM token account owner")]
    IllegalMsrmOwner,
    #[error("Wrong account tag")]
    WrongAccountTag,
    #[error("Failed to deserialize")]
    FailedToDeserialize,
}

impl From<TryFromSliceError> for AoError {
    fn from(e: TryFromSliceError) -> Self {
        msg!("{}", e);
        AoError::FailedToDeserialize
    }
}

impl From<AoError> for ProgramError {
    fn from(e: AoError) -> Self {
        e.print::<AoError>();
        ProgramError::Custom(e as u32)
    }
}

impl From<AoError> for anchor_lang::error::Error {
    fn from(e: AoError) -> Self {
        anchor_lang::error::Error::ProgramError(
            anchor_lang::error::ProgramErrorWithOrigin {
                program_error: ProgramError::from(e),
                source: None,
                account_name: None
            }
        )
    }
}

impl<T> DecodeError<T> for AoError {
    fn type_of() -> &'static str {
        "AOError"
    }
}

impl PrintProgramError for AoError {
    fn print<E>(&self)
    where
        E: 'static + std::error::Error + DecodeError<E> + PrintProgramError + FromPrimitive,
    {
        match self {
            AoError::AlreadyInitialized => msg!("Error: This account is already initialized"),
            AoError::WrongBidsAccount => msg!("Error: An invalid bids account has been provided."),
            AoError::WrongAsksAccount => msg!("Error: An invalid asks account has been provided."),
            AoError::WrongEventQueueAccount => {
                msg!("Error: An invalid event queue account has been provided.")
            }
            AoError::WrongCallerAuthority => {
                msg!("Error: An invalid caller authority account has been provided.")
            }
            AoError::EventQueueFull => msg!("Error: The event queue is full. "),
            AoError::OrderNotFound => msg!("Error: The order could not be found."),
            AoError::WouldSelfTrade => msg!("Error: The order would self trade."),
            AoError::SlabOutOfSpace => msg!("Error: The market's memory is full."),
            AoError::FeeNotPayed => msg!("Error: The fee was not correctly payed."),
            AoError::NoOperations => msg!("Error: This instruction is a No-op."),
            AoError::MarketStillActive => msg!("Error: The market is still active"),
            AoError::InvalidBaseQuantity => msg!("Error: The base quantity must be > 0"),
            AoError::WrongEventQueueOwner => {
                msg!("Error: The event queue should be owned by the AO program")
            }
            AoError::WrongBidsOwner => {
                msg!("Error: The bids account should be owned by the AO program")
            }
            AoError::WrongAsksOwner => {
                msg!("Error: The asks account should be owned by the AO program")
            }
            AoError::WrongMarketOwner => {
                msg!("Error: The market account should be owned by the AO program")
            }
            AoError::WrongMsrmOwner => {
                msg!("Error: The MSRM token account should be owned by the cranker")
            }
            AoError::WrongMsrmMint => {
                msg!("Error: An invalid MSRM mint has been provided")
            }
            AoError::WrongMsrmBalance => {
                msg!("Error: The MSRM token account does not have enough balances")
            }
            AoError::IllegalMsrmOwner => {
                msg!("Error: Illegal MSRM token account owner")
            }
            AoError::WrongAccountTag => {
                msg!("Error: Wrong account tag")
            }

            AoError::FailedToDeserialize => {
                msg!("Error: Failed to deserialize slab header")
            }
        }
    }
}
