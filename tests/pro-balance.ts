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

describe("pro-balance", () => {
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

  before(async () => {
    masterAddress = (
      await anchor.web3.PublicKey.findProgramAddress(
        [Buffer.from("master")],
        new anchor.web3.PublicKey(
          "8ZwcssGn5vKE1d6oBNNTTjDsFyTDKSuPtoooZQe9MHXb"
        )
      )
    )[0];

    const tx1 = await program.methods
      .initMaster()
      .accounts({
        master: masterAddress,
        payer: anchor.getProvider().publicKey,
        admin: admin,
        operator: operator,
        systemProgram: anchor.web3.SystemProgram.programId,
      })
      .rpc();
    //console.log("Your INITMASTER transaction signature", tx1);
  });

  it("Is initialized!", async () => {
    const masterAcc = await program.account.master.fetch(masterAddress);
    expect(masterAcc.balance.toString()).to.be.eq("0");
    expect(masterAcc.lastWithdrawTime.toString()).to.be.eq("0");
    expect(masterAcc.operator.toString()).to.be.eq(operator.toString());
    expect(masterAcc.admin.toString()).to.be.eq(admin.toString());
  });

  it("can deposit", async () => {
    await program.methods
      .deposit(depositAmount)
      .accounts({
        master: masterAddress,
        user: anchor.getProvider().publicKey,
        systemProgram: anchor.web3.SystemProgram.programId,
      })
      .rpc();

    const masterAcc = await program.account.master.fetch(masterAddress);
    expect(masterAcc.balance.toString()).to.be.eq(depositAmount.toString());
  });

  it("can withdraw", async () => {
    await program.methods
      .deposit(depositAmount)
      .accounts({
        master: masterAddress,
        user: anchor.getProvider().publicKey,
        systemProgram: anchor.web3.SystemProgram.programId,
      })
      .rpc();

    const tx_withdraw = await program.methods
      .withdraw(new anchor.BN(100))
      .accounts({
        master: masterAddress,
        admin: anchor.getProvider().publicKey,
        systemProgram: anchor.web3.SystemProgram.programId,
      })
      .rpc();
    //console.log("Your WITHDRAW transaction signature", tx_withdraw);
  });

  it("can sendWithdraw", async () => {
    await program.methods
      .deposit(depositAmount)
      .accounts({
        master: masterAddress,
        user: anchor.getProvider().publicKey,
        systemProgram: anchor.web3.SystemProgram.programId,
      })
      .rpc();

    const tx_send = await program.methods
      .sendWithdraw(new anchor.BN(1000000))
      .accounts({
        master: masterAddress,
        operator: anchor.getProvider().publicKey,
        receiver: user1.publicKey,
        systemProgram: anchor.web3.SystemProgram.programId,
      })
      .rpc();
    // console.log("Your SEND WITHDRAW transaction signature", tx_send);
  });

  it("can sendWithdraw", async () => {
    await program.methods
      .deposit(depositAmount)
      .accounts({
        master: masterAddress,
        user: anchor.getProvider().publicKey,
        systemProgram: anchor.web3.SystemProgram.programId,
      })
      .rpc();

    const tx_send = await program.methods
      .sendWithdraw(new anchor.BN(1000000))
      .accounts({
        master: masterAddress,
        operator: anchor.getProvider().publicKey,
        receiver: user1.publicKey,
        systemProgram: anchor.web3.SystemProgram.programId,
      })
      .rpc();
    //console.log("Your SEND WITHDRAW transaction signature", tx_send);
  });

  it("cant sendWithdraw with unauthorized user", async () => {
    try {
      await program.methods
        .sendWithdraw(new anchor.BN(1000000))
        .accounts({
          master: masterAddress,
          operator: user1.publicKey,
          receiver: user1.publicKey,
          systemProgram: anchor.web3.SystemProgram.programId,
        })
        .signers([user1])
        .rpc();

      assert.ok(false);
    } catch (_err) {
      // console.log(_err);
      assert.isTrue(_err instanceof AnchorError);
      const err: AnchorError = _err;
      const errMsg = "An address constraint was violated";
      assert.strictEqual(err.error.errorMessage, errMsg);
    }
  });

  it("cant init same ATA twice", async () => {
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
    // await program.methods
    //   .initAta()
    //   .accounts({
    //     master: masterAddress,
    //     masterAta: masterAta,
    //     tokenMint: mintSC,
    //     user: user.publicKey,
    //     tokenProgram: TOKEN_PROGRAM_ID,
    //     associatedTokenProgram: ASSOCIATED_TOKEN_PROGRAM_ID,
    //     systemProgram: anchor.web3.SystemProgram.programId,
    //   })
    //   .rpc();

    // await program.methods
    //   .initAta()
    //   .accounts({
    //     master: masterAddress,
    //     masterAta: masterAta,
    //     tokenMint: mintSC,
    //     user: user.publicKey,
    //     tokenProgram: TOKEN_PROGRAM_ID,
    //     associatedTokenProgram: ASSOCIATED_TOKEN_PROGRAM_ID,
    //     systemProgram: anchor.web3.SystemProgram.programId,
    //   })
    //   .rpc();
  });
});
