import * as anchor from "@coral-xyz/anchor";
import { Program, AnchorError } from "@coral-xyz/anchor";

import {
  createMint,
  createAccount,
  getAccount,
  getOrCreateAssociatedTokenAccount,
  transfer,
  mintTo,
  TOKEN_PROGRAM_ID,
  getAssociatedTokenAddress,
  ASSOCIATED_TOKEN_PROGRAM_ID,
} from "@solana/spl-token";

import { ProBalance } from "../target/types/pro_balance";
import { assert, expect } from "chai";

describe("tokenActions", () => {
  // Configure the client to use the local cluster.
  anchor.setProvider(anchor.AnchorProvider.env());

  const program = anchor.workspace.ProBalance as Program<ProBalance>;

  const provider = anchor.AnchorProvider.env();
  const user = provider.wallet;

  const user1 = anchor.web3.Keypair.generate();

  let masterAddress;

  const depositAmount = new anchor.BN(1000000000);

  const operator = anchor.getProvider().publicKey;
  const admin = anchor.getProvider().publicKey;
  const LAMPORTS_PER_SOL = 1000000000;
  const person1 = anchor.web3.Keypair.generate();
  const PaYeR = anchor.web3.Keypair.generate();
  const mintAuthSC = anchor.web3.Keypair.generate();
  const mintKeypairSC = anchor.web3.Keypair.generate();
  let mintSC;
  let person1ATA;

  it("can deposit token", async () => {
    masterAddress = (
      await anchor.web3.PublicKey.findProgramAddress(
        [Buffer.from("master")],
        new anchor.web3.PublicKey(
          "8ZwcssGn5vKE1d6oBNNTTjDsFyTDKSuPtoooZQe9MHXb"
        )
      )
    )[0];

    // const tx1 = await program.methods
    //   .initMaster()
    //   .accounts({
    //     master: masterAddress,
    //     payer: anchor.getProvider().publicKey,
    //     admin: admin,
    //     operator: operator,
    //     systemProgram: anchor.web3.SystemProgram.programId,
    //   })
    //   .rpc();

    await provider.connection.confirmTransaction(
      await provider.connection.requestAirdrop(
        PaYeR.publicKey,
        2 * LAMPORTS_PER_SOL
      )
    );

    // Stablecoin mint
    mintSC = await createMint(
      provider.connection,
      PaYeR,
      mintAuthSC.publicKey,
      mintAuthSC.publicKey,
      10,
      mintKeypairSC,
      undefined,
      TOKEN_PROGRAM_ID
    );

    const masterAta = await getAssociatedTokenAddress(
      mintSC,
      masterAddress,
      true
    );
    await program.methods
      .initAta()
      .accounts({
        master: masterAddress,
        masterAta: masterAta,
        tokenMint: mintSC,
        user: anchor.getProvider().publicKey,
        tokenProgram: TOKEN_PROGRAM_ID,
        associatedTokenProgram: ASSOCIATED_TOKEN_PROGRAM_ID,
        systemProgram: anchor.web3.SystemProgram.programId,
      })
      .rpc();

    // Initialise ATA
    await getOrCreateAssociatedTokenAccount(
      provider.connection,
      PaYeR,
      mintSC,
      user.publicKey
    );

    await getOrCreateAssociatedTokenAccount(
      provider.connection,
      PaYeR,
      mintSC,
      user1.publicKey
    );

    await getOrCreateAssociatedTokenAccount(
      provider.connection,
      PaYeR,
      mintSC,
      anchor.getProvider().publicKey
    );

    person1ATA = await getAssociatedTokenAddress(mintSC, user.publicKey);
    const user1ATA = await getAssociatedTokenAddress(mintSC, user1.publicKey);
    const adminATA = await getAssociatedTokenAddress(
      mintSC,
      anchor.getProvider().publicKey
    );

    // Top up test account with SPL
    await mintTo(
      provider.connection,
      PaYeR,
      mintSC,
      person1ATA,
      mintAuthSC,
      100,
      [],
      undefined,
      TOKEN_PROGRAM_ID
    );

    await mintTo(
      provider.connection,
      PaYeR,
      mintSC,
      user1ATA,
      mintAuthSC,
      100,
      [],
      undefined,
      TOKEN_PROGRAM_ID
    );
    let userTokenBalance = (
      await provider.connection.getParsedAccountInfo(person1ATA)
    ).value.data.parsed.info.tokenAmount.amount;
    // console.log(userTokenBalance);

    let programTokenBalance = await provider.connection.getParsedAccountInfo(
      masterAta
    );
    //console.log(programTokenBalance);

    assert.equal(userTokenBalance, 100);
    //  assert.equal(programTokenBalance, 0);

    await program.methods
      .depositToken(new anchor.BN(10))
      .accounts({
        master: masterAddress,
        tokenProgram: TOKEN_PROGRAM_ID,
        systemProgram: anchor.web3.SystemProgram.programId,
        from: person1ATA,
        masterAta: masterAta,
        tokenMint: mintSC,
        user: anchor.getProvider().publicKey,
      })
      .rpc();

    userTokenBalance = (
      await provider.connection.getParsedAccountInfo(person1ATA)
    ).value.data.parsed.info.tokenAmount.amount;
    // console.log(userTokenBalance);

    programTokenBalance = (
      await provider.connection.getParsedAccountInfo(masterAta)
    ).value.data.parsed.info.tokenAmount.amount;
    //console.log(programTokenBalance);

    assert.equal(userTokenBalance, 90);
    assert.equal(programTokenBalance, 10);

    await program.methods
      .depositToken(new anchor.BN(30))
      .accounts({
        master: masterAddress,
        tokenProgram: TOKEN_PROGRAM_ID,
        systemProgram: anchor.web3.SystemProgram.programId,
        from: user1ATA,
        masterAta: masterAta,
        tokenMint: mintSC,
        user: user1.publicKey,
      })
      .signers([user1])
      .rpc();

    const user1TokenBalance = (
      await provider.connection.getParsedAccountInfo(user1ATA)
    ).value.data.parsed.info.tokenAmount.amount;

    // console.log(user1TokenBalance);

    programTokenBalance = (
      await provider.connection.getParsedAccountInfo(masterAta)
    ).value.data.parsed.info.tokenAmount.amount;
    //console.log(programTokenBalance);

    assert.equal(user1TokenBalance, 70);
    assert.equal(programTokenBalance, 40);

    await program.methods
      .withdrawToken(new anchor.BN(30))
      .accounts({
        master: masterAddress,
        masterAta: masterAta,
        admin: anchor.getProvider().publicKey,
        adminAta: adminATA,
        tokenMint: mintSC,
        tokenProgram: TOKEN_PROGRAM_ID,
        systemProgram: anchor.web3.SystemProgram.programId,
      })
      .rpc();

    programTokenBalance = (
      await provider.connection.getParsedAccountInfo(masterAta)
    ).value.data.parsed.info.tokenAmount.amount;
    //console.log(programTokenBalance);

    assert.equal(programTokenBalance, 10);

    const adminTokenBalance = (
      await provider.connection.getParsedAccountInfo(adminATA)
    ).value.data.parsed.info.tokenAmount.amount;

    assert.equal(adminTokenBalance, 120);

    await program.methods
      .sendWithdrawToken(new anchor.BN(10))
      .accounts({
        master: masterAddress,
        masterAta: masterAta,
        operator: anchor.getProvider().publicKey,
        receiverAta: user1ATA,
        receiver: user1.publicKey,
        tokenMint: mintSC,
        tokenProgram: TOKEN_PROGRAM_ID,
        systemProgram: anchor.web3.SystemProgram.programId,
      })
      .rpc();

    programTokenBalance = (
      await provider.connection.getParsedAccountInfo(masterAta)
    ).value.data.parsed.info.tokenAmount.amount;

    assert.equal(programTokenBalance, 0);
  });
});
