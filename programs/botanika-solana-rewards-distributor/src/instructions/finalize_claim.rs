use anchor_lang::prelude::*;
use anchor_spl::token_interface::{transfer_checked, Mint, TokenAccount, TokenInterface, TransferChecked};
use ephemeral_rollups_sdk::anchor::action;
use ephemeral_rollups_sdk::pda::ephemeral_balance_pda_from_payer;

use crate::error::RewardError;
use crate::instructions::update_root::ACTION_ESCROW_INDEX;
use crate::state::{ClaimLock, ClaimReceipt, RewardDistributor, RewardSettlementState, LOCK_DURATION_SECS};

/// The only claim mechanism in the target design (Design Freeze v1
/// §3.10/§6.4): triggered by a Magic Action from
/// `botanika-magicblock-contracts`'s `commit_reward_balance` once
/// `RewardBalance` on the ER already established correctness -- no Merkle
/// proof needed here, unlike `claim_reward` (kept, unmodified, as the PAD
/// v1.0 §11.3 cold fallback; not a second live path).
///
/// Does NOT pay the beneficiary: the claimed `amount` moves
/// `token_vault -> lock_vault` and a per-claim `ClaimLock` is created with
/// `unlock_at = now + LOCK_DURATION_SECS` (60 days). The beneficiary later
/// moves unlocked funds out via `withdraw_unlocked`.
///
/// See `update_root`'s doc comment for why this is an `#[action]` handler
/// with injected `escrow`/`escrow_auth` (and `caller_program`) rather than
/// a plain `Signer`. The declared field order below is the account order
/// `botanika-magicblock-contracts`'s `commit_reward_balance` passes (8
/// remaining accounts + `system_program`, then the injected three) -- keep
/// both sides in sync.
///
/// `settlement` closes a real gap found reviewing this flow: previously
/// `settlement_id` was accepted as a bare argument, stored on
/// `ClaimReceipt` for audit, but never checked against anything -- a
/// caller could reference a `settlement_id` that was never actually
/// published via `update_root` at all. Mirrors `claim_reward.rs`'s
/// identical `settlement` field exactly (that instruction already needed
/// this same binding for its own Merkle-proof verification, same
/// `RewardSettlementState` account, same `SettlementIdMismatch` guard).
/// This does NOT verify `amount` cryptographically (there is deliberately
/// no per-leaf Merkle proof in this claim path -- see the module doc
/// comment above); it closes the narrower but real gap that `settlement_id`
/// itself was previously unconstrained to any actually-published root.
#[action]
#[derive(Accounts)]
#[instruction(claim_nonce: u64, beneficiary: Pubkey, node_id_hash: [u8; 32], wallet_binding_id: u64, amount: u64, reward_state_hash: [u8; 32], settlement_id: u64)]
pub struct FinalizeClaim<'info> {
    #[account(
        mut,
        seeds = [RewardDistributor::SEED],
        bump = reward_distributor.bump,
        constraint = !reward_distributor.is_paused @ RewardError::Paused,
    )]
    // Boxed: `try_accounts` for this struct overflowed the SBF VM's 4 KiB
    // stack frame by 8 bytes without it (confirmed via `cargo build-sbf`),
    // same fix `claim_reward.rs`'s identical comment already applies.
    pub reward_distributor: Box<Account<'info, RewardDistributor>>,

    #[account(
        seeds = [RewardSettlementState::SEED, &settlement_id.to_le_bytes()],
        bump = settlement.bump,
        constraint = settlement.settlement_id == settlement_id @ RewardError::SettlementIdMismatch,
    )]
    pub settlement: Box<Account<'info, RewardSettlementState>>,

    #[account(
        init,
        payer = escrow,
        space = 8 + ClaimReceipt::INIT_SPACE,
        seeds = [
            ClaimReceipt::SEED,
            beneficiary.as_ref(),
            &node_id_hash,
            &wallet_binding_id.to_le_bytes(),
            &claim_nonce.to_le_bytes(),
        ],
        bump
    )]
    pub claim_receipt: Box<Account<'info, ClaimReceipt>>,

    #[account(
        init,
        payer = escrow,
        space = 8 + ClaimLock::INIT_SPACE,
        seeds = [ClaimLock::SEED, claim_receipt.key().as_ref()],
        bump
    )]
    pub claim_lock: Box<Account<'info, ClaimLock>>,

    #[account(
        mut,
        constraint = token_vault.key() == reward_distributor.token_vault @ RewardError::InvalidVault,
        token::mint = reward_mint,
        token::authority = reward_distributor,
    )]
    pub token_vault: Box<InterfaceAccount<'info, TokenAccount>>,

    #[account(
        mut,
        constraint = lock_vault.key() == reward_distributor.lock_vault @ RewardError::InvalidLockVault,
        token::mint = reward_mint,
        token::authority = reward_distributor,
    )]
    pub lock_vault: Box<InterfaceAccount<'info, TokenAccount>>,

    #[account(
        constraint = reward_mint.key() == reward_distributor.reward_mint @ RewardError::InvalidMint,
    )]
    pub reward_mint: Box<InterfaceAccount<'info, Mint>>,

    pub token_program: Interface<'info, TokenInterface>,
    pub system_program: Program<'info, System>,

    /// CHECK: see `update_root`'s identical `caller_program` doc comment.
    pub caller_program: UncheckedAccount<'info>,

    /// CHECK: bound to `reward_distributor.finalize_claim_authority`
    /// (rotatable via `set_authority`) -- see `update_root`'s identical
    /// `escrow_auth` doc comment.
    #[account(address = reward_distributor.finalize_claim_authority @ RewardError::Unauthorized)]
    pub escrow_auth: UncheckedAccount<'info>,

    /// CHECK: see `update_root`'s identical `escrow` doc comment.
    #[account(
        mut,
        signer,
        address = ephemeral_balance_pda_from_payer(&escrow_auth.key(), ACTION_ESCROW_INDEX),
    )]
    pub escrow: UncheckedAccount<'info>,
}

