use anchor_lang::prelude::*;

use crate::error::RewardError;
use crate::state::RewardDistributor;

#[derive(Accounts)]
pub struct Pause<'info> {
    #[account(
        mut,
        seeds = [RewardDistributor::SEED],
        bump = reward_distributor.bump,
        has_one = pause_authority @ RewardError::Unauthorized,
    )]
    pub reward_distributor: Account<'info, RewardDistributor>,

    pub pause_authority: Signer<'info>,
}

pub fn pause_handler(ctx: Context<Pause>) -> Result<()> {
    ctx.accounts.reward_distributor.is_paused = true;
    Ok(())
}
