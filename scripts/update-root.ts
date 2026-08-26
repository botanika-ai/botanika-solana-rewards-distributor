import * as anchor from "@coral-xyz/anchor";
import { Program } from "@coral-xyz/anchor";
import { Connection, Keypair, PublicKey } from "@solana/web3.js";
import { keccak_256 } from "@noble/hashes/sha3";
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

    const nextEpochId = new anchor.BN(merkleData.NEXT_EPOCH_ID ?? 1);
    const leafCount = merkleData.LEAF_COUNT ?? 0;
    const totalLiability = new anchor.BN(merkleData.TOTAL_LIABILITY ?? 0);

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

    // 5. Derive PDAs
    const [rewardDistributorPda] = PublicKey.findProgramAddressSync(
      [Buffer.from("reward_distributor")],
      programId
    );
    const epochIdBuffer = Buffer.alloc(8);
    epochIdBuffer.writeBigUInt64LE(BigInt(nextEpochId.toString()));
    const [settlementPda] = PublicKey.findProgramAddressSync(
      [Buffer.from("settlement"), epochIdBuffer],
      programId
    );

    // 6. Settlement metadata (P0-RWD-03) — binds this root to the off-chain
    // proof epoch / reward-ledger snapshot it was computed from. epoch_from /
    // epoch_to and proof_commitment should ultimately come from the proof
    // service; canonical_ledger_hash / policy_hash are placeholders here
    // (hash of the reward list + a fixed policy tag) until reward-service
    // wires in real values from its config/ledger.
    const canonicalLedgerHash = Buffer.from(
      keccak_256(JSON.stringify(merkleData.REWARDS ?? []))
    );
    const proofCommitment = Buffer.from(
      keccak_256(Buffer.concat(newRoot.length ? [Buffer.from(newRoot)] : [Buffer.alloc(32)]))
    );
    const policyHash = Buffer.from(keccak_256("BOTANIKA_REWARD_POLICY_PLACEHOLDER_V1"));

    const settlement = {
      epochFrom: nextEpochId,
      epochTo: nextEpochId,
      proofCommitment: Array.from(proofCommitment),
      policyHash: Array.from(policyHash),
      canonicalLedgerHash: Array.from(canonicalLedgerHash),
      revisionNo: 0,
      leafCount,
      totalLiability,
    };

    // 7. Execute update_root transaction
    console.log("Sending update_root transaction...");
    const tx = await program.methods
      .updateRoot(newRoot, settlement)
      .accounts({
        rewardDistributor: rewardDistributorPda,
        settlement: settlementPda,
        rootAuthority: walletKeypair.publicKey,
        systemProgram: anchor.web3.SystemProgram.programId,
      })
      .rpc();

    // 8. Fetch the updated state to verify
    console.log("Fetching updated on-chain account state...");
    const accountState = await program.account.rewardDistributor.fetch(rewardDistributorPda);

    console.log("----------------------------------------");
    console.log(`UPDATE_ROOT_TX = ${tx}`);
    console.log(`CURRENT_ROOT = [${accountState.currentRoot.join(", ")}]`);
    console.log(`EPOCH_ID = ${accountState.epochId.toString()}`);
    console.log(`SETTLEMENT_PDA = ${settlementPda.toBase58()}`);
    console.log("----------------------------------------");

    // Save details to config
    config.UPDATE_ROOT_TX = tx;
    config.CURRENT_ROOT = accountState.currentRoot;
    config.EPOCH_ID = accountState.epochId.toNumber();
    config.SETTLEMENT_PDA = settlementPda.toBase58();
    fs.writeFileSync(configPath, JSON.stringify(config, null, 2));

  } catch (error) {
    console.error("Error updating Merkle root:", error);
    process.exit(1);
  }
}

main();
