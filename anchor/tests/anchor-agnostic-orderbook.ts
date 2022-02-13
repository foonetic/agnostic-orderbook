import * as anchor from '@project-serum/anchor';
import {BN, getProvider} from '@project-serum/anchor';
import {Keypair, SystemProgram} from "@solana/web3.js";

describe('anchor-agnostic-orderbook', () => {
  anchor.setProvider(anchor.Provider.env());

  const program = anchor.workspace.AnchorAgnosticOrderbook;

  it('create market', async () => {
    const market = Keypair.generate();
    const eventQueue = Keypair.generate();
    const bids = Keypair.generate();
    const asks = Keypair.generate();

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

    const account = await getProvider().connection.getAccountInfo(market.publicKey);
    console.log(account);
  });
});
