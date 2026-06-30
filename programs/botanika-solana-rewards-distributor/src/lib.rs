use anchor_lang::prelude::*;

pub mod error;
pub mod events;
pub mod instructions;
pub mod state;
pub mod utils;

use instructions::*;

declare_id!("2rBEttbbLFtXpfkQZuUj3iXoCtE5ZWcMUCtkAARm8yoK");

#[program]
pub mod botanika_solana_rewards_distributor {
    use super::*;

    pub fn initialize(ctx: Context<Initialize>, authority: Pubkey) -> Result<()> {
        initialize_handler(ctx, authority)
    }

    pub fn update_root(ctx: Context<UpdateRoot>, new_root: [u8; 32]) -> Result<()> {
        update_root_handler(ctx, new_root)
    }

    pub fn batch_payout<'a, 'b, 'c, 'info>(
        ctx: Context<'a, 'b, 'c, 'info, BatchPayout<'info>>,
        batch_id: u64,
        payouts: Vec<PayoutItem>,
    ) -> Result<()> {
        batch_payout_handler(ctx, batch_id, payouts)
    }

    pub fn claim_reward(
        ctx: Context<ClaimReward>,
        node_id_hash: [u8; 32],
        cumulative_amount: u64,
        proof: Vec<[u8; 32]>,
    ) -> Result<()> {
        claim_reward_handler(ctx, node_id_hash, cumulative_amount, proof)
    }

    pub fn pause(ctx: Context<Pause>) -> Result<()> {
        pause_handler(ctx)
    }

    pub fn unpause(ctx: Context<Unpause>) -> Result<()> {
        unpause_handler(ctx)
    }

    pub fn set_authority(ctx: Context<SetAuthority>, new_authority: Pubkey) -> Result<()> {
        set_authority_handler(ctx, new_authority)
    }

    pub fn withdraw_vault(ctx: Context<WithdrawVault>, amount: u64) -> Result<()> {
        withdraw_vault_handler(ctx, amount)
    }
}
