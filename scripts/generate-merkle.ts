import * as anchor from "@coral-xyz/anchor";
import { Program } from "@coral-xyz/anchor";
import { Connection, Keypair, PublicKey } from "@solana/web3.js";
import { keccak_256 } from "@noble/hashes/sha3";
import * as fs from "fs";
import * as path from "path";
import * as os from "os";
import { resolveClusterUrl } from "./utils";

// Domain separation tag for reward Merkle leaves (P1-RWD-07). Must match
// LEAF_DOMAIN in programs/.../src/instructions/claim_reward.rs exactly.
const LEAF_DOMAIN = Buffer.from("BOTANIKA_REWARD_LEAF_V1", "utf-8");

function hashNodeId(nodeId: string): Buffer {
  return Buffer.from(keccak_256(nodeId));
}

// Leaf hashing matching the Rust contract:
// keccak256(domain || program_id || distributor || reward_mint || epoch_id_le || miner || node_id_hash || amount_le)
function hashLeaf(
  programId: PublicKey,
  distributor: PublicKey,
  rewardMint: PublicKey,
  epochId: bigint,
  miner: PublicKey,
  nodeIdHash: Buffer,
  cumulativeAmount: bigint
): Buffer {
  const epochIdBuffer = Buffer.alloc(8);
  epochIdBuffer.writeBigUInt64LE(epochId);
  const amountBuffer = Buffer.alloc(8);
  amountBuffer.writeBigUInt64LE(cumulativeAmount);
  const data = Buffer.concat([
    LEAF_DOMAIN,
    programId.toBuffer(),
    distributor.toBuffer(),
    rewardMint.toBuffer(),
    epochIdBuffer,
    miner.toBuffer(),
    nodeIdHash,
    amountBuffer,
  ]);
  return Buffer.from(keccak_256(data));
}

// Node hashing: sort and keccak256(left || right)
function hashPair(a: Buffer, b: Buffer): Buffer {
  if (Buffer.compare(a, b) <= 0) {
    return Buffer.from(keccak_256(Buffer.concat([a, b])));
  } else {
    return Buffer.from(keccak_256(Buffer.concat([b, a])));
  }
}

class MerkleTree {
  leaves: Buffer[];
  layers: Buffer[][];

  constructor(leaves: Buffer[]) {
    // Sort leaves to ensure deterministic tree generation regardless of original order
    this.leaves = [...leaves].sort(Buffer.compare);
    this.layers = [this.leaves];

    while (this.layers[this.layers.length - 1].length > 1) {
      this.layers.push(this.getNextLayer(this.layers[this.layers.length - 1]));
    }
  }

  private getNextLayer(elements: Buffer[]): Buffer[] {
    const nextLayer: Buffer[] = [];
    for (let i = 0; i < elements.length; i += 2) {
      if (i + 1 < elements.length) {
        nextLayer.push(hashPair(elements[i], elements[i + 1]));
      } else {
        nextLayer.push(elements[i]);
      }
    }
    return nextLayer;
  }

  getRoot(): Buffer {
    return this.layers[this.layers.length - 1][0] || Buffer.alloc(32);
  }

  getProof(leaf: Buffer): Buffer[] {
    let index = -1;
    for (let i = 0; i < this.leaves.length; i++) {
      if (this.leaves[i].equals(leaf)) {
        index = i;
        break;
      }
    }
    if (index === -1) return [];

    const proof: Buffer[] = [];
    for (let i = 0; i < this.layers.length - 1; i++) {
      const layer = this.layers[i];
      const isRightNode = index % 2 === 1;
      const pairIndex = isRightNode ? index - 1 : index + 1;
      if (pairIndex < layer.length) {
        proof.push(layer[pairIndex]);
      }
      index = Math.floor(index / 2);
    }
    return proof;
  }
}

