use anchor_lang::prelude::*;

pub mod error;
pub mod events;
pub mod instructions;
pub mod state;
pub mod utils;

use instructions::*;

declare_id!("EgXM56PxY7JNSDwwBu2S4LXQV8JMJaJcQq2KTJzg5RQg");

#[program]
pub mod botanika_solana_rewards_distributor {
    use super::*;

    pub fn initialize(ctx: Context<Initialize>, authority: Pubkey) -> Result<()> {
        initialize_handler(ctx, authority)
    }

    pub fn update_root(ctx: Context<UpdateRoot>, new_root: [u8; 32]) -> Result<()> {
        update_root_handler(ctx, new_root)
    }

    pub fn claim_reward(
        ctx: Context<ClaimReward>,
        cumulative_amount: u64,
        proof: Vec<[u8; 32]>,
    ) -> Result<()> {
        claim_reward_handler(ctx, cumulative_amount, proof)
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
}
