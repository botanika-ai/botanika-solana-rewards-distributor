import { Connection, Keypair, PublicKey } from "@solana/web3.js";
import { getAccount, transfer } from "@solana/spl-token";
import * as fs from "fs";
import * as path from "path";
import * as os from "os";

async function main() {
  console.log("Transferring tokens to vault...");
  try {
    // 1. Load config
    const configPath = path.join(__dirname, "config.json");
    if (!fs.existsSync(configPath)) {
      throw new Error("config.json not found! Run create-token.ts first.");
    }
    const config = JSON.parse(fs.readFileSync(configPath, "utf-8"));
    const adminAta = new PublicKey(config.ADMIN_TOKEN_ACCOUNT);
    const tokenVault = new PublicKey(config.TOKEN_VAULT);

    // 2. Load wallet
    const walletPath = path.resolve(os.homedir(), ".config/solana/id.json");
    const walletKeypair = Keypair.fromSecretKey(
      Uint8Array.from(JSON.parse(fs.readFileSync(walletPath, "utf-8")))
    );

    // 3. Connection
    const connection = new Connection("https://api.devnet.solana.com", "confirmed");

    // 4. Transfer tokens (e.g. 500,000 tokens)
    const amount = 500000;
    // Decimals is 9, so amount in raw units is 500000 * 10^9
    const rawAmount = BigInt(amount) * BigInt(1_000_000_000);

    console.log(`Sending transfer of ${amount} tokens to vault (${tokenVault.toBase58()})...`);
    const txSig = await transfer(
      connection,
      walletKeypair, // payer
      adminAta, // source
      tokenVault, // destination
      walletKeypair, // owner of source account
      rawAmount // amount
    );

    // 5. Query vault balance to verify
    console.log("Verifying vault balance...");
    const vaultAccountInfo = await getAccount(connection, tokenVault);
    const vaultBalance = Number(vaultAccountInfo.amount) / 1_000_000_000;

    console.log("----------------------------------------");
    console.log(`VAULT_BALANCE = ${vaultBalance}`);
    console.log(`TRANSFER_TX = ${txSig}`);
    console.log("----------------------------------------");

    // Save balance to config
    config.VAULT_BALANCE = vaultBalance;
    config.TRANSFER_TX = txSig;
    fs.writeFileSync(configPath, JSON.stringify(config, null, 2));

  } catch (error) {
    console.error("Error transferring tokens to vault:", error);
    process.exit(1);
  }
}

main();
