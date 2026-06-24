use anchor_lang::prelude::*;
use anchor_lang::solana_program::program::invoke_signed;
use anchor_lang::solana_program::system_instruction;
use anchor_spl::token_interface::{transfer_checked, Mint, TokenAccount, TokenInterface, TransferChecked};

use crate::error::RewardError;
use crate::state::{ClaimStatus, RewardDistributor};

/// Maximum number of payouts per transaction to stay within compute/size limits.
pub const MAX_PAYOUTS_PER_TX: usize = 10;

#[derive(AnchorSerialize, AnchorDeserialize, Clone)]
pub struct PayoutItem {
    pub recipient: Pubkey,
    pub node_id_hash: [u8; 32],
    pub amount: u64,
}

#[derive(Accounts)]
pub struct BatchPayout<'info> {
    #[account(
        mut,
        seeds = [RewardDistributor::SEED],
        bump = reward_distributor.bump,
        has_one = authority @ RewardError::Unauthorized,
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

    #[account(mut)]
    pub authority: Signer<'info>,

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

    let reward_distributor = &mut ctx.accounts.reward_distributor;
    let distributor_key = reward_distributor.key();
    let now = Clock::get()?.unix_timestamp;

    // PDA signer seeds for token transfers (reward_distributor is vault authority)
    let seeds = &[RewardDistributor::SEED, &[reward_distributor.bump]];
    let signer_seeds = &[&seeds[..]];

    for (i, item) in payouts.iter().enumerate() {
        require!(item.amount > 0, RewardError::InvalidAmount);

        let claim_status_info = &ctx.remaining_accounts[i * 2];
        let recipient_token_info = &ctx.remaining_accounts[i * 2 + 1];

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

        // ── 2. Init or load ClaimStatus ──
        let claim_status_data_len = claim_status_info.try_data_len()?;
        if claim_status_data_len == 0 {
            // Account does not exist yet — create it
            let space = 8 + ClaimStatus::INIT_SPACE;
            let rent = Rent::get()?.minimum_balance(space);

            let create_ix = system_instruction::create_account(
                ctx.accounts.authority.key,
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
                    ctx.accounts.authority.to_account_info(),
                    claim_status_info.clone(),
                    ctx.accounts.system_program.to_account_info(),
                ],
                &[pda_signer_seeds],
            )?;

            // Initialize the newly created account
            let mut data = claim_status_info.try_borrow_mut_data()?;
            let mut cursor = std::io::Cursor::new(&mut data[..]);

            // Write discriminator (first 8 bytes)
            let discriminator = ClaimStatus::DISCRIMINATOR;
            anchor_lang::prelude::borsh::BorshSerialize::serialize(&discriminator, &mut cursor)?;

            // Write fields
            let claim_status = ClaimStatus {
                node_id_hash: item.node_id_hash,
                amount_claimed: item.amount,
                last_claim: now,
                bump: pda_bump,
            };
            anchor_lang::prelude::borsh::BorshSerialize::serialize(&claim_status, &mut cursor)?;
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

            claim_status.amount_claimed = claim_status
                .amount_claimed
                .checked_add(item.amount)
                .ok_or(RewardError::Overflow)?;
            claim_status.last_claim = now;

            // Reserialize
            let mut cursor = std::io::Cursor::new(&mut data[..]);
            anchor_lang::prelude::borsh::BorshSerialize::serialize(
                &ClaimStatus::DISCRIMINATOR,
                &mut cursor,
            )?;
            anchor_lang::prelude::borsh::BorshSerialize::serialize(&claim_status, &mut cursor)?;
        }

        // ── 3. Transfer tokens ──
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
            item.amount,
            ctx.accounts.reward_mint.decimals,
        )?;

        // ── 4. Update distributor total ──
        reward_distributor.total_distributed = reward_distributor
            .total_distributed
            .checked_add(item.amount)
            .ok_or(RewardError::Overflow)?;

        // ── 5. Emit event ──
        let cumulative = if claim_status_data_len == 0 {
            item.amount
        } else {
            // Re-read for cumulative (we already wrote it above)
            let data = claim_status_info.try_borrow_data()?;
            let claim: ClaimStatus = ClaimStatus::try_deserialize(&mut &data[..])?;
            claim.amount_claimed
        };

        emit!(crate::events::PayoutProcessed {
            batch_id,
            recipient: item.recipient,
            node_id_hash: item.node_id_hash,
            amount: item.amount,
            cumulative_amount: cumulative,
            timestamp: now,
        });
    }

    Ok(())
}
