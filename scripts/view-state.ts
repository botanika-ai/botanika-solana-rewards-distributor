import * as anchor from "@coral-xyz/anchor";
import { Program } from "@coral-xyz/anchor";
import { Connection, PublicKey } from "@solana/web3.js";
import * as fs from "fs";
import * as path from "path";

async function main() {
  try {
    const configPath = path.join(__dirname, "config.json");
    if (!fs.existsSync(configPath)) {
      throw new Error("config.json not found!");
    }
    const config = JSON.parse(fs.readFileSync(configPath, "utf-8"));
    const rewardDistributorPda = new PublicKey(config.REWARD_DISTRIBUTOR_PDA);

    // Setup Connection and read-only Provider
    const connection = new Connection("https://api.devnet.solana.com", "confirmed");
    const provider = new anchor.AnchorProvider(
      connection, 
      new anchor.Wallet(anchor.web3.Keypair.generate()), 
      { commitment: "confirmed" }
    );

    // Load Program IDL
    const idlPath = path.resolve(__dirname, "../target/idl/botanika_solana_rewards_distributor.json");
    const idl = JSON.parse(fs.readFileSync(idlPath, "utf-8"));
    const program: any = new Program(idl as any, provider);

    // Fetch account state
    console.log(`Fetching RewardDistributor state from PDA: ${rewardDistributorPda.toBase58()}...`);
    const state = await program.account.rewardDistributor.fetch(rewardDistributorPda);

    console.log("\n----------------------------------------");
    console.log("REWARD DISTRIBUTOR STATE ON-CHAIN:");
    console.log(`Authority:        ${state.authority.toBase58()}`);
    console.log(`Reward Mint:      ${state.rewardMint.toBase58()}`);
    console.log(`Token Vault:      ${state.tokenVault.toBase58()}`);
    console.log(`Current Root Hex: 0x${Buffer.from(state.currentRoot).toString("hex")}`);
    console.log(`Current Root Arr: [${state.currentRoot.join(", ")}]`);
    console.log(`Root Version:     ${state.rootVersion.toString()}`);
    console.log(`Is Paused:        ${state.isPaused}`);
    console.log(`Total Distributed: ${state.totalDistributed.toString()}`);
    console.log("----------------------------------------\n");

  } catch (error) {
    console.error("Error fetching state:", error);
    process.exit(1);
  }
}

main();
