use anchor_lang::prelude::*;
use anchor_spl::token_interface::{transfer_checked, Mint, TokenAccount, TokenInterface, TransferChecked};

use crate::error::RewardError;
use crate::state::{ClaimLock, RewardDistributor, MAX_LOCKS_PER_WITHDRAW, MIN_LOCK_WITHDRAW};

/// Moves unlocked claim funds `lock_vault -> destination_token_account`.
///
/// `remaining_accounts` are the `ClaimLock` PDAs (writable) to draw from,
/// `amounts[i]` the base-unit amount taken from `remaining_accounts[i]`.
/// Every lock must belong to the signing `beneficiary` and be past its
/// `unlock_at`; the sum must reach `MIN_LOCK_WITHDRAW`. Which locks to
/// drain first (FIFO) is decided off-chain by whoever builds the tx.
/// `destination_token_account` may be owned by anyone (e.g. the Botanika
/// treasury for VND payouts signed by the custody beneficiary wallet).
#[derive(Accounts)]
pub struct WithdrawUnlocked<'info> {
    #[account(
        mut,
        seeds = [RewardDistributor::SEED],
        bump = reward_distributor.bump,
        constraint = !reward_distributor.is_paused @ RewardError::Paused,
    )]
    pub reward_distributor: Box<Account<'info, RewardDistributor>>,

    pub beneficiary: Signer<'info>,

    #[account(
        mut,
        constraint = lock_vault.key() == reward_distributor.lock_vault @ RewardError::InvalidLockVault,
        token::mint = reward_mint,
        token::authority = reward_distributor,
    )]
    pub lock_vault: Box<InterfaceAccount<'info, TokenAccount>>,

    #[account(
        mut,
        token::mint = reward_mint,
    )]
    pub destination_token_account: Box<InterfaceAccount<'info, TokenAccount>>,

    #[account(
        constraint = reward_mint.key() == reward_distributor.reward_mint @ RewardError::InvalidMint,
    )]
    pub reward_mint: Box<InterfaceAccount<'info, Mint>>,

    pub token_program: Interface<'info, TokenInterface>,
}

pub fn withdraw_unlocked_handler<'info>(
    ctx: Context<'_, '_, 'info, 'info, WithdrawUnlocked<'info>>,
    amounts: Vec<u64>,
) -> Result<()> {
    let lock_infos = ctx.remaining_accounts;
    require!(!lock_infos.is_empty(), RewardError::InvalidRemainingAccounts);
    require!(
        lock_infos.len() <= MAX_LOCKS_PER_WITHDRAW as usize,
        RewardError::TooManyLocks
    );
    require!(
        amounts.len() == lock_infos.len(),
        RewardError::InvalidRemainingAccounts
    );

    let program_id = ctx.program_id;
    let beneficiary = ctx.accounts.beneficiary.key();
    let now = Clock::get()?.unix_timestamp;

    let mut total: u64 = 0;
    let mut lock_keys: Vec<Pubkey> = Vec::with_capacity(lock_infos.len());

    for (lock_info, &amount) in lock_infos.iter().zip(amounts.iter()) {
        require!(
            !lock_keys.contains(lock_info.key),
            RewardError::DuplicateClaimLock
        );
        require!(lock_info.is_writable, RewardError::InvalidClaimLock);

        // Checks owner == this program and the `ClaimLock` discriminator.
        let mut lock: Account<'info, ClaimLock> =
            Account::try_from(lock_info).map_err(|_| error!(RewardError::InvalidClaimLock))?;
        let expected = Pubkey::create_program_address(
            &[ClaimLock::SEED, lock.claim_receipt.as_ref(), &[lock.bump]],
            program_id,
        )
        .map_err(|_| error!(RewardError::InvalidClaimLock))?;
        require_keys_eq!(expected, *lock_info.key, RewardError::InvalidClaimLock);

        require_keys_eq!(lock.beneficiary, beneficiary, RewardError::Unauthorized);
        require!(now >= lock.unlock_at, RewardError::LockNotUnlocked);
        require!(amount > 0, RewardError::InvalidAmount);
        let remaining = lock
            .amount
            .checked_sub(lock.withdrawn)
            .ok_or(RewardError::Overflow)?;
        require!(amount <= remaining, RewardError::LockAmountExceeded);

        lock.withdrawn = lock
            .withdrawn
            .checked_add(amount)
            .ok_or(RewardError::Overflow)?;
        lock.exit(program_id)?;

        total = total.checked_add(amount).ok_or(RewardError::Overflow)?;
        lock_keys.push(*lock_info.key);
    }

    require!(total >= MIN_LOCK_WITHDRAW, RewardError::BelowMinimumWithdraw);

    let reward_distributor = &mut ctx.accounts.reward_distributor;
    reward_distributor.total_lock_withdrawn = reward_distributor
        .total_lock_withdrawn
        .checked_add(total)
        .ok_or(RewardError::Overflow)?;

    let seeds = &[RewardDistributor::SEED, &[reward_distributor.bump]];
    let signer_seeds = &[&seeds[..]];

    transfer_checked(
        CpiContext::new_with_signer(
            ctx.accounts.token_program.to_account_info(),
            TransferChecked {
                from: ctx.accounts.lock_vault.to_account_info(),
                mint: ctx.accounts.reward_mint.to_account_info(),
                to: ctx.accounts.destination_token_account.to_account_info(),
                authority: reward_distributor.to_account_info(),
            },
            signer_seeds,
        ),
        total,
        ctx.accounts.reward_mint.decimals,
    )?;

    emit!(crate::events::LockWithdrawn {
        beneficiary,
        destination: ctx.accounts.destination_token_account.key(),
        total,
        locks: lock_keys,
        amounts,
        timestamp: now,
    });

    Ok(())
}
