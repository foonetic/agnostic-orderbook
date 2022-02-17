import * as anchor from '@project-serum/anchor';
import {BN, getProvider} from '@project-serum/anchor';
import {Keypair, SystemProgram} from "@solana/web3.js";

describe('anchor-agnostic-orderbook', () => {
  anchor.setProvider(anchor.Provider.env());

  const program = anchor.workspace.AnchorAgnosticOrderbook;

  const market = Keypair.generate();
  const eventQueue = Keypair.generate();
  const bids = Keypair.generate();
  const asks = Keypair.generate();

  it('create market', async () => {
    const tx = await program.methods
        .createMarket(
            getProvider().wallet.publicKey,
            new BN(32),
            new BN(32),
            new BN(10),
            new BN(1),
            new BN(0)
        )
        .accounts({
          market: market.publicKey,
          eventQueue: eventQueue.publicKey,
          bids: bids.publicKey,
          asks: asks.publicKey,
          payer: getProvider().wallet.publicKey,
          systemProgram: SystemProgram.programId,
        })
        .signers([market, eventQueue, bids, asks])
        .rpc()
    console.log('create market', tx);
    console.log(program.account)
    console.log(await getProvider().connection.getAccountInfo(market.publicKey));
  });

  it('new bid', async () => {
    const tx2 = await program.methods
        .newOrder(
            new BN(1000),
            new BN(1000),
            new BN(1000),
            0,
            new BN(3),
            getProvider().wallet.publicKey.toBuffer(),
            false,
            true,
            1,
        )
        .accounts({
          market: market.publicKey,
          eventQueue: eventQueue.publicKey,
          bids: bids.publicKey,
          asks: asks.publicKey,
          authority: getProvider().wallet.publicKey,
        })
        .rpc()
    console.log('new bid', tx2);
  })

  it('new ask', async () => {
    const tx3 = await program.methods
        .newOrder(
            new BN(1100),
            new BN(1100),
            new BN(1000),
            1,
            new BN(3),
            getProvider().wallet.publicKey.toBuffer(),
            false,
            true,
            1,
        )
        .accounts({
          market: market.publicKey,
          eventQueue: eventQueue.publicKey,
          bids: bids.publicKey,
          asks: asks.publicKey,
          authority: getProvider().wallet.publicKey,
        })
        .rpc()
  })

  it('consume events', async () => {
    console.log(await program.account.eventQueue.fetch(eventQueue.publicKey));
  })

  it('new cancel', async () => {
    const tx = await program.methods
        .cancelOrder(
            new BN(1)
        )
        .accounts({
          market: market.publicKey,
          eventQueue: eventQueue.publicKey,
          bids: bids.publicKey,
          asks: asks.publicKey,
          authority: getProvider().wallet.publicKey,
        })
        .rpc()
    console.log('new cancel', tx);
  })

  // it('consume events', async() => {
  //   const tx = await program.methods.consume
  //   const eventQueueAccount = await program.account.eventQueue.fetch(eventQueue.publicKey);
  //   console.log(eventQueueAccount);
  // })

  // it('cancel order', async() => {
  //   const tx = await program.methods
  //       .cancel_order(
  //           new BN(1000),
  //           new BN(1000),
  //           new BN(1000),
  //           0,
  //           "",
  //           false,
  //           true,
  //           1,
  //           3
  //       )
  //       .accounts({
  //         market: market.publicKey,
  //         eventQueue: eventQueue.publicKey,
  //         bids: bids.publicKey,
  //         asks: asks.publicKey,
  //         authority: getProvider().wallet.publicKey,
  //       })
  //       .rpc()
  //   console.log('new order', tx);
  // })
});