#[allow(clippy::too_many_arguments)]
pub fn finalize_claim_handler(
    ctx: Context<FinalizeClaim>,
    claim_nonce: u64,
    beneficiary: Pubkey,
    node_id_hash: [u8; 32],
    wallet_binding_id: u64,
    amount: u64,
    reward_state_hash: [u8; 32],
    settlement_id: u64,
) -> Result<()> {
    require!(amount > 0, RewardError::InvalidAmount);

    let now = Clock::get()?.unix_timestamp;
    let unlock_at = now
        .checked_add(LOCK_DURATION_SECS)
        .ok_or(RewardError::Overflow)?;

    let claim_receipt_key = ctx.accounts.claim_receipt.key();
    let claim_receipt = &mut ctx.accounts.claim_receipt;
    claim_receipt.claim_nonce = claim_nonce;
    claim_receipt.beneficiary = beneficiary;
    claim_receipt.node_id_hash = node_id_hash;
    claim_receipt.wallet_binding_id = wallet_binding_id;
    claim_receipt.amount = amount;
    claim_receipt.reward_state_hash = reward_state_hash;
    claim_receipt.settlement_id = settlement_id;
    claim_receipt.paid_at = now;
    claim_receipt.bump = ctx.bumps.claim_receipt;

    let claim_lock = &mut ctx.accounts.claim_lock;
    claim_lock.beneficiary = beneficiary;
    claim_lock.node_id_hash = node_id_hash;
    claim_lock.wallet_binding_id = wallet_binding_id;
    claim_lock.claim_nonce = claim_nonce;
    claim_lock.settlement_id = settlement_id;
    claim_lock.amount = amount;
    claim_lock.withdrawn = 0;
    claim_lock.locked_at = now;
    claim_lock.unlock_at = unlock_at;
    claim_lock.claim_receipt = claim_receipt_key;
    claim_lock.bump = ctx.bumps.claim_lock;

    let reward_distributor = &mut ctx.accounts.reward_distributor;
    reward_distributor.total_locked = reward_distributor
        .total_locked
        .checked_add(amount)
        .ok_or(RewardError::Overflow)?;

    let seeds = &[RewardDistributor::SEED, &[reward_distributor.bump]];
    let signer_seeds = &[&seeds[..]];

    transfer_checked(
        CpiContext::new_with_signer(
            ctx.accounts.token_program.to_account_info(),
            TransferChecked {
                from: ctx.accounts.token_vault.to_account_info(),
                mint: ctx.accounts.reward_mint.to_account_info(),
                to: ctx.accounts.lock_vault.to_account_info(),
                authority: reward_distributor.to_account_info(),
            },
            signer_seeds,
        ),
        amount,
        ctx.accounts.reward_mint.decimals,
    )?;

    emit!(crate::events::ClaimFinalized {
        claim_nonce,
        beneficiary,
        node_id_hash,
        wallet_binding_id,
        amount,
        settlement_id,
        timestamp: now,
        claim_lock: ctx.accounts.claim_lock.key(),
        unlock_at,
    });

    Ok(())
}
