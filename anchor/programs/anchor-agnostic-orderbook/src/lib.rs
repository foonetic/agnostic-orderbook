use std::ops::DerefMut;

use anchor_lang::prelude::*;
use anchor_lang::solana_program::log::sol_log_compute_units;
use borsh::BorshDeserialize;
use borsh::BorshSerialize;
use bytemuck::{Pod, Zeroable};
use num_traits::FromPrimitive;

use crate::aob::critbit::Slab;
use crate::aob::error::AoError;
use crate::aob::orderbook::OrderBookState;
use crate::aob::orderbook::OrderSummary;
use crate::aob::params::NewOrderParams;
use crate::aob::state::get_side_from_order_id;
use crate::aob::state::EventQueue;
use crate::aob::state::{AccountTag, MarketState};
use crate::aob::state::{SelfTradeBehavior, Side};
use crate::aob::utils::fp32_mul;
use crate::aob::utils::round_price;

pub mod aob;

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
        let market_state = &mut ctx.accounts.market.load_init()?;
        **market_state = crate::aob::state::MarketState {
            tag: AccountTag::Market as u64,
            caller_authority: caller_authority.to_bytes(),
            event_queue: ctx.accounts.event_queue.key().to_bytes(),
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

        let event_queue = &mut ctx.accounts.event_queue.load_init()?;
        event_queue.callback_info_len = callback_info_len;

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
        callback_info: [u8; 32],
        post_only: bool,
        post_allowed: bool,
        self_trade_behavior: u8,
    ) -> ProgramResult {
        let mut market_state = ctx.accounts.market.load_mut()?;
        let side = Side::from_u8(side).ok_or(AoError::FailedToDeserialize)?;
        let self_trade_behavior =
            SelfTradeBehavior::from_u8(self_trade_behavior).ok_or(AoError::FailedToDeserialize)?;
        let limit_price = round_price(market_state.tick_size, limit_price, side);
        let _callback_info_len = market_state.callback_info_len as usize;

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
        let mut event_queue = ctx.accounts.event_queue.load_mut()?;
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
        msg!("{:?}", order_summary);
        // event_queue.write_to_register(order_summary);

        msg!("Committing changes");
        sol_log_compute_units();
        order_book.commit_changes();
        sol_log_compute_units();

        // Verify that fees were transfered. Fees are expected to be transfered by the caller
        // program in order to reduce the CPI call stack depth.
        if ctx.accounts.market.to_account_info().lamports() - market_state.initial_lamports
            < market_state
                .fee_budget
                .checked_add(market_state.cranker_reward)
                .unwrap()
        {
            msg!("Fees were not correctly payed during caller runtime.");
            return Err(AoError::FeeNotPayed.into());
        }
        market_state.fee_budget =
            ctx.accounts.market.to_account_info().lamports() - market_state.initial_lamports;
        order_book.release(&ctx.accounts.bids, &ctx.accounts.asks);

        msg!("BUFFER {:?}", event_queue.buffer);
        Ok(())
    }

    pub fn cancel_order(ctx: Context<CancelOrder>, order_id: u128) -> ProgramResult {
        let market_state = &mut ctx.accounts.market.load_mut()?;
        let mut order_book = OrderBookState::new(
            &ctx.accounts.bids,
            &ctx.accounts.asks,
            market_state.callback_info_len as usize,
            market_state.callback_id_len as usize,
        )?;

        let slab = order_book.get_tree(get_side_from_order_id(order_id));
        // let node = slab.remove_by_key(order_id).ok_or(AoError::OrderNotFound)?;
        // let leaf_node = node.as_leaf().unwrap();
        // let total_base_qty = leaf_node.base_quantity;
        // let total_quote_qty = fp32_mul(leaf_node.base_quantity, leaf_node.price());
        //
        // let order_summary = OrderSummary {
        //     posted_order_id: None,
        //     total_base_qty,
        //     total_quote_qty,
        //     total_base_qty_posted: 0,
        // };

        // let event_queue = &mut ctx.accounts.event_queue.load_mut()?;
        // let event_queue = &mut ctx.accounts.event_queue;
        // event_queue.write_to_register(order_summary);

        order_book.commit_changes();
        order_book.release(&ctx.accounts.bids, &ctx.accounts.asks);

        Ok(())
    }

    pub fn consume_events(
        ctx: Context<ConsumeEvents>,
        number_of_entries_to_consume: u64,
    ) -> ProgramResult {
        let market_state = &mut ctx.accounts.market.load_mut()?;

        // Reward payout
        // let event_queue = &mut ctx.accounts.event_queue.load_mut()?;
        let event_queue = &mut ctx.accounts.event_queue.load_mut()?;
        let number_of_entries_to_consume = event_queue.count.min(number_of_entries_to_consume);
        let reward = (market_state.fee_budget * number_of_entries_to_consume)
            .checked_div(event_queue.count as u64)
            .ok_or(AoError::NoOperations)
            .unwrap();
        market_state.fee_budget -= reward;
        let market_account = ctx.accounts.market.to_account_info();
        **market_account.try_borrow_mut_lamports()? -= reward;
        let reward_target_account = ctx.accounts.reward_target.to_account_info();
        **reward_target_account.try_borrow_mut_lamports()? += reward;

        // Pop Events
        for _ in 0..number_of_entries_to_consume {
            event_queue.pop_front();
        }

        msg!(
            "Number of events consumed: {:?}",
            number_of_entries_to_consume
        );

        Ok(())
    }

    pub fn close_market(ctx: Context<CloseMarket>) -> ProgramResult {
        let market_state = &mut ctx.accounts.market.load_mut()?;

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

        // let event_queue = &ctx.accounts.event_queue.load_mut()?;
        let event_queue = &ctx.accounts.event_queue.load()?;
        // Check if all events have been processed
        if event_queue.count != 0 {
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

#[derive(Accounts)]
pub struct CreateMarket<'info> {
    // TODO PDAs?
    #[account(init, payer = payer)]
    pub market: AccountLoader<'info, MarketState>,
    // TODO pass in space size instead of just max
    #[account(init, payer = payer, space = 8 + std::mem::size_of::<EventQueue>())]
    pub event_queue: AccountLoader<'info, EventQueue>,
    // TODO it would be nicer to parameterize with the actual types `T` instead of the `AccountInfo`
    // escape hatch.
    //
    // We want to let Anchor do the serde boilerplate, checks etc. for us
    // but `Slab` does not implement `Clone` (which Anchor needs -- see comment on `Slab` struct)
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
    pub market: AccountLoader<'info, MarketState>,
    #[account(mut)]
    pub event_queue: AccountLoader<'info, EventQueue>,
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
    pub market: AccountLoader<'info, MarketState>,
    #[account(mut)]
    pub event_queue: AccountLoader<'info, EventQueue>,
    #[account(mut)]
    pub bids: AccountInfo<'info>,
    #[account(mut)]
    pub asks: AccountInfo<'info>,
    #[account(mut)]
    pub authority: Signer<'info>,
}

#[derive(Accounts)]
pub struct ConsumeEvents<'info> {
    pub market: AccountLoader<'info, MarketState>,
    #[account(mut)]
    pub event_queue: AccountLoader<'info, EventQueue>,
    #[account(mut)]
    pub authority: Signer<'info>,
    pub reward_target: AccountInfo<'info>,
}

#[derive(Accounts)]
pub struct CloseMarket<'info> {
    #[account(mut)]
    pub market: AccountLoader<'info, MarketState>,
    #[account(mut)]
    pub event_queue: AccountLoader<'info, EventQueue>,
    #[account(mut)]
    pub bids: AccountInfo<'info>,
    #[account(mut)]
    pub asks: AccountInfo<'info>,
    #[account(mut)]
    pub authority: Signer<'info>,
    #[account(mut)]
    pub lamports_target_account: Signer<'info>,
}
