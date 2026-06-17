import { execSync } from "child_process";
import * as fs from "fs";
import * as path from "path";

async function main() {
  console.log("Creating SPL Token on Solana Devnet...");

  try {
    // 1. Create token mint
    console.log("Running: spl-token create-token --url devnet");
    const createTokenOut = execSync("spl-token create-token --url devnet").toString();
    console.log(createTokenOut);
    
    // Extract Token Address
    const tokenMintMatch = createTokenOut.match(/Creating token\s+([A-Za-z0-9]{32,44})/);
    if (!tokenMintMatch) {
      throw new Error("Failed to parse token mint address from CLI output");
    }
    const tokenMint = tokenMintMatch[1];

    // 2. Create admin token account
    console.log(`Running: spl-token create-account ${tokenMint} --url devnet`);
    const createAccountOut = execSync(`spl-token create-account ${tokenMint} --url devnet`).toString();
    console.log(createAccountOut);

    // Extract Associated Token Account (ATA) Address
    const ataMatch = createAccountOut.match(/Creating account\s+([A-Za-z0-9]{32,44})/);
    if (!ataMatch) {
      throw new Error("Failed to parse ATA address from CLI output");
    }
    const adminAta = ataMatch[1];

    // 3. Mint initial supply (e.g., 1,000,000 tokens)
    const initialSupply = 1000000;
    console.log(`Running: spl-token mint ${tokenMint} ${initialSupply} --url devnet`);
    const mintOut = execSync(`spl-token mint ${tokenMint} ${initialSupply} --url devnet`).toString();
    console.log(mintOut);

    // 4. Output the result in the requested format
    console.log("----------------------------------------");
    console.log(`TOKEN_MINT = ${tokenMint}`);
    console.log(`ADMIN_TOKEN_ACCOUNT = ${adminAta}`);
    console.log(`INITIAL_SUPPLY = ${initialSupply}`);
    console.log("----------------------------------------");

    // 5. Save details to config.json
    const configPath = path.join(__dirname, "config.json");
    const config = {
      TOKEN_MINT: tokenMint,
      ADMIN_TOKEN_ACCOUNT: adminAta,
      INITIAL_SUPPLY: initialSupply
    };
    fs.writeFileSync(configPath, JSON.stringify(config, null, 2));
    console.log(`Config saved to ${configPath}`);

  } catch (error) {
    console.error("Error setting up SPL token:", error);
    process.exit(1);
  }
}

main();