async function main() {
  console.log("Generating off-chain Merkle Tree...");

  try {
    // 1. Load config to get/set miners
    const configPath = path.join(__dirname, "config.json");
    if (!fs.existsSync(configPath)) {
      throw new Error("config.json not found! Run create-token.ts first.");
    }
    const config = JSON.parse(fs.readFileSync(configPath, "utf-8"));

    // If miners don't exist in config, generate and save them
    if (!config.MINER_1 || !config.MINER_2) {
      console.log("Generating fresh Miner wallets...");
      const miner1Keypair = Keypair.generate();
      const miner2Keypair = Keypair.generate();

      config.MINER_1 = {
        publicKey: miner1Keypair.publicKey.toBase58(),
        secretKey: Array.from(miner1Keypair.secretKey)
      };
      config.MINER_2 = {
        publicKey: miner2Keypair.publicKey.toBase58(),
        secretKey: Array.from(miner2Keypair.secretKey)
      };

      fs.writeFileSync(configPath, JSON.stringify(config, null, 2));
      console.log("Saved fresh miner wallets to config.");
    }

    const wallet1 = config.MINER_1.publicKey;
    const wallet2 = config.MINER_2.publicKey;

    // 2. Resolve on-chain distributor state — the leaf domain binds
    // program_id / distributor / reward_mint / epoch_id (P1-RWD-07), and the
    // epoch_id used must be the one this root will have *after* update-root
    // runs (current epoch_id + 1), since claim_reward reads it post-update.
    const clusterUrl = config.CLUSTER_URL || resolveClusterUrl();
    const connection = new Connection(clusterUrl, "confirmed");

    const idlPath = path.resolve(__dirname, "../target/idl/botanika_solana_rewards_distributor.json");
    const idl = JSON.parse(fs.readFileSync(idlPath, "utf-8"));
    const programId = new PublicKey(config.PROGRAM_ID || idl.address || "4HfLqCMnNW4EPrLiDkwcEewCBaNMVWkKhShKe5rRwB8o");
    idl.address = programId.toBase58();

    const walletPath = path.resolve(os.homedir(), ".config/solana/id.json");
    const walletKeypair = Keypair.fromSecretKey(Uint8Array.from(JSON.parse(fs.readFileSync(walletPath, "utf-8"))));
    const wallet = new anchor.Wallet(walletKeypair);
    const provider = new anchor.AnchorProvider(connection, wallet, { commitment: "confirmed" });
    anchor.setProvider(provider);
    const program: any = new Program(idl as any, provider);

    const [rewardDistributorPda] = PublicKey.findProgramAddressSync(
      [Buffer.from("reward_distributor")],
      programId
    );
    const distributorState = await program.account.rewardDistributor.fetch(rewardDistributorPda);
    const currentEpochId: bigint = BigInt(distributorState.epochId.toString());
    const nextEpochId = currentEpochId + 1n;
    const rewardMint: PublicKey = distributorState.rewardMint;

    console.log(`Current on-chain epoch_id: ${currentEpochId}, leaves will bind to next epoch_id: ${nextEpochId}`);

    // 3. Prepare sample rewards (one leaf per node)
    const rewards = [
      { miner: wallet1, nodeId: "node-001", amount: 1000 },
      { miner: wallet2, nodeId: "node-002", amount: 5000 },
      { miner: wallet1, nodeId: "node-003", amount: 2500 },
    ];

    console.log("Sample Reward List:");
    console.log(JSON.stringify(rewards, null, 2));

    // 4. Compute leaves
    const leaves = rewards.map((reward) => {
      const nodeIdHash = hashNodeId(reward.nodeId);
      return {
        reward,
        nodeIdHash,
        leaf: hashLeaf(
          programId,
          rewardDistributorPda,
          rewardMint,
          nextEpochId,
          new PublicKey(reward.miner),
          nodeIdHash,
          BigInt(reward.amount)
        ),
      };
    });

    // 5. Construct tree
    const tree = new MerkleTree(leaves.map((entry) => entry.leaf));
    const root = tree.getRoot();

    // 6. Compute proofs
    const proofs = leaves.map((entry) => ({
      ...entry,
      proof: tree.getProof(entry.leaf).map((b) => Array.from(b)),
    }));

    // Output root as hex and as 32-byte array
    const rootHex = root.toString("hex");
    const rootArray = Array.from(root);
    const totalLiability = rewards.reduce((sum, r) => sum + r.amount, 0);

    console.log("\n----------------------------------------");
    console.log(`MERKLE_ROOT = [${rootArray.join(", ")}]`);
    console.log(`(Hex: 0x${rootHex})`);
    console.log(`NEXT_EPOCH_ID = ${nextEpochId}`);
    console.log(`LEAF_COUNT = ${rewards.length}`);
    console.log(`TOTAL_LIABILITY = ${totalLiability}`);
    console.log("\nPROOFS = {");
    for (const entry of proofs) {
      const key = `${entry.reward.miner}:${entry.reward.nodeId}`;
      console.log(`  "${key}": [${entry.proof.map((p) => "[" + p.join(", ") + "]").join(", ")}],`);
    }
    console.log("}");
    console.log("----------------------------------------\n");

    // Save proof output
    const merkleOutputPath = path.join(__dirname, "merkle-output.json");
    const outputData = {
      MERKLE_ROOT: rootArray,
      MERKLE_ROOT_HEX: rootHex,
      NEXT_EPOCH_ID: nextEpochId.toString(),
      LEAF_COUNT: rewards.length,
      TOTAL_LIABILITY: totalLiability,
      PROOFS: Object.fromEntries(
        proofs.map((entry) => [
          `${entry.reward.miner}:${entry.reward.nodeId}`,
          {
            nodeId: entry.reward.nodeId,
            nodeIdHash: Array.from(entry.nodeIdHash),
            proof: entry.proof,
          },
        ])
      ),
      REWARDS: rewards,
    };

    fs.writeFileSync(merkleOutputPath, JSON.stringify(outputData, null, 2));
    console.log(`Merkle outputs saved to ${merkleOutputPath}`);

  } catch (error) {
    console.error("Error generating Merkle tree:", error);
    process.exit(1);
  }
}

main();
