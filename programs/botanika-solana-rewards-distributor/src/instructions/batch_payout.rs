use anchor_lang::prelude::*;
use anchor_lang::solana_program::program::invoke_signed;
use anchor_lang::solana_program::system_instruction;
use anchor_spl::token_interface::{transfer_checked, Mint, TokenAccount, TokenInterface, TransferChecked};

use crate::error::RewardError;
use crate::state::{BatchRecord, ClaimStatus, RewardDistributor};

/// Maximum number of payouts per transaction to stay within compute/size limits.
pub const MAX_PAYOUTS_PER_TX: usize = 10;

#[derive(AnchorSerialize, AnchorDeserialize, Clone)]
pub struct PayoutItem {
    pub recipient: Pubkey,
    pub node_id_hash: [u8; 32],
    pub cumulative_amount: u64,
}

#[derive(Accounts)]
#[instruction(batch_id: u64)]
pub struct BatchPayout<'info> {
    #[account(
        mut,
        seeds = [RewardDistributor::SEED],
        bump = reward_distributor.bump,
        has_one = payout_authority @ RewardError::Unauthorized,
        constraint = !reward_distributor.is_paused @ RewardError::Paused,
    )]
    pub reward_distributor: Account<'info, RewardDistributor>,

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

    /// Persists batch_id on-chain — re-submitting the same batch_id fails
    /// because this PDA already exists (P1-RWD-05).
    #[account(
        init,
        payer = payout_authority,
        space = 8 + BatchRecord::INIT_SPACE,
        seeds = [BatchRecord::SEED, &batch_id.to_le_bytes()],
        bump
    )]
    pub batch_record: Account<'info, BatchRecord>,

    #[account(mut)]
    pub payout_authority: Signer<'info>,

    pub token_program: Interface<'info, TokenInterface>,
    pub system_program: Program<'info, System>,
}

