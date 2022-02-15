#![cfg(feature = "test-bpf")]

use std::cell::RefCell;
use std::rc::Rc;

use anyhow::anyhow;
use borsh::BorshDeserialize;
use solana_sdk::account::Account;
use solana_sdk::pubkey::Pubkey;
use solana_sdk::signature::Keypair;
use solana_sdk::system_instruction;
use solana_sdk::{signature::Signer, transaction::Transaction};
use solana_validator::test_validator::*;
use agnostic_orderbook::instruction::{cancel_order, create_market, new_order};

use aob::params::{CancelOrderParams, CreateMarketParams, NewOrderParams};
use aob::state::{
    EventQueue, EventQueueHeader, MarketState, OrderSummary, SelfTradeBehavior, Side,
    MARKET_STATE_LEN,
};

#[test]
fn test_agnostic_orderbook() -> anyhow::Result<()> {
    solana_logger::setup_with_default("solana_runtime::message_processor=debug");
    let (test_validator, payer) = TestValidatorGenesis::default()
        .add_program("agnostic_orderbook", agnostic_orderbook::ID)
        .start();
    let rpc_client = test_validator.get_rpc_client();
    let blockhash = rpc_client.get_latest_blockhash()?;
// TODO devnet
    // rpc_client.request_airdrop(&user.pubkey(), 1_000_000)?;
    // let rpc_client = RpcClient::new_with_commitment("https://api.devnet.solana.com".to_string(), CommitmentConfig::confirmed());

    // Create Event Queue account
    let event_queue = Keypair::new();
    let event_queue_tx = Transaction::new_signed_with_payer(
        &[system_instruction::create_account(
            &payer.pubkey(),
            &event_queue.pubkey(),
            rpc_client.get_minimum_balance_for_rent_exemption(1_000_000)?,
            1_000_000,
            &agnostic_orderbook::id(),
        )],
        Some(&payer.pubkey()),
        &vec![&payer, &event_queue],
        blockhash,
    );
    rpc_client.send_and_confirm_transaction(&event_queue_tx)?;

    // Create Bids account
    let bids = Keypair::new();
    let bids_tx = Transaction::new_signed_with_payer(
        &[system_instruction::create_account(
            &payer.pubkey(),
            &bids.pubkey(),
            rpc_client.get_minimum_balance_for_rent_exemption(1_000_000)?,
            1_000_000,
            &agnostic_orderbook::id(),
        )],
        Some(&payer.pubkey()),
        &vec![&payer, &bids],
        blockhash,
    );
    rpc_client.send_and_confirm_transaction(&bids_tx)?;

    // Create Asks account
    let asks = Keypair::new();
    let asks_tx = Transaction::new_signed_with_payer(
        &[system_instruction::create_account(
            &payer.pubkey(),
            &asks.pubkey(),
            rpc_client.get_minimum_balance_for_rent_exemption(1_000_000)?,
            1_000_000,
            &agnostic_orderbook::id(),
        )],
        Some(&payer.pubkey()),
        &vec![&payer, &asks],
        blockhash,
    );
    rpc_client.send_and_confirm_transaction(&asks_tx)?;

    let blockhash = rpc_client.get_new_latest_blockhash(&blockhash)?; // refresh blockhash

    // Create Market account
    let market = Keypair::new();
    let market_tx = Transaction::new_signed_with_payer(
        &[
            system_instruction::create_account(
                &payer.pubkey(),
                &market.pubkey(),
                rpc_client.get_minimum_balance_for_rent_exemption(1_000_000)?,
                1_000_000,
                &agnostic_orderbook::id(),
            ),
            create_market(
                create_market::Accounts {
                    market: &market.pubkey(),
                    event_queue: &event_queue.pubkey(),
                    bids: &bids.pubkey(),
                    asks: &asks.pubkey(),
                },
                CreateMarketParams {
                    caller_authority: payer.pubkey().to_bytes(),
                    callback_info_len: 32,
                    callback_id_len: 32,
                    min_base_order_size: 10,
                    tick_size: 1,
                    cranker_reward: 0,
                },
            ),
        ],
        Some(&payer.pubkey()),
        &vec![&payer, &market],
        blockhash,
    );
    rpc_client.send_and_confirm_transaction(&market_tx)?;

    let mut market_data = rpc_client.get_account(&market.pubkey())?.data;
    let market_state =
        bytemuck::try_from_bytes_mut::<MarketState>(&mut market_data[..MARKET_STATE_LEN])?;

    // Create bid order to buy 1000 units of base for 1000 units of quote
    let new_bid_tx = Transaction::new_signed_with_payer(
        &[new_order(
            new_order::Accounts {
                market: &market.pubkey(),
                event_queue: &Pubkey::new_from_array(market_state.event_queue),
                bids: &Pubkey::new_from_array(market_state.bids),
                asks: &Pubkey::new_from_array(market_state.asks),
                authority: &Pubkey::new_from_array(market_state.caller_authority),
            },
            NewOrderParams {
                max_base_qty: 1000,
                max_quote_qty: 1000,
                limit_price: 1000,
                side: Side::Bid,
                callback_info: Pubkey::new_unique().to_bytes().to_vec(),
                post_only: false,
                post_allowed: true,
                self_trade_behavior: SelfTradeBehavior::CancelProvide,
                match_limit: 3,
            },
        )],
        Some(&payer.pubkey()),
        &vec![&payer],
        blockhash,
    );
    rpc_client.send_and_confirm_transaction(&new_bid_tx)?;

    // Create ask order to sell 1000 units of base for 1000 units quote, and
    // post an additional 100 units of base
    let new_ask_tx = Transaction::new_signed_with_payer(
        &[new_order(
            new_order::Accounts {
                market: &market.pubkey(),
                event_queue: &Pubkey::new_from_array(market_state.event_queue),
                bids: &Pubkey::new_from_array(market_state.bids),
                asks: &Pubkey::new_from_array(market_state.asks),
                authority: &Pubkey::new_from_array(market_state.caller_authority),
            },
            NewOrderParams {
                max_base_qty: 1100,
                max_quote_qty: 1000,
                limit_price: 1000,
                side: Side::Ask,
                callback_info: Pubkey::new_unique().to_bytes().to_vec(),
                post_only: false,
                post_allowed: true,
                self_trade_behavior: SelfTradeBehavior::CancelProvide,
                match_limit: 3,
            },
        )],
        Some(&payer.pubkey()),
        &vec![&payer],
        blockhash,
    );
    rpc_client.send_and_confirm_transaction(&new_ask_tx)?;

    // Confirm that 1000 units were filled
    let mut market_data = rpc_client.get_account(&market.pubkey())?.data;
    let market_state =
        bytemuck::try_from_bytes_mut::<MarketState>(&mut market_data[..MARKET_STATE_LEN])?;

    let mut event_queue_account =
        rpc_client.get_account(&Pubkey::new_from_array(market_state.event_queue))?;
    let event_queue = get_event_queue(&mut event_queue_account)?;
    let order_summary: OrderSummary = event_queue
        .read_register()?
        .ok_or(anyhow!("Unable to read event queue register"))?; // TODO make this less ugly
    println!("Fill order summary {:?}", order_summary);
    assert_eq!(
        order_summary.total_base_qty - order_summary.total_base_qty_posted,
        1000
    );

    // Cancel remaining order, and confirm that cancelled amount == 100
    let new_cancel_tx = Transaction::new_signed_with_payer(
        &[cancel_order(
            cancel_order::Accounts {
                market: &market.pubkey(),
                event_queue: &Pubkey::new_from_array(market_state.event_queue),
                bids: &Pubkey::new_from_array(market_state.bids),
                asks: &Pubkey::new_from_array(market_state.asks),
                authority: &Pubkey::new_from_array(market_state.caller_authority),
            },
            CancelOrderParams {
                order_id: order_summary.posted_order_id.unwrap(),
            },
        )],
        Some(&payer.pubkey()),
        &vec![&payer],
        blockhash,
    );
    rpc_client.send_and_confirm_transaction(&new_cancel_tx)?;

    let mut event_queue_account =
        rpc_client.get_account(&Pubkey::new_from_array(market_state.event_queue))?;
    let event_queue = get_event_queue(&mut event_queue_account)?;
    let order_summary: OrderSummary = event_queue
        .read_register()?
        .ok_or(anyhow!("Unable to read event queue register"))?; // TODO make this less ugly
    println!("Cancel order summary {:?}", order_summary);
    assert_eq!(order_summary.total_base_qty, 100);

    Ok(())
}

fn get_event_queue(event_queue_account: &mut Account) -> anyhow::Result<EventQueue<'_>> {
    let event_queue_header =
        EventQueueHeader::deserialize(&mut (&event_queue_account.data as &[u8]))?;
    Ok(EventQueue::new(
        event_queue_header,
        Rc::new(RefCell::new(&mut event_queue_account.data)),
        32,
    ))
}
