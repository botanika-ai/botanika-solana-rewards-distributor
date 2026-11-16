use anchor_lang::prelude::*;

/// Every finalized claim is locked for this long (60 days, chain clock)
/// before its beneficiary can move it out via `withdraw_unlocked`.
#[constant]
pub const LOCK_DURATION_SECS: i64 = 5_184_000;

/// Minimum total (sum of `amounts`) a single `withdraw_unlocked`
/// instruction must move: 100 BONSAI at 9 decimals.
#[constant]
pub const MIN_LOCK_WITHDRAW: u64 = 100_000_000_000u64;

/// Upper bound on `ClaimLock` accounts one `withdraw_unlocked` may touch.
#[constant]
pub const MAX_LOCKS_PER_WITHDRAW: u8 = 16;

/// Per-claim time lock created by `finalize_claim`: the claimed `amount`
/// sits in `RewardDistributor.lock_vault` until `unlock_at`, after which
/// `beneficiary` may withdraw any part of `amount - withdrawn` via
/// `withdraw_unlocked`. Which locks to drain first (FIFO) is an off-chain
/// business rule; on-chain only the per-lock bounds are enforced.
///
/// Seeded by its `ClaimReceipt` (one lock per finalized claim). Like
/// `ClaimReceipt`, it has no close instruction -- a fully withdrawn lock
/// stays as an audit record.
#[account]
#[derive(InitSpace)]
pub struct ClaimLock {
    pub beneficiary: Pubkey,
    pub node_id_hash: [u8; 32],
    pub wallet_binding_id: u64,
    pub claim_nonce: u64,
    pub settlement_id: u64,
    pub amount: u64,
    pub withdrawn: u64,
    pub locked_at: i64,
    pub unlock_at: i64,
    pub claim_receipt: Pubkey,
    pub bump: u8,
}

impl ClaimLock {
    pub const SEED: &'static [u8] = b"claim_lock";
}
