import * as anchor from "@anchor-lang/core";
import { Program } from "@anchor-lang/core";
import { AnchorEscrowQ22026 } from "../target/types/anchor_escrow_q2_2026";
import { Commitment, Keypair, LAMPORTS_PER_SOL, PublicKey, SystemProgram } from "@solana/web3.js";
import {
  TOKEN_PROGRAM_ID,
  createMint,
  getAssociatedTokenAddressSync,
  getOrCreateAssociatedTokenAccount,
  mintTo,
} from "@solana/spl-token";
import NodeWallet from "@anchor-lang/core/dist/cjs/nodewallet";
import { BN } from "bn.js";
import { randomBytes } from "crypto";
import { ASSOCIATED_PROGRAM_ID } from "@anchor-lang/core/dist/cjs/utils/token";
import { expect } from "chai";

const commitment: Commitment = "confirmed";

describe("anchor-escrow-q2-2026", () => {
  const confirmTx = async (signature: string) => {
    const latestBlockhash = await anchor.getProvider().connection.getLatestBlockhash();
    await anchor.getProvider().connection.confirmTransaction(
      {
        signature,
        ...latestBlockhash,
      },
      commitment
    )
  }

  const confirmTxs = async (signatures: string[]) => {
    await Promise.all(signatures.map(confirmTx))
  }
  // Configure the client to use the local cluster.
  const provider = anchor.AnchorProvider.env();
  anchor.setProvider(provider);

  const program = anchor.workspace.anchorEscrowQ22026 as Program<AnchorEscrowQ22026>;

  const connection = provider.connection;
  
  const payer = provider.wallet as NodeWallet;
  const taker = Keypair.generate();

  let mintA : PublicKey;
  let mintB : PublicKey;

  let makerAtaA: PublicKey;
  let makerAtaB: PublicKey;

  let takerAtaA: PublicKey;
  let takerAtaB: PublicKey;

  let vault: PublicKey;

  const seed = new BN(randomBytes(8));

  const escrow = PublicKey.findProgramAddressSync([
    Buffer.from("escrow"), payer.publicKey.toBuffer(), seed.toBuffer("le", 8)
  ], program.programId)[0];

  it("Request airdrop to taker!", async () => {
    await Promise.all([payer, taker].map(async (k) => {

      // Request airdrop for the 'auth' account and confirm the transaction
      return await anchor.getProvider().connection.requestAirdrop(k.publicKey, 100 * anchor.web3.LAMPORTS_PER_SOL)
    })).then(confirmTxs);

  });

  it("Mint Tokens to Maker and Taker!", async () => {

    mintA = await createMint(
      connection,
      payer.payer,
      provider.publicKey,
      provider.publicKey,
      6,
    );

    console.log("mintA", mintA.toBase58());

    vault = getAssociatedTokenAddressSync(mintA, escrow, true);

    mintB = await createMint(
      connection,
      payer.payer,
      provider.publicKey,
      provider.publicKey,
      6,    
    );    
    console.log("mintB", mintB.toBase58());

    makerAtaA = (await getOrCreateAssociatedTokenAccount(
      connection,
      payer.payer,
      mintA,
      provider.publicKey,
    )).address;

    makerAtaB = (await getOrCreateAssociatedTokenAccount(
      connection,
      payer.payer,
      mintB,
      provider.publicKey,
    )).address;    

    takerAtaA = (await getOrCreateAssociatedTokenAccount(
      connection,
      payer.payer,
      mintA,
      taker.publicKey,
    )).address;

    takerAtaB = (await getOrCreateAssociatedTokenAccount(
      connection,
      payer.payer,
      mintB,
      taker.publicKey,
    )).address;


  await mintTo(
    connection,
    payer.payer,
    mintA,
    makerAtaA,
    payer.payer,
    1000_000_000,
  );
  console.log("tokens mints to makerataA", makerAtaA.toBase58());


  await mintTo(
    connection,
    payer.payer,
    mintB,
    takerAtaB,
    payer.payer,
    1000_000_000,
  );
  console.log("tokens mints to makerataB", makerAtaB.toBase58());

  });


  it("Make!", async () => {

    const initialMakerAtaABalance = await provider.connection.getTokenAccountBalance(makerAtaA);
    console.log("initial Maker Ata A balance", initialMakerAtaABalance.value.amount);

    const tx = await program.methods.make(
      seed,
      new BN(1_000_000),
      new BN(1_000_000),
    ).accountsStrict({
      maker: payer.publicKey,
      mintA: mintA,
      mintB: mintB,
      makerAtaA: makerAtaA,
      escrow: escrow,
      vault: vault,
      tokenProgram: TOKEN_PROGRAM_ID,
      associatedTokenProgram: ASSOCIATED_PROGRAM_ID,
      systemProgram: SystemProgram.programId,
    })
    .rpc();

    await confirmTx(tx)

    const finalVaultBalance = await provider.connection.getTokenAccountBalance(vault);
    console.log("vault balance", finalVaultBalance.value.amount);
    const finalMakerAtaABalance = await provider.connection.getTokenAccountBalance(makerAtaA);
    console.log("Final Maker Ata A  balance", finalMakerAtaABalance.value.amount);
    console.log("make tx", tx);

  });

  it("Refund!", async () => {

    const tx = await program.methods.refund(
    ).accountsPartial({
      maker: provider.publicKey,
      mintA: mintA,
      makerAtaA: makerAtaA,
      vault: vault,
      escrow: escrow,
      tokenProgram: TOKEN_PROGRAM_ID,
      systemProgram: SystemProgram.programId,
    })
    .rpc();

    await confirmTx(tx)
    
    expect(await provider.connection.getBalance(vault)).to.equal(0);
    const vaultStateInfo = await provider.connection.getAccountInfo(vault);
    expect(vaultStateInfo).to.be.null;
    console.log("Refund tx", tx);
  });

  it("Take!", async () => {

    const tx = await program.methods.take(
    ).accountsPartial({
      taker: taker.publicKey,
      maker: provider.publicKey,
      mintA: mintA,
      mintB: mintB,
      vault: vault,
      makerAtaB: makerAtaB,
      takerAtaA: takerAtaA,
      takerAtaB: takerAtaB,
      escrow: escrow,
      tokenProgram: TOKEN_PROGRAM_ID,
      associatedTokenProgram: ASSOCIATED_PROGRAM_ID,
      systemProgram: SystemProgram.programId,
    })
    .signers([taker])
    .rpc();

    await confirmTx(tx)

    expect(await provider.connection.getBalance(vault)).to.equal(0);
    const vaultStateInfo = await provider.connection.getAccountInfo(vault);
    expect(vaultStateInfo).to.be.null;
    console.log("Take tx", tx);
  });
});
