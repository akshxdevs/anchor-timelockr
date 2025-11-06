import * as anchor from "@coral-xyz/anchor";
import { Program } from "@coral-xyz/anchor";
import { AnchorTimelockr } from "../target/types/anchor_timelockr";
import { PublicKey, Keypair, SystemProgram } from "@solana/web3.js";
import { BN } from "bn.js";
import { assert, expect } from "chai";
import {
  TOKEN_PROGRAM_ID,
  createMint,
  getAccount,
  getOrCreateAssociatedTokenAccount,
  mintTo,
} from "@solana/spl-token";

describe("anchor-timelockr", () => {
  anchor.setProvider(anchor.AnchorProvider.env());
  const program = anchor.workspace.anchorTimelockr as Program<AnchorTimelockr>;
  const provider = anchor.getProvider();

  let owner: Keypair;
  let backup: Keypair;
  let mint: PublicKey;
  let vault: PublicKey;
  let vaultAta: PublicKey;
  let userAta: PublicKey;

  before(async () => {
    owner = Keypair.generate();
    backup = Keypair.generate();

    const owner_airdrop = await provider.connection.requestAirdrop(
      owner.publicKey,
      100 * anchor.web3.LAMPORTS_PER_SOL
    );
    await provider.connection.requestAirdrop(
      backup.publicKey,
      100 * anchor.web3.LAMPORTS_PER_SOL
    );
    await provider.connection.confirmTransaction(owner_airdrop, "confirmed");
    const ownerBalance = await provider.connection.getBalance(owner.publicKey);
    console.log("Owner Balance:", ownerBalance / anchor.web3.LAMPORTS_PER_SOL);

    await new Promise((resolve) => setTimeout(resolve, 2000));
  });

  it("Should initialize vault successfully", async () => {
    mint = await createMint(
      provider.connection,
      owner,
      owner.publicKey,
      null,
      6
    );

    const [vaultPda, vaultBump] = PublicKey.findProgramAddressSync(
      [Buffer.from("vault"), owner.publicKey.toBuffer()],
      program.programId
    );
    vault = vaultPda;

    vaultAta = (
      await getOrCreateAssociatedTokenAccount(
        provider.connection,
        owner,
        mint,
        vault,
        true
      )
    ).address;

    userAta = (
      await getOrCreateAssociatedTokenAccount(
        provider.connection,
        owner,
        mint,
        owner.publicKey
      )
    ).address;

    await mintTo(provider.connection, owner, mint, userAta, owner, 1_000_000);

    const unlockTime = new BN(Date.now() / 1000 + 10);
    const tx = await program.methods
      .initialize(unlockTime, backup.publicKey)
      .accounts({
        vault: vault,
        user: owner.publicKey,
        systemProgram: SystemProgram.programId,
      }as any)
      .signers([owner])
      .rpc();

    console.log("Initialize transaction signature:", tx);

    const vaultAccount = await program.account.vault.fetch(vault);
    expect(vaultAccount.owner.toString()).to.equal(owner.publicKey.toString());
    expect(vaultAccount.backupAdr.toString()).to.equal(
      backup.publicKey.toString()
    );
    expect(vaultAccount.bump).to.equal(vaultBump);
  });

  it("Should fail to initialize vault twice", async () => {
    const unlockTime = new BN(Date.now() / 1000 + 10);

    try {
      await program.methods
        .initialize(unlockTime, backup.publicKey)
        .accounts({
          vault: vault,
          user: owner.publicKey,
          systemProgram: SystemProgram.programId,
        }as any)
        .signers([owner])
        .rpc();
      expect.fail("Should have thrown an error");
    } catch (error) {
      expect(error.toString()).to.include("already in use");
    }
  });

  it("Should deposit tokens successfully", async () => {
    const depositAmount = new BN(1_000_000);

    const tx = await program.methods
      .deposite(depositAmount)
      .accounts({
        vaultAta: vaultAta,
        userAta: userAta,
        vault: vault,
        user: owner.publicKey,
        systemProgram: SystemProgram.programId,
        tokenProgram: TOKEN_PROGRAM_ID,
      }as any)
      .signers([owner])
      .rpc();

    console.log("Deposit transaction signature:", tx);

    const vaultAccount = await program.account.vault.fetch(vault);
    console.log("Vault balance: ", vaultAccount.amount.toNumber());
    expect(vaultAccount.amount.toNumber()).to.equal(depositAmount.toNumber());
  });

  it("Allows backup to trigger recovery", async () => {
    await program.methods
      .triggerRecovery()
      .accounts({
        vault: vault,
        user: backup.publicKey,
        systemProgram: SystemProgram.programId,
      }as any)
      .signers([backup])
      .rpc();

    const vaultAcc = await program.account.vault.fetch(vault);

    assert.equal(vaultAcc.recoveryEnabled, true);
    const now = Math.floor(Date.now() / 1000 + 9);
    console.log("Recovery delay time: ", vaultAcc.recoveryReqTime.toNumber());
    console.log("Now: ", now);
    assert.isTrue(vaultAcc.recoveryReqTime.toNumber() >= now); // at least 10 seconds added
  });
  it("Should allow withdrawal after unlock time", async () => {
    const beforeUserTokenAccount = await getAccount(
      provider.connection,
      userAta
    );
    console.log("User Token Account Balance: ", beforeUserTokenAccount.amount);
    await new Promise((resolve) => setTimeout(resolve, 11000)); // Wait 11 seconds
    const BeforeVaultAccount = await program.account.vault.fetch(vault);
    console.log(
      "Vault Balance Before Withdrawl: ",
      BeforeVaultAccount.amount.toNumber()
    );
    const tx = await program.methods
      .withdrawl()
      .accounts({
        owner: owner.publicKey,
        vaultAta: vaultAta,
        userAta: userAta,
        vault: vault,
        user: owner.publicKey,
        systemProgram: SystemProgram.programId,
        tokenProgram: TOKEN_PROGRAM_ID,
      }as any)
      .signers([owner])
      .rpc();

    console.log("Successful withdrawal transaction signature:", tx);

    const AfterVaultAccount = await program.account.vault.fetch(vault);
    console.log(
      "Vault Balance After Withdrawl: ",
      AfterVaultAccount.amount.toNumber()
    );
    expect(AfterVaultAccount.amount.toNumber()).to.equal(0);
    const userTokenAccount = await getAccount(provider.connection, userAta);
    console.log("User Token Account Balance: ", userTokenAccount.amount);
    expect(Number(userTokenAccount.amount)).to.equal(900000);
  });
});