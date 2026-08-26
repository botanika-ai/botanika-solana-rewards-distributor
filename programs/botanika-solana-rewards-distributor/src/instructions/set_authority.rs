use anchor_lang::prelude::*;

use crate::error::RewardError;
use crate::state::RewardDistributor;

/// Individually-rotatable roles (P0-RWD-02) — separated so a compromise of
/// one operational key (e.g. payout automation) cannot also move the root,
/// pause the program, or sweep the vault.
#[derive(AnchorSerialize, AnchorDeserialize, Clone, Copy, PartialEq, Eq)]
pub enum AuthorityRole {
    Admin,
    Root,
    Payout,
    Pause,
    Treasury,
}

#[derive(Accounts)]
pub struct SetAuthority<'info> {
    #[account(
        mut,
        seeds = [RewardDistributor::SEED],
        bump = reward_distributor.bump,
        has_one = admin_authority @ RewardError::Unauthorized,
    )]
    pub reward_distributor: Account<'info, RewardDistributor>,

    pub admin_authority: Signer<'info>,
}

pub fn set_authority_handler(
    ctx: Context<SetAuthority>,
    role: AuthorityRole,
    new_authority: Pubkey,
) -> Result<()> {
    let reward_distributor = &mut ctx.accounts.reward_distributor;
    match role {
        AuthorityRole::Admin => reward_distributor.admin_authority = new_authority,
        AuthorityRole::Root => reward_distributor.root_authority = new_authority,
        AuthorityRole::Payout => reward_distributor.payout_authority = new_authority,
        AuthorityRole::Pause => reward_distributor.pause_authority = new_authority,
        AuthorityRole::Treasury => reward_distributor.treasury_authority = new_authority,
    }

    emit!(crate::events::AuthorityUpdated {
        role,
        new_authority,
        updated_by: ctx.accounts.admin_authority.key(),
    });

    Ok(())
}
