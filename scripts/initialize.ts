import * as anchor from "@coral-xyz/anchor";
import { Program } from "@coral-xyz/anchor";
import { Connection, Keypair, PublicKey } from "@solana/web3.js";
import { TOKEN_PROGRAM_ID } from "@solana/spl-token";
import * as fs from "fs";
import * as path from "path";
import * as os from "os";

async function main() {
  console.log("Initializing Reward Distributor on Solana Devnet...");

  try {
    // 1. Load config
    const configPath = path.join(__dirname, "config.json");
    if (!fs.existsSync(configPath)) {
      throw new Error("config.json not found! Run create-token.ts first.");
    }
    const config = JSON.parse(fs.readFileSync(configPath, "utf-8"));
    const tokenMint = new PublicKey(config.TOKEN_MINT);

    // 2. Load wallet
    const walletPath = path.resolve(os.homedir(), ".config/solana/id.json");
    const walletSecret = JSON.parse(fs.readFileSync(walletPath, "utf-8"));
    const walletKeypair = Keypair.fromSecretKey(Uint8Array.from(walletSecret));

    // 3. Setup Connection and Provider
    const connection = new Connection("https://api.devnet.solana.com", "confirmed");
    const wallet = new anchor.Wallet(walletKeypair);
    const provider = new anchor.AnchorProvider(connection, wallet, {
      commitment: "confirmed",
    });
    anchor.setProvider(provider);

    // 4. Load Program
    const programId = new PublicKey("J9gu41htkjXKAJxrmEfciYDPxPdP7xg8BV7sgNtaXdZs");
    const idlPath = path.resolve(__dirname, "../target/idl/botanika_solana_rewards_distributor.json");
    if (!fs.existsSync(idlPath)) {
      throw new Error(`IDL file not found at ${idlPath}`);
    }
    const idl = JSON.parse(fs.readFileSync(idlPath, "utf-8"));
    const program: any = new Program(idl as any, provider);

    // 5. Derive PDAs
    const [rewardDistributorPda] = PublicKey.findProgramAddressSync(
      [Buffer.from("reward_distributor")],
      programId
    );

    const [tokenVaultPda] = PublicKey.findProgramAddressSync(
      [Buffer.from("vault"), tokenMint.toBuffer()],
      programId
    );

    console.log(`Deriving PDAs...`);
    console.log(`reward_distributor: ${rewardDistributorPda.toBase58()}`);
    console.log(`token_vault: ${tokenVaultPda.toBase58()}`);

    // 6. Call initialize instruction
    console.log("Sending initialize transaction...");
    const tx = await program.methods
      .initialize(walletKeypair.publicKey)
      .accounts({
        rewardDistributor: rewardDistributorPda,
        rewardMint: tokenMint,
        tokenVault: tokenVaultPda,
        payer: walletKeypair.publicKey,
        tokenProgram: TOKEN_PROGRAM_ID,
        systemProgram: anchor.web3.SystemProgram.programId,
      })
      .rpc();

    // 7. Output expected formatting
    console.log("----------------------------------------");
    console.log(`REWARD_DISTRIBUTOR_PDA = ${rewardDistributorPda.toBase58()}`);
    console.log(`TOKEN_VAULT = ${tokenVaultPda.toBase58()}`);
    console.log(`TX_SIGNATURE = ${tx}`);
    console.log("----------------------------------------");

    // Save derived PDAs to config
    config.REWARD_DISTRIBUTOR_PDA = rewardDistributorPda.toBase58();
    config.TOKEN_VAULT = tokenVaultPda.toBase58();
    fs.writeFileSync(configPath, JSON.stringify(config, null, 2));
    console.log(`Updated config saved to ${configPath}`);

  } catch (error) {
    console.error("Error initializing reward distributor:", error);
    process.exit(1);
  }
}

main();
