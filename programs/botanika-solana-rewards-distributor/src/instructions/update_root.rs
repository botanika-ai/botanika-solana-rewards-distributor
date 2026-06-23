use anchor_lang::prelude::*;

use crate::error::RewardError;
use crate::state::RewardDistributor;

#[derive(Accounts)]
pub struct UpdateRoot<'info> {
    #[account(
        mut,
        seeds = [RewardDistributor::SEED],
        bump = reward_distributor.bump,
        has_one = authority @ RewardError::Unauthorized,
    )]
    pub reward_distributor: Account<'info, RewardDistributor>,

    pub authority: Signer<'info>,
}

pub fn update_root_handler(ctx: Context<UpdateRoot>, new_root: [u8; 32]) -> Result<()> {
    let reward_distributor = &mut ctx.accounts.reward_distributor;
    reward_distributor.current_root = new_root;
    reward_distributor.epoch_id = reward_distributor
        .epoch_id
        .checked_add(1)
        .ok_or(RewardError::Overflow)?;
    reward_distributor.last_updated_at = Clock::get()?.unix_timestamp;

    emit!(crate::events::RootUpdated {
        authority: ctx.accounts.authority.key(),
        new_root,
        epoch_id: reward_distributor.epoch_id,
    });

    Ok(())
}
