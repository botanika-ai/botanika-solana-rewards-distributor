import * as anchor from "@coral-xyz/anchor";
import { Program } from "@coral-xyz/anchor";
import { Connection, Keypair, PublicKey } from "@solana/web3.js";
import { TOKEN_PROGRAM_ID } from "@solana/spl-token";
import * as fs from "fs";
import * as path from "path";
import * as os from "os";
import { resolveClusterUrl } from "./utils";

/**
 * Production `initialize` for a FRESH `RewardDistributor` deployment --
 * every authority is multisig-controlled (or the correct escrow PDA) from
 * the very first transaction, so there is never a window where one EOA
 * controls everything (PAD Developer Commentary P0-RWD-02). Cannot be run
 * against an already-initialized program -- `initialize` uses `init`, so
 * re-running this against the existing testnet distributor
 * (REWARD_DISTRIBUTOR_PDA already in config.json) will fail with "account
 * already in use". Use `rotate-authorities-to-multisig.ts` for that one.
 *
 * Six authority roles, deliberately NOT all multisig:
 *
 *   admin_authority    -> ADMIN_MULTISIG_VAULT    (human multisig)
 *   pause_authority    -> PAUSE_MULTISIG_VAULT     (human multisig)
 *   treasury_authority -> TREASURY_MULTISIG_VAULT  (human multisig)
 *   payout_authority   -> PAYOUT_MULTISIG_VAULT    (human multisig --
 *                          batch_payout is a PAD section 11.3 cold-fallback
 *                          path now, but still moves real funds by hand)
 *   root_authority     -> escrow PDA (see below)   -- NOT multisig
 *   finalize_claim_authority -> same escrow PDA    -- NOT multisig
 *
 * root_authority/finalize_claim_authority are NOT human-signable roles at
 * all: `update_root`/`finalize_claim` (see their own doc comments) accept
 * an `escrow_auth: UncheckedAccount` checked only by `address ==
 * reward_distributor.root_authority` -- it is never a `Signer`. The real
 * authorization is a *separate* `escrow` account that only
 * MagicBlock's delegation program can sign for via `invoke_signed` when it
 * dispatches botanika-magicblock-contracts's `commit_epoch_state`/
 * `commit_reward_balance` Magic Action. `escrow_auth`'s job is to whitelist
 * *which program's* Magic Action may call in -- the only valid value is
 * this fixed PDA:
 *
 *   PublicKey.findProgramAddressSync([Buffer.from("escrow_auth")], MAGICBLOCK_PROGRAM_ID)
 *
 * Setting these two to a human multisig instead would make
 * `update_root`/`finalize_claim` permanently uncallable (every real Magic
 * Action call would fail the address check) -- not a security improvement,
 * a hard outage. This script computes and uses the correct PDA
 * automatically from `MAGICBLOCK_PROGRAM_ID` in config.json.
 *
 * config.json required keys:
 *   TOKEN_MINT                 -- existing reward_mint (see create-token.ts)
 *   MAGICBLOCK_PROGRAM_ID      -- botanika-magicblock-contracts's deployed program id
 *   ADMIN_MULTISIG_VAULT
 *   PAUSE_MULTISIG_VAULT
 *   TREASURY_MULTISIG_VAULT
 *   PAYOUT_MULTISIG_VAULT
 *   DEPLOY_KEYPAIR_PATH        -- optional, defaults to ~/.config/solana/id.json (pays rent only, holds no authority)
 */
