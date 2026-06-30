import { execSync } from "child_process";
import * as fs from "fs";
import * as path from "path";
import { resolveClusterUrl } from "./utils";

async function main() {
  const clusterUrl = resolveClusterUrl();
  console.log(`Creating SPL Token on Solana (${clusterUrl})...`);

  try {
    // Check if token already exists in config.json (useful for Mainnet)
    const configPath = path.join(__dirname, "config.json");
    let existingConfig: any = {};
    if (fs.existsSync(configPath)) {
      try {
        existingConfig = JSON.parse(fs.readFileSync(configPath, "utf-8"));
      } catch (e) {}
    }

    if (existingConfig.TOKEN_MINT && existingConfig.ADMIN_TOKEN_ACCOUNT) {
      console.log(`Using existing SPL Token Mint from config.json: ${existingConfig.TOKEN_MINT}`);
      console.log(`Using existing Admin Token Account: ${existingConfig.ADMIN_TOKEN_ACCOUNT}`);
      existingConfig.CLUSTER_URL = existingConfig.CLUSTER_URL || clusterUrl;
      fs.writeFileSync(configPath, JSON.stringify(existingConfig, null, 2));
      return;
    }

    // 1. Create token mint (decimals must be 9 for rewards distributor)
    console.log(`Running: spl-token create-token --decimals 9 --url ${clusterUrl}`);
    const createTokenOut = execSync(`spl-token create-token --decimals 9 --url ${clusterUrl}`).toString();
    console.log(createTokenOut);
    
    // Extract Token Address
    const tokenMintMatch = createTokenOut.match(/Creating token\s+([A-Za-z0-9]{32,44})/);
    if (!tokenMintMatch) {
      throw new Error("Failed to parse token mint address from CLI output");
    }
    const tokenMint = tokenMintMatch[1];

    // 2. Create admin token account
    console.log(`Running: spl-token create-account ${tokenMint} --url ${clusterUrl}`);
    const createAccountOut = execSync(`spl-token create-account ${tokenMint} --url ${clusterUrl}`).toString();
    console.log(createAccountOut);

    // Extract Associated Token Account (ATA) Address
    const ataMatch = createAccountOut.match(/Creating account\s+([A-Za-z0-9]{32,44})/);
    if (!ataMatch) {
      throw new Error("Failed to parse ATA address from CLI output");
    }
    const adminAta = ataMatch[1];

    // 3. Mint initial supply (e.g., 1,000,000 tokens)
    const initialSupply = 1000000;
    console.log(`Running: spl-token mint ${tokenMint} ${initialSupply} --url ${clusterUrl}`);
    const mintOut = execSync(`spl-token mint ${tokenMint} ${initialSupply} --url ${clusterUrl}`).toString();
    console.log(mintOut);

    // 4. Output the result in the requested format
    console.log("----------------------------------------");
    console.log(`TOKEN_MINT = ${tokenMint}`);
    console.log(`ADMIN_TOKEN_ACCOUNT = ${adminAta}`);
    console.log(`INITIAL_SUPPLY = ${initialSupply}`);
    console.log("----------------------------------------");

    // 5. Save details to config.json (reuse configPath from above)
    const config = {
      CLUSTER_URL: clusterUrl,
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
