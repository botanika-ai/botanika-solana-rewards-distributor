use anchor_lang::prelude::*;

use crate::error::RewardError;
use crate::state::{RewardDistributor, RewardSettlementState};

/// Minimum age (not PAD-specified -- no retention period is mandated
/// anywhere in docs/Botanika_Planning_Architecture_Decision_v1.0_EN.md)
/// before a `RewardSettlementState` may be closed for rent reclaim. Chosen
/// conservatively (1 year) because there is no on-chain way to know whether
/// every legitimate claim referencing this `settlement_id` has already
/// landed: `claim_reward`'s own doc comment says a proof "remains valid
/// indefinitely... as long as that settlement's account hasn't been closed",
/// and `finalize_claim` (added this session) now requires the same account
/// to exist for every claim against this settlement. Closing prematurely
/// would silently and permanently block any future claim/finalize_claim for
/// this `settlement_id`, not merely fail loudly -- `admin_authority` is
/// trusted to only close settlements genuinely done being claimed against,
/// same operational-trust class as `treasury_authority` sweeping the vault.
pub const MIN_SETTLEMENT_RETENTION_SEC: i64 = 365 * 24 * 60 * 60;

/// Closes a `RewardSettlementState` account no longer needed, reclaiming
/// its rent to `admin_authority`. Closes a real gap: `RewardSettlementState`
/// accounts are created once per `update_root` call and never closed by any
/// instruction, growing without bound forever (P0-RWD-03 created the
/// account, but no corresponding cleanup path ever existed).
#[derive(Accounts)]
pub struct CloseSettlement<'info> {
    #[account(
        seeds = [RewardDistributor::SEED],
        bump = reward_distributor.bump,
        has_one = admin_authority @ RewardError::Unauthorized,
    )]
    pub reward_distributor: Account<'info, RewardDistributor>,

    #[account(
        mut,
        close = admin_authority,
        seeds = [RewardSettlementState::SEED, &settlement.settlement_id.to_le_bytes()],
        bump = settlement.bump,
    )]
    pub settlement: Account<'info, RewardSettlementState>,

    #[account(mut)]
    pub admin_authority: Signer<'info>,
}

pub fn close_settlement_handler(ctx: Context<CloseSettlement>) -> Result<()> {
    let now = Clock::get()?.unix_timestamp;
    require!(
        now >= ctx.accounts.settlement.settled_at + MIN_SETTLEMENT_RETENTION_SEC,
        RewardError::SettlementTooRecentToClose
    );
    Ok(())
}
