import { execSync } from "child_process";
import * as path from "path";
import * as fs from "fs";

async function main() {
  console.log("=================================================================");
  console.log("🚀 STARTING AUTOMATED BUILD, DEPLOY & DISTRIBUTOR SETUP");
  console.log("=================================================================");

  try {
    // Step 0.1: Build program (to ensure latest code and IDL)
    console.log("\n👉 [Step 0.1] Building Anchor program...");
    execSync("anchor build", { stdio: "inherit" });

    // Step 0.2: Deploy program
    console.log("\n👉 [Step 0.2] Deploying program to Solana...");
    execSync("anchor deploy -- --max-sign-attempts 100 --with-compute-unit-price 50000", { stdio: "inherit" });

    // Step 0.3: Publish IDL on-chain
    console.log("\n👉 [Step 0.3] Publishing / Initializing IDL on-chain...");
    const idlPath = path.resolve(__dirname, "../target/idl/botanika_solana_rewards_distributor.json");
    if (fs.existsSync(idlPath)) {
      const idl = JSON.parse(fs.readFileSync(idlPath, "utf-8"));
      const programId = idl.address;
      if (programId) {
        console.log(`Program ID found in IDL: ${programId}`);
        try {
          console.log(`Running: anchor idl init --filepath "${idlPath}" ${programId}`);
          execSync(`anchor idl init --filepath "${idlPath}" ${programId}`, { stdio: "inherit" });
          console.log("IDL initialized on-chain successfully!");
        } catch (idlErr) {
          console.log("anchor idl init failed (maybe already initialized). Trying anchor idl upgrade...");
          try {
            console.log(`Running: anchor idl upgrade --filepath "${idlPath}" ${programId}`);
            execSync(`anchor idl upgrade --filepath "${idlPath}" ${programId}`, { stdio: "inherit" });
            console.log("IDL upgraded on-chain successfully!");
          } catch (upgradeErr) {
            console.warn("Warning: Failed to publish IDL on-chain. Skipping to setup scripts. Error:", upgradeErr);
          }
        }
      } else {
        console.warn("Warning: No program address field found in IDL file.");
      }
    } else {
      console.warn(`Warning: IDL file not found at ${idlPath}. Cannot publish IDL on-chain.`);
    }

    // Step 1: Create Token Mint, Associated Account, and Mint Initial Supply
    console.log("\n👉 [Step 1] Creating Token Mint & Minting Supply...");
    const createTokenScript = path.join(__dirname, "create-token.ts");
    execSync(`npx ts-node "${createTokenScript}"`, { stdio: "inherit" });

    // Step 2: Initialize the Rewards Distributor Program on-chain
    console.log("\n👉 [Step 2] Initializing Reward Distributor on Solana...");
    const initializeScript = path.join(__dirname, "initialize.ts");
    execSync(`npx ts-node "${initializeScript}"`, { stdio: "inherit" });

    // Step 3: Transfer Reward Tokens to the Program's Vault
    console.log("\n👉 [Step 3] Transferring Initial Tokens to Vault...");
    const transferScript = path.join(__dirname, "transfer-to-vault.ts");
    execSync(`npx ts-node "${transferScript}"`, { stdio: "inherit" });

    console.log("\n=================================================================");
    console.log("✅ AUTOMATED SETUP COMPLETED SUCCESSFULLY!");
    console.log("Program built, deployed, IDL published, initialized & funded.");
    console.log("Please run step 4 & 5 to generate and upload your Merkle Root.");
    console.log("=================================================================");
  } catch (error) {
    console.error("\n❌ Setup failed during execution:", error);
    process.exit(1);
  }
}

main();
