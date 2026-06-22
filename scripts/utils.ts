import * as fs from "fs";
import * as path from "path";

/**
 * Resolves the Solana RPC cluster URL.
 * Order of priority:
 * 1. Environment variable SOLANA_NETWORK (can be a name like 'mainnet-beta' or a custom URL).
 * 2. CLUSTER_URL specified in scripts/config.json.
 * 3. [provider] cluster configured in Anchor.toml.
 * 4. Fallback to default Testnet URL.
 */
export function resolveClusterUrl(): string {
  // 1. Env Var override
  if (process.env.SOLANA_NETWORK) {
    const envNet = process.env.SOLANA_NETWORK.trim();
    if (envNet === "mainnet" || envNet === "mainnet-beta") {
      return "https://api.mainnet-beta.solana.com";
    }
    if (envNet === "devnet") {
      return "https://api.devnet.solana.com";
    }
    if (envNet === "testnet") {
      return "https://api.testnet.solana.com";
    }
    if (envNet === "localnet") {
      return "http://127.0.0.1:8899";
    }
    return envNet;
  }

  // 2. config.json override
  try {
    const configPath = path.join(__dirname, "config.json");
    if (fs.existsSync(configPath)) {
      const config = JSON.parse(fs.readFileSync(configPath, "utf-8"));
      if (config.CLUSTER_URL) return config.CLUSTER_URL;
    }
  } catch (e) {}

  // 3. Anchor.toml parse
  try {
    const tomlPath = path.resolve(__dirname, "../Anchor.toml");
    if (fs.existsSync(tomlPath)) {
      const tomlContent = fs.readFileSync(tomlPath, "utf-8");
      const match = tomlContent.match(/cluster\s*=\s*["']([^"']+)["']/);
      if (match) {
        const cluster = match[1].trim();
        if (cluster.startsWith("http://") || cluster.startsWith("https://")) {
          return cluster;
        }
        if (cluster === "mainnet" || cluster === "mainnet-beta") {
          return "https://api.mainnet-beta.solana.com";
        }
        if (cluster === "devnet") {
          return "https://api.devnet.solana.com";
        }
        if (cluster === "testnet") {
          return "https://api.testnet.solana.com";
        }
        if (cluster === "localnet") {
          return "http://127.0.0.1:8899";
        }
      }
    }
  } catch (e) {}

  // 4. Default to testnet
  return "https://api.testnet.solana.com";
}
