use anchor_lang::prelude::*;

use crate::error::RewardError;
use crate::state::RewardDistributor;

#[derive(Accounts)]
pub struct SetAuthority<'info> {
    #[account(
        mut,
        seeds = [RewardDistributor::SEED],
        bump = reward_distributor.bump,
        has_one = authority @ RewardError::Unauthorized,
    )]
    pub reward_distributor: Account<'info, RewardDistributor>,

    pub authority: Signer<'info>,
}

pub fn set_authority_handler(ctx: Context<SetAuthority>, new_authority: Pubkey) -> Result<()> {
    ctx.accounts.reward_distributor.authority = new_authority;
    Ok(())
}
