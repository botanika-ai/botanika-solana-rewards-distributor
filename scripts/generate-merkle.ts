import { Keypair, PublicKey } from "@solana/web3.js";
import { keccak_256 } from "@noble/hashes/sha3";
import * as fs from "fs";
import * as path from "path";

// Leaf hashing matching the Rust contract: keccak256(miner_pubkey || amount_le)
function hashLeaf(miner: PublicKey, cumulativeAmount: bigint): Buffer {
  const minerBuffer = miner.toBuffer();
  const amountBuffer = Buffer.alloc(8);
  amountBuffer.writeBigUInt64LE(cumulativeAmount);
  const data = Buffer.concat([minerBuffer, amountBuffer]);
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

    // 2. Prepare sample rewards
    const rewards = [
      { miner: wallet1, amount: 1000 },
      { miner: wallet2, amount: 5000 }
    ];

    console.log("Sample Reward List:");
    console.log(JSON.stringify(rewards, null, 2));

    // 3. Compute leaves
    const leaf1 = hashLeaf(new PublicKey(wallet1), BigInt(rewards[0].amount));
    const leaf2 = hashLeaf(new PublicKey(wallet2), BigInt(rewards[1].amount));

    // 4. Construct tree
    const tree = new MerkleTree([leaf1, leaf2]);
    const root = tree.getRoot();

    // 5. Compute proofs
    const proof1 = tree.getProof(leaf1).map(b => Array.from(b));
    const proof2 = tree.getProof(leaf2).map(b => Array.from(b));

    // Output root as hex and as 32-byte array
    const rootHex = root.toString("hex");
    const rootArray = Array.from(root);

    console.log("\n----------------------------------------");
    console.log(`MERKLE_ROOT = [${rootArray.join(", ")}]`);
    console.log(`(Hex: 0x${rootHex})`);
    console.log("\nPROOFS = {");
    console.log(`  "${wallet1}": [${proof1.map(p => "[" + p.join(", ") + "]").join(", ")}],`);
    console.log(`  "${wallet2}": [${proof2.map(p => "[" + p.join(", ") + "]").join(", ")}]`);
    console.log("}");
    console.log("----------------------------------------\n");

    // Save proof output
    const merkleOutputPath = path.join(__dirname, "merkle-output.json");
    const outputData = {
      MERKLE_ROOT: rootArray,
      MERKLE_ROOT_HEX: rootHex,
      PROOFS: {
        [wallet1]: proof1,
        [wallet2]: proof2
      },
      REWARDS: rewards
    };
    
    fs.writeFileSync(merkleOutputPath, JSON.stringify(outputData, null, 2));
    console.log(`Merkle outputs saved to ${merkleOutputPath}`);

  } catch (error) {
    console.error("Error generating Merkle tree:", error);
    process.exit(1);
  }
}

main();
