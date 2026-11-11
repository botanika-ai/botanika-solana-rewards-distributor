use anchor_lang::prelude::*;

use crate::error::RewardError;
use crate::events::LegacyClaimsToggled;
use crate::state::RewardDistributor;

#[derive(Accounts)]
pub struct SetLegacyClaimsEnabled<'info> {
    #[account(
        mut,
        seeds = [RewardDistributor::SEED],
        bump = reward_distributor.bump,
        has_one = admin_authority @ RewardError::Unauthorized,
    )]
    pub reward_distributor: Account<'info, RewardDistributor>,

    pub admin_authority: Signer<'info>,
}

pub fn set_legacy_claims_enabled_handler(
    ctx: Context<SetLegacyClaimsEnabled>,
    enabled: bool,
) -> Result<()> {
    ctx.accounts.reward_distributor.legacy_claims_enabled = enabled;
    emit!(LegacyClaimsToggled {
        enabled,
        updated_by: ctx.accounts.admin_authority.key(),
    });
    Ok(())
}
