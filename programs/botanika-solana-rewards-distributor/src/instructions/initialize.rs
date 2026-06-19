use anchor_lang::prelude::*;
use anchor_spl::token_interface::{Mint, TokenAccount, TokenInterface};

use crate::state::RewardDistributor;
use crate::error::RewardError;

#[derive(Accounts)]
pub struct Initialize<'info> {
    #[account(
        init,
        payer = payer,
        space = 8 + RewardDistributor::INIT_SPACE,
        seeds = [RewardDistributor::SEED],
        bump
    )]
    pub reward_distributor: Account<'info, RewardDistributor>,

    #[account(
        constraint = reward_mint.decimals == 9 @ RewardError::InvalidDecimals
    )]
    pub reward_mint: InterfaceAccount<'info, Mint>,

    #[account(
        init,
        payer = payer,
        token::mint = reward_mint,
        token::authority = reward_distributor,
    )]
    pub token_vault: InterfaceAccount<'info, TokenAccount>,

    #[account(mut)]
    pub payer: Signer<'info>,

    pub token_program: Interface<'info, TokenInterface>,
    pub system_program: Program<'info, System>,
}

pub fn initialize_handler(ctx: Context<Initialize>, authority: Pubkey) -> Result<()> {
    let reward_distributor = &mut ctx.accounts.reward_distributor;
    reward_distributor.authority = authority;
    reward_distributor.reward_mint = ctx.accounts.reward_mint.key();
    reward_distributor.token_vault = ctx.accounts.token_vault.key();
    reward_distributor.bump = ctx.bumps.reward_distributor;
    reward_distributor.current_root = [0u8; 32];
    reward_distributor.root_version = 0;
    reward_distributor.is_paused = false;
    reward_distributor.last_updated_at = Clock::get()?.unix_timestamp;
    reward_distributor.total_distributed = 0;

    Ok(())
}
