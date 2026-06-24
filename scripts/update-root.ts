import * as anchor from "@coral-xyz/anchor";
import { Program } from "@coral-xyz/anchor";
import { Connection, Keypair, PublicKey } from "@solana/web3.js";
import * as fs from "fs";
import * as path from "path";
import * as os from "os";
import { resolveClusterUrl } from "./utils";

async function main() {
  console.log("Updating Merkle Root on-chain...");

  try {
    // 1. Load config and Merkle output
    const configPath = path.join(__dirname, "config.json");
    if (!fs.existsSync(configPath)) {
      throw new Error("config.json not found! Run create-token.ts first.");
    }
    const config = JSON.parse(fs.readFileSync(configPath, "utf-8"));

    const merklePath = path.join(__dirname, "merkle-output.json");
    if (!fs.existsSync(merklePath)) {
      throw new Error("merkle-output.json not found! Run generate-merkle.ts first.");
    }
    const merkleData = JSON.parse(fs.readFileSync(merklePath, "utf-8"));
    const newRoot: number[] = merkleData.MERKLE_ROOT;

    if (newRoot.length !== 32) {
      throw new Error("Invalid Merkle Root length in merkle-output.json");
    }

    // 2. Load wallet
    const walletPath = path.resolve(os.homedir(), ".config/solana/id.json");
    const walletSecret = JSON.parse(fs.readFileSync(walletPath, "utf-8"));
    const walletKeypair = Keypair.fromSecretKey(Uint8Array.from(walletSecret));

    // 3. Setup Connection and Provider
    const clusterUrl = config.CLUSTER_URL || resolveClusterUrl();
    const connection = new Connection(clusterUrl, "confirmed");
    const wallet = new anchor.Wallet(walletKeypair);
    const provider = new anchor.AnchorProvider(connection, wallet, {
      commitment: "confirmed",
    });
    anchor.setProvider(provider);

    // 4. Load Program
    const idlPath = path.resolve(__dirname, "../target/idl/botanika_solana_rewards_distributor.json");
    const idl = JSON.parse(fs.readFileSync(idlPath, "utf-8"));
    const programId = new PublicKey(config.PROGRAM_ID || idl.address || "4HfLqCMnNW4EPrLiDkwcEewCBaNMVWkKhShKe5rRwB8o");
    idl.address = programId.toBase58();
    const program: any = new Program(idl as any, provider);

    // 5. Derive PDA
    const [rewardDistributorPda] = PublicKey.findProgramAddressSync(
      [Buffer.from("reward_distributor")],
      programId
    );

    // 6. Execute update_root transaction
    console.log("Sending update_root transaction...");
    const tx = await program.methods
      .updateRoot(newRoot)
      .accounts({
        rewardDistributor: rewardDistributorPda,
        authority: walletKeypair.publicKey,
      })
      .rpc();

    // 7. Fetch the updated state to verify
    console.log("Fetching updated on-chain account state...");
    const accountState = await program.account.rewardDistributor.fetch(rewardDistributorPda);
    
    console.log("----------------------------------------");
    console.log(`UPDATE_ROOT_TX = ${tx}`);
    console.log(`CURRENT_ROOT = [${accountState.currentRoot.join(", ")}]`);
    console.log(`EPOCH_ID = ${accountState.epochId.toString()}`);
    console.log("----------------------------------------");

    // Save details to config
    config.UPDATE_ROOT_TX = tx;
    config.CURRENT_ROOT = accountState.currentRoot;
    config.EPOCH_ID = accountState.epochId.toNumber();
    fs.writeFileSync(configPath, JSON.stringify(config, null, 2));

  } catch (error) {
    console.error("Error updating Merkle root:", error);
    process.exit(1);
  }
}

main();
