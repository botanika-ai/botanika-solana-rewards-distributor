import * as anchor from "@coral-xyz/anchor";
import { Program } from "@coral-xyz/anchor";
import { Connection, Keypair, PublicKey } from "@solana/web3.js";
import { TOKEN_PROGRAM_ID } from "@solana/spl-token";
import * as fs from "fs";
import * as path from "path";
import * as os from "os";
import { resolveClusterUrl } from "./utils";

async function main() {
  console.log("Initializing Reward Distributor...");

  try {
    const configPath = path.join(__dirname, "config.json");
    if (!fs.existsSync(configPath)) {
      throw new Error("config.json not found! Run create-token.ts first.");
    }
    const config = JSON.parse(fs.readFileSync(configPath, "utf-8"));
    const tokenMint = new PublicKey(config.TOKEN_MINT);
    // finalize_claim_authority (Design Freeze v1 §5.1) must be the
    // escrow_auth PDA of botanika-magicblock-contracts's OWN program id,
    // never a human key or the zero pubkey -- `initialize_handler` now
    // rejects the zero pubkey outright (previously silently fell back to
    // admin_authority, a real gap: a forgotten config value would have
    // left `finalize_claim`'s only on-chain gate pointed at a human-signable
    // key instead of the Magic Action escrow PDA). Mirrors
    // `initialize-production.ts`'s identical derivation.
    if (!config.MAGICBLOCK_PROGRAM_ID) {
      throw new Error(
        "config.json is missing MAGICBLOCK_PROGRAM_ID -- botanika-magicblock-contracts's deployed program id is required to derive finalize_claim_authority (escrow_auth PDA)."
      );
    }
    const magicblockProgramId = new PublicKey(config.MAGICBLOCK_PROGRAM_ID);
    const [escrowAuthPda] = PublicKey.findProgramAddressSync([Buffer.from("escrow_auth")], magicblockProgramId);
    console.log(`Derived escrow_auth PDA (finalize_claim_authority): ${escrowAuthPda.toBase58()}`);
    const clusterUrl = config.CLUSTER_URL || resolveClusterUrl();

    const idlPath = path.resolve(
      __dirname,
      "../target/idl/botanika_solana_rewards_distributor.json"
    );
    if (!fs.existsSync(idlPath)) {
      throw new Error(`IDL file not found at ${idlPath}`);
    }
    const idl = JSON.parse(fs.readFileSync(idlPath, "utf-8"));
    const programId = new PublicKey(config.PROGRAM_ID || idl.address || "4HfLqCMnNW4EPrLiDkwcEewCBaNMVWkKhShKe5rRwB8o");

    const walletPath = path.resolve(os.homedir(), ".config/solana/id.json");
    const walletKeypair = Keypair.fromSecretKey(
      Uint8Array.from(JSON.parse(fs.readFileSync(walletPath, "utf-8")))
    );

    const connection = new Connection(clusterUrl, "confirmed");
    const wallet = new anchor.Wallet(walletKeypair);
    const provider = new anchor.AnchorProvider(connection, wallet, {
      commitment: "confirmed",
    });
    anchor.setProvider(provider);

    idl.address = programId.toBase58();
    const program: any = new Program(idl as any, provider);

    const [rewardDistributorPda] = PublicKey.findProgramAddressSync(
      [Buffer.from("reward_distributor")],
      programId
    );

    const tokenVaultKeypair = Keypair.generate();

    console.log("Deriving PDAs...");
    console.log(`reward_distributor: ${rewardDistributorPda.toBase58()}`);
    console.log(`token_vault: ${tokenVaultKeypair.publicKey.toBase58()}`);

    // P0-RWD-02: roles are separated on-chain so a single compromised key
    // cannot move the root, pause the program, AND sweep the vault. This
    // script still bootstraps the first five to the deployer wallet for
    // the POC -- production deployments must rotate each role to a
    // distinct multisig/timelock wallet via set_authority right after this
    // call. `finalize_claim_authority` is NOT bootstrapped to the wallet
    // like the others: it must be the escrow PDA from the very first
    // transaction, since `finalize_claim` would otherwise be permanently
    // uncallable (wrong type of key entirely, not merely a weaker one --
    // see the derivation above).
    const authorities = {
      adminAuthority: walletKeypair.publicKey,
      rootAuthority: walletKeypair.publicKey,
      payoutAuthority: walletKeypair.publicKey,
      pauseAuthority: walletKeypair.publicKey,
      treasuryAuthority: walletKeypair.publicKey,
      finalizeClaimAuthority: escrowAuthPda,
    };

    console.log("Sending initialize transaction...");
    const tx = await program.methods
      .initialize(authorities)
      .accounts({
        rewardDistributor: rewardDistributorPda,
        rewardMint: tokenMint,
        tokenVault: tokenVaultKeypair.publicKey,
        payer: walletKeypair.publicKey,
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

    config.REWARD_DISTRIBUTOR_PDA = rewardDistributorPda.toBase58();
    config.TOKEN_VAULT = tokenVaultKeypair.publicKey.toBase58();
    config.TOKEN_VAULT_SECRET = Array.from(tokenVaultKeypair.secretKey);
    config.PROGRAM_ID = programId.toBase58();
    fs.writeFileSync(configPath, JSON.stringify(config, null, 2));
    console.log(`Updated config saved to ${configPath}`);
  } catch (error) {
    console.error("Error initializing reward distributor:", error);
    process.exit(1);
  }
}

main();
