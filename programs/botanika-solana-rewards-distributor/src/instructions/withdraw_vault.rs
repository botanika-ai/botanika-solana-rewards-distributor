use anchor_lang::prelude::*;
use anchor_spl::token_interface::{transfer_checked, Mint, TokenAccount, TokenInterface, TransferChecked};

use crate::error::RewardError;
use crate::state::RewardDistributor;

#[derive(Accounts)]
pub struct WithdrawVault<'info> {
    #[account(
        mut,
        seeds = [RewardDistributor::SEED],
        bump = reward_distributor.bump,
        has_one = authority @ RewardError::Unauthorized,
    )]
    pub reward_distributor: Account<'info, RewardDistributor>,

    #[account(
        mut,
        address = reward_distributor.token_vault,
    )]
    pub token_vault: InterfaceAccount<'info, TokenAccount>,

    #[account(
        address = reward_distributor.reward_mint,
    )]
    pub reward_mint: InterfaceAccount<'info, Mint>,

    /// Treasury token account to receive the withdrawn tokens.
    /// Must hold the same mint as the vault.
    #[account(
        mut,
        token::mint = reward_mint,
    )]
    pub treasury: InterfaceAccount<'info, TokenAccount>,

    pub authority: Signer<'info>,

    pub token_program: Interface<'info, TokenInterface>,
}

/// Withdraw tokens from the vault back to treasury.
///
/// `amount` – the number of tokens (in base units) to sweep.
///            Pass `u64::MAX` to sweep the entire vault balance.
pub fn withdraw_vault_handler(ctx: Context<WithdrawVault>, amount: u64) -> Result<()> {
    let vault_balance = ctx.accounts.token_vault.amount;

    let withdraw_amount = if amount == u64::MAX {
        vault_balance
    } else {
        amount
    };

    require!(withdraw_amount > 0, RewardError::InvalidAmount);
    require!(vault_balance >= withdraw_amount, RewardError::InsufficientFunds);

    // PDA signer seeds for the reward_distributor (vault authority)
    let seeds = &[
        RewardDistributor::SEED,
        &[ctx.accounts.reward_distributor.bump],
    ];
    let signer_seeds = &[&seeds[..]];

    transfer_checked(
        CpiContext::new_with_signer(
            ctx.accounts.token_program.to_account_info(),
            TransferChecked {
                from: ctx.accounts.token_vault.to_account_info(),
                mint: ctx.accounts.reward_mint.to_account_info(),
                to: ctx.accounts.treasury.to_account_info(),
                authority: ctx.accounts.reward_distributor.to_account_info(),
            },
            signer_seeds,
        ),
        withdraw_amount,
        ctx.accounts.reward_mint.decimals,
    )?;

    emit!(crate::events::VaultWithdrawn {
        authority: ctx.accounts.authority.key(),
        treasury: ctx.accounts.treasury.key(),
        amount: withdraw_amount,
        remaining: vault_balance.checked_sub(withdraw_amount).unwrap_or(0),
        timestamp: Clock::get()?.unix_timestamp,
    });

    Ok(())
}
