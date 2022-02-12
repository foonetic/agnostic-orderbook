//! Execute a new order on the orderbook

use bonfida_utils::InstructionsAccount;
use borsh::{BorshDeserialize, BorshSerialize};
use solana_program::{
    account_info::{next_account_info, AccountInfo},
    entrypoint::ProgramResult,
    msg,
    program_error::ProgramError,
    pubkey::Pubkey,
};

use aob::params::NewOrderParams;
use aob::{
    error::AoError,
    orderbook::OrderBookState,
    state::{EventQueue, EventQueueHeader, MarketState, EVENT_QUEUE_HEADER_LEN},
    utils::{check_account_key, check_account_owner, check_signer, round_price},
};

/// The required accounts for a new_order instruction.
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
    #[allow(missing_docs)]
    #[cons(signer)]
    pub authority: &'a T,
}

impl<'a, 'b: 'a> Accounts<'a, AccountInfo<'b>> {
    pub(crate) fn parse(accounts: &'a [AccountInfo<'b>]) -> Result<Self, ProgramError> {
        let accounts_iter = &mut accounts.iter();
        let a = Self {
            market: next_account_info(accounts_iter)?,
            event_queue: next_account_info(accounts_iter)?,
            bids: next_account_info(accounts_iter)?,
            asks: next_account_info(accounts_iter)?,
            authority: next_account_info(accounts_iter)?,
        };
        Ok(a)
    }

    pub(crate) fn perform_checks(&self, program_id: &Pubkey) -> Result<(), ProgramError> {
        check_account_owner(
            self.market,
            &program_id.to_bytes(),
            AoError::WrongMarketOwner,
        )?;
        check_account_owner(
            self.event_queue,
            &program_id.to_bytes(),
            AoError::WrongEventQueueOwner,
        )?;
        check_account_owner(self.bids, &program_id.to_bytes(), AoError::WrongBidsOwner)?;
        check_account_owner(self.asks, &program_id.to_bytes(), AoError::WrongAsksOwner)?;
        #[cfg(not(feature = "lib"))]
        check_signer(self.authority).map_err(|e| {
            msg!("The market authority should be a signer for this instruction!");
            e
        })?;
        Ok(())
    }
}

/// Apply the new_order instruction to the provided accounts
pub fn process(
    program_id: &Pubkey,
    accounts: Accounts<AccountInfo>,
    mut params: NewOrderParams,
) -> ProgramResult {
    accounts.perform_checks(program_id)?;
    let mut market_state = MarketState::get(accounts.market)?;

    check_accounts(&accounts, &market_state)?;

    // Round price to nearest valid price tick
    params.limit_price = round_price(market_state.tick_size, params.limit_price, params.side);

    let callback_info_len = market_state.callback_info_len as usize;

    msg!("New Order: Creating order book");
    // sol_log_compute_units();
    let mut order_book = OrderBookState::new(
        accounts.bids,
        accounts.asks,
        market_state.callback_info_len as usize,
        market_state.callback_id_len as usize,
    )?;
    // sol_log_compute_units();

    if params.callback_info.len() != callback_info_len {
        msg!("Invalid callback information");
        return Err(ProgramError::InvalidArgument);
    }

    msg!("New Order: Creating event queue");
    // sol_log_compute_units();

    let header = {
        let mut event_queue_data: &[u8] =
            &accounts.event_queue.data.borrow()[0..EVENT_QUEUE_HEADER_LEN];
        EventQueueHeader::deserialize(&mut event_queue_data)
            .unwrap()
            .check()?
    };
    let mut event_queue = EventQueue::new_safe(header, accounts.event_queue, callback_info_len)?;
    // sol_log_compute_units();

    msg!("New Order: Creating new order");
    // sol_log_compute_units();
    let order_summary =
        order_book.new_order(params, &mut event_queue, market_state.min_base_order_size)?;
    // sol_log_compute_units();
    msg!("Order summary : {:?}", order_summary);
    event_queue.write_to_register(order_summary);

    let mut event_queue_header_data: &mut [u8] = &mut accounts.event_queue.data.borrow_mut();
    event_queue
        .header
        .serialize(&mut event_queue_header_data)
        .unwrap();
    msg!("Committing changes");
    // sol_log_compute_units();
    order_book.commit_changes();
    // sol_log_compute_units();

    //Verify that fees were transfered. Fees are expected to be transfered by the caller program in order
    // to reduce the CPI call stack depth.
    if accounts.market.lamports() - market_state.initial_lamports
        < market_state
            .fee_budget
            .checked_add(market_state.cranker_reward)
            .unwrap()
    {
        msg!("Fees were not correctly payed during caller runtime.");
        return Err(AoError::FeeNotPayed.into());
    }
    market_state.fee_budget = accounts.market.lamports() - market_state.initial_lamports;
    order_book.release(accounts.bids, accounts.asks);

    Ok(())
}

fn check_accounts<'a, 'b: 'a>(
    accounts: &Accounts<'a, AccountInfo<'b>>,
    market_state: &MarketState,
) -> ProgramResult {
    check_account_key(
        accounts.event_queue,
        &market_state.event_queue,
        AoError::WrongEventQueueAccount,
    )?;
    check_account_key(accounts.bids, &market_state.bids, AoError::WrongBidsAccount)?;
    check_account_key(accounts.asks, &market_state.asks, AoError::WrongAsksAccount)?;
    #[cfg(not(feature = "lib"))]
    check_account_key(
        accounts.authority,
        &market_state.caller_authority,
        AoError::WrongCallerAuthority,
    )?;

    Ok(())
}
