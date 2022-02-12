//! Create and initialize a new orderbook market
use bonfida_utils::InstructionsAccount;
use borsh::BorshSerialize;

use solana_program::{
    account_info::{next_account_info, AccountInfo},
    entrypoint::ProgramResult,
    program_error::ProgramError,
    pubkey::Pubkey,
};

use aob::params::CreateMarketParams;
use aob::{
    critbit::Slab,
    error::AoError,
    state::{AccountTag, EventQueue, EventQueueHeader, MarketState},
    utils::{check_account_owner, check_unitialized},
};

/// The required accounts for a create_market instruction.
#[derive(InstructionsAccount)]
pub struct Accounts<'a, T> {
    #[allow(missing_docs)]
    #[cons(writable)]
    pub market: &'a T,
    #[allow(missing_docs)]
    #[cons(writable)]
    pub event_queue: &'a T,
    #[allow(missing_docs)]
    #[cons(writable)]
    pub bids: &'a T,
    #[allow(missing_docs)]
    #[cons(writable)]
    pub asks: &'a T,
}

impl<'a, 'b> Accounts<'a, AccountInfo<'b>> {
    pub(crate) fn parse(accounts: &'a [AccountInfo<'b>]) -> Result<Self, ProgramError> {
        let accounts_iter = &mut accounts.iter();

        let a = Self {
            market: next_account_info(accounts_iter)?,
            event_queue: next_account_info(accounts_iter)?,
            bids: next_account_info(accounts_iter)?,
            asks: next_account_info(accounts_iter)?,
        };

        Ok(a)
    }

    pub(crate) fn perform_checks(&self, program_id: &Pubkey) -> Result<(), ProgramError> {
        check_account_owner(
            self.event_queue,
            &program_id.to_bytes(),
            AoError::WrongEventQueueOwner,
        )?;
        check_account_owner(self.bids, &program_id.to_bytes(), AoError::WrongBidsOwner)?;
        check_account_owner(self.asks, &program_id.to_bytes(), AoError::WrongAsksOwner)?;
        Ok(())
    }
}

/// Apply the create_market instruction to the provided accounts
pub fn process(
    program_id: &Pubkey,
    accounts: Accounts<AccountInfo>,
    params: CreateMarketParams,
) -> ProgramResult {
    accounts.perform_checks(program_id)?;
    let CreateMarketParams {
        caller_authority,
        callback_info_len,
        callback_id_len,
        min_base_order_size,
        tick_size,
        cranker_reward,
    } = params;

    check_unitialized(accounts.event_queue)?;
    check_unitialized(accounts.bids)?;
    check_unitialized(accounts.asks)?;
    check_unitialized(accounts.market)?;
    EventQueue::check_buffer_size(accounts.event_queue, params.callback_info_len)?;

    let mut market_state = MarketState::get_unchecked(accounts.market);

    *market_state = MarketState {
        tag: AccountTag::Market as u64,
        caller_authority,
        event_queue: accounts.event_queue.key.to_bytes(),
        bids: accounts.bids.key.to_bytes(),
        asks: accounts.asks.key.to_bytes(),
        callback_info_len,
        callback_id_len,
        fee_budget: 0,
        initial_lamports: accounts.market.lamports(),
        min_base_order_size,
        tick_size,
        cranker_reward,
    };

    let event_queue_header = EventQueueHeader::initialize(params.callback_info_len as usize);
    event_queue_header
        .serialize(&mut (&mut accounts.event_queue.data.borrow_mut() as &mut [u8]))
        .unwrap();

    Slab::initialize(
        accounts.bids,
        accounts.asks,
        *accounts.market.key,
        callback_info_len as usize,
    );

    Ok(())
}