/// Batch payout handler.
///
/// `remaining_accounts` layout (2 per payout item, in order):
///   [i*2]     = ClaimStatus PDA (writable) — may or may not exist yet
///   [i*2 + 1] = Recipient token account (writable)
pub fn batch_payout_handler<'a, 'b, 'c, 'info>(
    ctx: Context<'a, 'b, 'c, 'info, BatchPayout<'info>>,
    batch_id: u64,
    payouts: Vec<PayoutItem>,
) -> Result<()> {
    require!(
        !payouts.is_empty() && payouts.len() <= MAX_PAYOUTS_PER_TX,
        RewardError::BatchTooLarge
    );
    require!(
        ctx.remaining_accounts.len() == payouts.len() * 2,
        RewardError::InvalidRemainingAccounts
    );

    let reward_distributor = &ctx.accounts.reward_distributor;
    let distributor_key = reward_distributor.key();
    let now = Clock::get()?.unix_timestamp;

    // PDA signer seeds for token transfers (reward_distributor is vault authority)
    let seeds = &[RewardDistributor::SEED, &[reward_distributor.bump]];
    let signer_seeds = &[&seeds[..]];

    let mut batch_total_amount: u64 = 0u64;

    for (i, item) in payouts.iter().enumerate() {
        require!(item.cumulative_amount > 0, RewardError::InvalidAmount);

        let claim_status_info = &ctx.remaining_accounts[i * 2];
        let recipient_token_info = &ctx.remaining_accounts[i * 2 + 1];

        // ── 0. Bind the declared recipient to the actual transfer destination ──
        require!(
            recipient_token_info.key() == item.recipient,
            RewardError::InvalidRecipient
        );

        // ── 1. Verify ClaimStatus PDA address ──
        let (expected_pda, pda_bump) = Pubkey::find_program_address(
            &[
                ClaimStatus::SEED,
                &item.node_id_hash,
                distributor_key.as_ref(),
            ],
            ctx.program_id,
        );
        require!(
            claim_status_info.key() == expected_pda,
            RewardError::InvalidClaimStatusPda
        );

        let mut amount_to_transfer = 0u64;

        // ── 2. Init or load ClaimStatus ──
        let claim_status_data_len = claim_status_info.try_data_len()?;
        if claim_status_data_len == 0 {
            // Account does not exist yet — create it
            let space = 8 + ClaimStatus::INIT_SPACE;
            let rent = Rent::get()?.minimum_balance(space);

            let create_ix = system_instruction::create_account(
                ctx.accounts.payout_authority.key,
                &expected_pda,
                rent,
                space as u64,
                ctx.program_id,
            );

            let pda_signer_seeds: &[&[u8]] = &[
                ClaimStatus::SEED,
                &item.node_id_hash,
                distributor_key.as_ref(),
                &[pda_bump],
            ];

            invoke_signed(
                &create_ix,
                &[
                    ctx.accounts.payout_authority.to_account_info(),
                    claim_status_info.clone(),
                    ctx.accounts.system_program.to_account_info(),
                ],
                &[pda_signer_seeds],
            )?;

            // Update the AccountInfo's data slice length in the BPF VM
            claim_status_info.realloc(space, false)?;

            // Initialize the newly created account
            let claim_status = ClaimStatus {
                node_id_hash: item.node_id_hash,
                amount_claimed: item.cumulative_amount,
                last_claim: now,
                bump: pda_bump,
            };
            let mut data = claim_status_info.try_borrow_mut_data()?;
            let mut writer = &mut data[..];
            claim_status.try_serialize(&mut writer)?;

            amount_to_transfer = item.cumulative_amount;
        } else {
            // Account exists — deserialize, update, reserialize
            let mut data = claim_status_info.try_borrow_mut_data()?;

            // Verify discriminator
            let mut disc_bytes = [0u8; 8];
            disc_bytes.copy_from_slice(&data[..8]);
            require!(
                disc_bytes == ClaimStatus::DISCRIMINATOR,
                RewardError::InvalidClaimStatusPda
            );

            // Deserialize after discriminator
            let mut claim_status: ClaimStatus =
                ClaimStatus::try_deserialize(&mut &data[..])?;

            require!(
                claim_status.node_id_hash == item.node_id_hash,
                RewardError::InvalidNodeId
            );

            if item.cumulative_amount > claim_status.amount_claimed {
                amount_to_transfer = item.cumulative_amount
                    .checked_sub(claim_status.amount_claimed)
                    .ok_or(RewardError::Overflow)?;

                claim_status.amount_claimed = item.cumulative_amount;
                claim_status.last_claim = now;

                // Reserialize
                let mut writer = &mut data[..];
                claim_status.try_serialize(&mut writer)?;
            }
        }

        // ── 3. Transfer tokens ──
        if amount_to_transfer > 0 {
            // We construct the CPI accounts manually since recipient is from remaining_accounts
            let transfer_accounts = TransferChecked {
                from: ctx.accounts.token_vault.to_account_info(),
                mint: ctx.accounts.reward_mint.to_account_info(),
                to: recipient_token_info.clone(),
                authority: reward_distributor.to_account_info(),
            };

            transfer_checked(
                CpiContext::new_with_signer(
                    ctx.accounts.token_program.to_account_info(),
                    transfer_accounts,
                    signer_seeds,
                ),
                amount_to_transfer,
                ctx.accounts.reward_mint.decimals,
            )?;
        }

        batch_total_amount = batch_total_amount
            .checked_add(amount_to_transfer)
            .ok_or(RewardError::Overflow)?;

        // ── 5. Emit event ──
        emit!(crate::events::PayoutProcessed {
            batch_id,
            recipient: item.recipient,
            node_id_hash: item.node_id_hash,
            amount: amount_to_transfer,
            cumulative_amount: item.cumulative_amount,
            timestamp: now,
        });
    }

    // ── 6. Persist batch state + running total (P1-RWD-05 / P1-RWD-06) ──
    let batch_record = &mut ctx.accounts.batch_record;
    batch_record.batch_id = batch_id;
    batch_record.item_count = payouts.len() as u16;
    batch_record.total_amount = batch_total_amount;
    batch_record.executed_by = ctx.accounts.payout_authority.key();
    batch_record.processed_at = now;
    batch_record.bump = ctx.bumps.batch_record;

    ctx.accounts.reward_distributor.total_batch_distributed = ctx
        .accounts
        .reward_distributor
        .total_batch_distributed
        .checked_add(batch_total_amount)
        .ok_or(RewardError::Overflow)?;

    emit!(crate::events::BatchCompleted {
        batch_id,
        item_count: payouts.len() as u16,
        total_amount: batch_total_amount,
        executed_by: ctx.accounts.payout_authority.key(),
        timestamp: now,
    });

    Ok(())
}
