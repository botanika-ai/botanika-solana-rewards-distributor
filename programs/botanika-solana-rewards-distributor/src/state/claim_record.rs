use anchor_lang::prelude::*;

/// Idempotency/replay-guard state for the cold-fallback `claim_reward`/
/// `batch_payout` paths (cumulative-Merkle-proof claiming, keyed per
/// `(node_id_hash, reward_distributor)`). `amount_claimed` is the *sole*
/// on-chain record of what a node has already been paid under this
/// scheme -- `claim_reward_handler`/`batch_payout_handler` both check the
/// new proof's `cumulative_amount` against it before paying the delta.
///
/// Deliberately given NO close instruction, same reasoning as
/// `ClaimReceipt` (see that struct's doc comment): closing this and
/// letting it be recreated at `amount_claimed = 0` would let an
/// already-paid cumulative proof be replayed for a full double-payout,
/// not merely block a future legitimate claim the way closing
/// `RewardSettlementState`/`BatchRecord` too early would. Rent growth
/// here is an accepted, documented tradeoff, not an oversight.
#[account]
#[derive(InitSpace)]
pub struct ClaimStatus {
    pub node_id_hash: [u8; 32],
    pub amount_claimed: u64,
    pub last_claim: i64,
    pub bump: u8,
}

impl ClaimStatus {
    pub const SEED: &'static [u8] = b"claim_status";
}
