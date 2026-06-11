import * as anchor from "@coral-xyz/anchor";
import { Program } from "@coral-xyz/anchor";
import { BotanikaSolanaRewardsDistributor } from "../target/types/botanika_solana_rewards_distributor";

describe("botanika-solana-rewards-distributor", () => {
  // Configure the client to use the local cluster.
  anchor.setProvider(anchor.AnchorProvider.env());

  const program = anchor.workspace.botanikaSolanaRewardsDistributor as Program<BotanikaSolanaRewardsDistributor>;

  it("Is initialized!", async () => {
    // Add your test here.
    const tx = await program.methods.initialize().rpc();
    console.log("Your transaction signature", tx);
  });
});
