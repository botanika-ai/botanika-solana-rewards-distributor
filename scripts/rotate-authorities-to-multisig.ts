import * as anchor from "@coral-xyz/anchor";
import { Program } from "@coral-xyz/anchor";
import { Connection, Keypair, PublicKey } from "@solana/web3.js";
import * as fs from "fs";
import * as path from "path";
import * as os from "os";
import { resolveClusterUrl } from "./utils";

/**
 * Rotates `admin_authority`, `pause_authority`, and `treasury_authority` on
 * the live `RewardDistributor` from single EOA keypairs to multisig
 * addresses (PAD Developer Commentary P0-RWD-02: "Do not concentrate all
 * roles in one personal hot wallet... use multisig for production upgrade
 * and vault authority"). `root_authority`/`finalize_claim_authority` are
 * deliberately NOT touched here -- in the target architecture they are
 * already escrow PDAs driven only by Magic Actions from
 * botanika-magicblock-contracts (see update_root.rs/finalize_claim.rs doc
 * comments), not human keys, so multisig doesn't apply to them.
 * `payout_authority` (batch_payout, PAD section 11.3 cold fallback only) is
 * also left out here; rotate it the same way with a fourth `setAuthority`
 * call if/when desired.
 *
 * PREREQUISITE this script does NOT do: creating the multisig itself. Use
 * an audited multisig program (e.g. Squads Protocol, https://app.squads.so)
 * -- add the real committee member pubkeys and pick a threshold (M-of-N)
 * there. Squads (and similar) sign on-chain as a derived VAULT PDA, not the
 * multisig account itself -- copy that vault address into config.json.
 *
 * config.json additions required:
 *   "TREASURY_MULTISIG_VAULT": "<pubkey>",
 *   "PAUSE_MULTISIG_VAULT": "<pubkey>",
 *   "ADMIN_MULTISIG_VAULT": "<pubkey>"
 * (the same address may be reused for all three if one committee governs
 * everything; a separate, smaller/faster committee for Pause is common so
 * an incident can be paused quickly without waiting on the full treasury
 * threshold).
 *
 * Order matters and is fixed below: Treasury and Pause rotate first, while
 * the CURRENT admin_authority (a single EOA) can still sign `set_authority`
 * calls. Admin rotates LAST -- once that succeeds, the EOA permanently
 * loses the ability to call `set_authority` again; only the new multisig
 * can from then on. This script reads back on-chain state after
 * Treasury/Pause and aborts before the irreversible Admin step if either
 * doesn't match what was just set.
 */