async function main() {
  const configPath = path.join(__dirname, "config.json");
  if (!fs.existsSync(configPath)) throw new Error("config.json not found!");
  const config = JSON.parse(fs.readFileSync(configPath, "utf-8"));

  const tokenMint = new PublicKey(requireConfig(config, "TOKEN_MINT"));
  const magicblockProgramId = new PublicKey(requireConfig(config, "MAGICBLOCK_PROGRAM_ID"));
  const adminVault = new PublicKey(requireConfig(config, "ADMIN_MULTISIG_VAULT"));
  const pauseVault = new PublicKey(requireConfig(config, "PAUSE_MULTISIG_VAULT"));
  const treasuryVault = new PublicKey(requireConfig(config, "TREASURY_MULTISIG_VAULT"));
  const payoutVault = new PublicKey(requireConfig(config, "PAYOUT_MULTISIG_VAULT"));

  const [escrowAuthPda] = PublicKey.findProgramAddressSync([Buffer.from("escrow_auth")], magicblockProgramId);
  console.log(`Derived escrow_auth PDA (root_authority / finalize_claim_authority): ${escrowAuthPda.toBase58()}`);

  const clusterUrl = config.CLUSTER_URL || resolveClusterUrl();
  const idlPath = path.resolve(__dirname, "../target/idl/botanika_solana_rewards_distributor.json");
  if (!fs.existsSync(idlPath)) throw new Error(`IDL file not found at ${idlPath} -- run \`anchor build\` first.`);
  const idl = JSON.parse(fs.readFileSync(idlPath, "utf-8"));
  const programId = new PublicKey(config.PROGRAM_ID || idl.address);
  idl.address = programId.toBase58();

  const deployKeypairPath = config.DEPLOY_KEYPAIR_PATH
    ? path.resolve(config.DEPLOY_KEYPAIR_PATH)
    : path.resolve(os.homedir(), ".config/solana/id.json");
  const deployKeypair = Keypair.fromSecretKey(Uint8Array.from(JSON.parse(fs.readFileSync(deployKeypairPath, "utf-8"))));

  // Safety net: this keypair only ever pays rent for this transaction. If
  // it accidentally matches any authority below, P0-RWD-02 is right back
  // where it started -- fail loudly instead of silently bootstrapping a
  // single point of failure again.
  const vaults = { adminVault, pauseVault, treasuryVault, payoutVault, escrowAuthPda };
  for (const [name, vault] of Object.entries(vaults)) {
    if (vault.equals(deployKeypair.publicKey)) {
      throw new Error(
        `${name} is set to the deploy keypair's own pubkey (${deployKeypair.publicKey.toBase58()}) -- ` +
          "that defeats the entire point of this script. Fix config.json.",
      );
    }
  }

  const connection = new Connection(clusterUrl, "confirmed");
  const provider = new anchor.AnchorProvider(connection, new anchor.Wallet(deployKeypair), { commitment: "confirmed" });
  anchor.setProvider(provider);
  const program: any = new Program(idl as any, provider);

  const [rewardDistributorPda] = PublicKey.findProgramAddressSync([Buffer.from("reward_distributor")], programId);
  const tokenVaultKeypair = Keypair.generate();

  console.log("----------------------------------------");
  console.log("AUTHORITIES FOR THIS DEPLOYMENT:");
  console.log(`admin_authority           = ${adminVault.toBase58()} (multisig)`);
  console.log(`pause_authority           = ${pauseVault.toBase58()} (multisig)`);
  console.log(`treasury_authority        = ${treasuryVault.toBase58()} (multisig)`);
  console.log(`payout_authority          = ${payoutVault.toBase58()} (multisig)`);
  console.log(`root_authority            = ${escrowAuthPda.toBase58()} (escrow PDA, NOT multisig)`);
  console.log(`finalize_claim_authority  = ${escrowAuthPda.toBase58()} (escrow PDA, NOT multisig)`);
  console.log("----------------------------------------");

  const authorities = {
    adminAuthority: adminVault,
    rootAuthority: escrowAuthPda,
    payoutAuthority: payoutVault,
    pauseAuthority: pauseVault,
    treasuryAuthority: treasuryVault,
    finalizeClaimAuthority: escrowAuthPda,
  };

  console.log("\nSending initialize transaction...");
  const tx = await program.methods
    .initialize(authorities)
    .accounts({
      rewardDistributor: rewardDistributorPda,
      rewardMint: tokenMint,
      tokenVault: tokenVaultKeypair.publicKey,
      payer: deployKeypair.publicKey,
      tokenProgram: TOKEN_PROGRAM_ID,
      systemProgram: anchor.web3.SystemProgram.programId,
    })
    .signers([tokenVaultKeypair])
    .rpc();

  console.log("----------------------------------------");
  console.log(`REWARD_DISTRIBUTOR_PDA = ${rewardDistributorPda.toBase58()}`);
  console.log(`TOKEN_VAULT = ${tokenVaultKeypair.publicKey.toBase58()}`);
  console.log(`TX_SIGNATURE = ${tx}`);
  console.log("----------------------------------------");
  console.log(
    "\nNo authority here is the deploy keypair. There is no rotation step to " +
      "run afterward -- this deployment never had a single-key window.",
  );

  config.REWARD_DISTRIBUTOR_PDA = rewardDistributorPda.toBase58();
  config.TOKEN_VAULT = tokenVaultKeypair.publicKey.toBase58();
  config.TOKEN_VAULT_SECRET = Array.from(tokenVaultKeypair.secretKey);
  config.PROGRAM_ID = programId.toBase58();
  fs.writeFileSync(configPath, JSON.stringify(config, null, 2));
  console.log(`Updated config saved to ${configPath}`);
}

function requireConfig(config: Record<string, unknown>, key: string): string {
  const value = config[key];
  if (typeof value !== "string" || value.length === 0) {
    throw new Error(`config.json is missing "${key}" -- see this script's doc comment for what's required.`);
  }
  return value;
}

main().catch((error) => {
  console.error("\nInitialize failed:", error);
  process.exit(1);
});
