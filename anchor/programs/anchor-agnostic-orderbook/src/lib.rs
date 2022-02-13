use anchor_lang::prelude::*;

use aob::critbit::Slab;
use aob::state::MarketState;

declare_id!("aaobKniTtDGvCZces7GH5UReLYP671bBkB96ahr9x3e");

#[program]
pub mod anchor_agnostic_orderbook {
    use std::ops::DerefMut;
    use aob::state::{AccountTag, EventQueueHeader};

    use super::*;

    pub fn create_market(
        ctx: Context<CreateMarket>,
        // The caller authority will be the required signer for all market instructions.
        //
        // In practice, it will almost always be a program-derived address..
        caller_authority: Pubkey,
        // Callback information can be used by the caller to attach specific information to all
        // orders.
        //
        // An example of this would be to store a public key to uniquely identify the owner of a
        // particular order.
        //
        // This example would thus require a value of 32
        callback_info_len: u64,
        // The prefix length of callback information which is used to identify self-trading
        callback_id_len: u64,
        // The minimum order size that can be inserted into the orderbook after matching.
        min_base_order_size: u64,
        // Enables the limiting of price precision on the orderbook (price ticks)
        tick_size: u64,
        // Fixed fee for every new order operation. A higher fee increases incentives for cranking.
        cranker_reward: u64,
    ) -> ProgramResult {
        let market_state = &mut ctx.accounts.market.load_init()?;
        *market_state.deref_mut() = aob::state::MarketState {
            tag: AccountTag::Market as u64,
            caller_authority: caller_authority.to_bytes(),
            event_queue: ctx.accounts.event_queue.key.to_bytes(),
            bids: ctx.accounts.bids.key.to_bytes(),
            asks: ctx.accounts.asks.key.to_bytes(),
            callback_info_len,
            callback_id_len,
            fee_budget: 0,
            initial_lamports: ctx.accounts.market.to_account_info().lamports(),
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

    // pub fn new_order(ctx: Context<Initialize>) -> ProgramResult {
    //     Ok(())
    // }
    //
    // pub fn consume_events(ctx: Context<Initialize>) -> ProgramResult {
    //     Ok(())
    // }
    //
    // pub fn cancel_order(ctx: Context<Initialize>) -> ProgramResult {
    //     Ok(())
    // }
}

#[derive(Accounts)]
pub struct CreateMarket<'info> {
    // TODO PDAs?
    #[account(init, payer = payer)]
    pub market: AccountLoader<'info, MarketState>,
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