async function main() {
  const configPath = path.join(__dirname, "config.json");
  if (!fs.existsSync(configPath)) throw new Error("config.json not found!");
  const config = JSON.parse(fs.readFileSync(configPath, "utf-8"));

  const treasuryVault = new PublicKey(requireConfig(config, "TREASURY_MULTISIG_VAULT"));
  const pauseVault = new PublicKey(requireConfig(config, "PAUSE_MULTISIG_VAULT"));
  const adminVault = new PublicKey(requireConfig(config, "ADMIN_MULTISIG_VAULT"));
  const rewardDistributorPda = new PublicKey(requireConfig(config, "REWARD_DISTRIBUTOR_PDA"));
  const clusterUrl = config.CLUSTER_URL || resolveClusterUrl();

  const idlPath = path.resolve(__dirname, "../target/idl/botanika_solana_rewards_distributor.json");
  if (!fs.existsSync(idlPath)) throw new Error(`IDL file not found at ${idlPath} -- run \`anchor build\` first.`);
  const idl = JSON.parse(fs.readFileSync(idlPath, "utf-8"));
  const programId = new PublicKey(config.PROGRAM_ID || idl.address);
  idl.address = programId.toBase58();

  const walletPath = config.ADMIN_KEYPAIR_PATH
    ? path.resolve(config.ADMIN_KEYPAIR_PATH)
    : path.resolve(os.homedir(), ".config/solana/id.json");
  const adminKeypair = Keypair.fromSecretKey(Uint8Array.from(JSON.parse(fs.readFileSync(walletPath, "utf-8"))));

  const connection = new Connection(clusterUrl, "confirmed");
  const provider = new anchor.AnchorProvider(connection, new anchor.Wallet(adminKeypair), { commitment: "confirmed" });
  anchor.setProvider(provider);
  const program: any = new Program(idl as any, provider);

  const before = await program.account.rewardDistributor.fetch(rewardDistributorPda);
  console.log(`Loaded admin keypair: ${adminKeypair.publicKey.toBase58()}`);
  console.log(`On-chain admin_authority: ${before.adminAuthority.toBase58()}`);
  if (!before.adminAuthority.equals(adminKeypair.publicKey)) {
    throw new Error(
      `Loaded keypair is not the current on-chain admin_authority -- set "ADMIN_KEYPAIR_PATH" in ` +
        `config.json to the key that matches ${before.adminAuthority.toBase58()}.`,
    );
  }

  console.log(`\n[1/3] Rotating treasury_authority -> ${treasuryVault.toBase58()}`);
  await setAuthority(program, rewardDistributorPda, adminKeypair, { treasury: {} }, treasuryVault);

  console.log(`\n[2/3] Rotating pause_authority -> ${pauseVault.toBase58()}`);
  await setAuthority(program, rewardDistributorPda, adminKeypair, { pause: {} }, pauseVault);

  const afterTwo = await program.account.rewardDistributor.fetch(rewardDistributorPda);
  if (!afterTwo.treasuryAuthority.equals(treasuryVault) || !afterTwo.pauseAuthority.equals(pauseVault)) {
    throw new Error(
      "On-chain read-back after Treasury/Pause rotation does not match the expected vault addresses " +
        "-- aborting before the irreversible Admin rotation. Investigate before retrying.",
    );
  }

  console.log(
    `\n[3/3] Rotating admin_authority -> ${adminVault.toBase58()} ` +
      `(point of no return: ${walletPath} loses all control on this program after this transaction)`,
  );
  await setAuthority(program, rewardDistributorPda, adminKeypair, { admin: {} }, adminVault);

  const after = await program.account.rewardDistributor.fetch(rewardDistributorPda);
  console.log("\n----------------------------------------");
  console.log("REWARD DISTRIBUTOR AUTHORITIES AFTER ROTATION:");
  console.log(`Admin Authority:           ${after.adminAuthority.toBase58()}`);
  console.log(`Root Authority:            ${after.rootAuthority.toBase58()} (unchanged -- Magic Action escrow PDA)`);
  console.log(`Payout Authority:          ${after.payoutAuthority.toBase58()} (unchanged -- rotate separately if desired)`);
  console.log(`Pause Authority:           ${after.pauseAuthority.toBase58()}`);
  console.log(`Treasury Authority:        ${after.treasuryAuthority.toBase58()}`);
  console.log(`Finalize Claim Authority:  ${after.finalizeClaimAuthority.toBase58()} (unchanged -- Magic Action escrow PDA)`);
  console.log("----------------------------------------");
}

function requireConfig(config: Record<string, unknown>, key: string): string {
  const value = config[key];
  if (typeof value !== "string" || value.length === 0) {
    throw new Error(
      `config.json is missing "${key}". Create the multisig first (e.g. via https://app.squads.so), ` +
        `then add its vault address to config.json under this key.`,
    );
  }
  return value;
}

async function setAuthority(
  program: any,
  rewardDistributorPda: PublicKey,
  adminKeypair: Keypair,
  role: { admin: {} } | { pause: {} } | { treasury: {} },
  newAuthority: PublicKey,
): Promise<void> {
  const tx = await program.methods
    .setAuthority(role, newAuthority)
    .accounts({ rewardDistributor: rewardDistributorPda, adminAuthority: adminKeypair.publicKey })
    .signers([adminKeypair])
    .rpc();
  console.log(`  tx: ${tx}`);
}

main().catch((error) => {
  console.error("\nRotation failed:", error);
  process.exit(1);
});
