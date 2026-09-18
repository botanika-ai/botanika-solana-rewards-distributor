use anchor_lang::prelude::*;
use anchor_spl::token_interface::{transfer_checked, Mint, TokenAccount, TokenInterface, TransferChecked};
use ephemeral_rollups_sdk::anchor::action;
use ephemeral_rollups_sdk::pda::ephemeral_balance_pda_from_payer;

use crate::error::RewardError;
use crate::instructions::update_root::ACTION_ESCROW_INDEX;
use crate::state::{ClaimReceipt, RewardDistributor};

/// The only claim mechanism in the target design (Design Freeze v1
/// §3.10/§6.4): triggered by a Magic Action from
/// `botanika-magicblock-contracts`'s `commit_reward_balance` once
/// `RewardBalance` on the ER already established correctness -- no Merkle
/// proof needed here, unlike `claim_reward` (kept, unmodified, as the PAD
/// v1.0 §11.3 cold fallback; not a second live path).
///
/// See `update_root`'s doc comment for why this is an `#[action]` handler
/// with injected `escrow`/`escrow_auth` rather than a plain `Signer` --
/// confirmed live against MagicBlock devnet-as 2026-09-17.
#[action]
#[derive(Accounts)]
#[instruction(claim_nonce: u64, beneficiary: Pubkey, node_id_hash: [u8; 32], wallet_binding_id: u64)]
pub struct FinalizeClaim<'info> {
    #[account(
        seeds = [RewardDistributor::SEED],
        bump = reward_distributor.bump,
        constraint = !reward_distributor.is_paused @ RewardError::Paused,
    )]
    // Boxed: `try_accounts` for this struct overflowed the SBF VM's 4 KiB
    // stack frame by 8 bytes without it (confirmed via `cargo build-sbf`).
    pub reward_distributor: Box<Account<'info, RewardDistributor>>,

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
    pub claim_receipt: Account<'info, ClaimReceipt>,

    #[account(
        mut,
        token::mint = reward_mint,
        constraint = beneficiary_token_account.owner == beneficiary @ RewardError::InvalidRecipient,
    )]
    pub beneficiary_token_account: InterfaceAccount<'info, TokenAccount>,

    #[account(
        mut,
        constraint = token_vault.key() == reward_distributor.token_vault @ RewardError::InvalidVault,
        token::mint = reward_mint,
        token::authority = reward_distributor,
    )]
    pub token_vault: InterfaceAccount<'info, TokenAccount>,

    #[account(
        constraint = reward_mint.key() == reward_distributor.reward_mint @ RewardError::InvalidMint,
    )]
    pub reward_mint: InterfaceAccount<'info, Mint>,

    pub token_program: Interface<'info, TokenInterface>,
    pub system_program: Program<'info, System>,

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

    let reward_distributor = &ctx.accounts.reward_distributor;
    let seeds = &[RewardDistributor::SEED, &[reward_distributor.bump]];
    let signer_seeds = &[&seeds[..]];

    transfer_checked(
        CpiContext::new_with_signer(
            ctx.accounts.token_program.to_account_info(),
            TransferChecked {
                from: ctx.accounts.token_vault.to_account_info(),
                mint: ctx.accounts.reward_mint.to_account_info(),
                to: ctx.accounts.beneficiary_token_account.to_account_info(),
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
    });

    Ok(())
}
