use anchor_lang::prelude::*;

use crate::error::RewardError;
use crate::state::{BatchRecord, RewardDistributor};
use crate::instructions::close_settlement::MIN_SETTLEMENT_RETENTION_SEC;

/// Closes a `BatchRecord` no longer needed, reclaiming its rent to
/// `admin_authority`. Closes a real gap, same class as `close_settlement`
/// (added this session): `BatchRecord` is created once per `batch_id`
/// (`batch_payout.rs`) and never closed by any instruction, growing
/// without bound forever.
///
/// Safe to close with a simple retention window, unlike `ClaimReceipt`/
/// `ClaimStatus` (deliberately NOT given a close instruction -- see
/// `claim_record.rs`'s doc comment): `BatchRecord`'s only purpose is "a
/// batch_id can only ever be executed once" (P1-RWD-05, enforced purely by
/// the PDA's existence, not by any field value `batch_payout_handler`
/// reads back). Reusing `MIN_SETTLEMENT_RETENTION_SEC` as the same
/// conservative, not-PAD-specified window `close_settlement` already uses.
#[derive(Accounts)]
pub struct CloseBatchRecord<'info> {
    #[account(
        seeds = [RewardDistributor::SEED],
        bump = reward_distributor.bump,
        has_one = admin_authority @ RewardError::Unauthorized,
    )]
    pub reward_distributor: Account<'info, RewardDistributor>,

    #[account(
        mut,
        close = admin_authority,
        seeds = [BatchRecord::SEED, &batch_record.batch_id.to_le_bytes()],
        bump = batch_record.bump,
    )]
    pub batch_record: Account<'info, BatchRecord>,

    #[account(mut)]
    pub admin_authority: Signer<'info>,
}

pub fn close_batch_record_handler(ctx: Context<CloseBatchRecord>) -> Result<()> {
    let now = Clock::get()?.unix_timestamp;
    require!(
        now >= ctx.accounts.batch_record.processed_at + MIN_SETTLEMENT_RETENTION_SEC,
        RewardError::BatchRecordTooRecentToClose
    );
    Ok(())
}
