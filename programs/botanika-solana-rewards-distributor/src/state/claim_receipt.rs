use anchor_lang::prelude::*;

/// Idempotent payout + audit state for the target claim flow (PAD v1.0
/// §6.10), replacing per-claim Merkle proofs entirely -- `finalize_claim`
/// is triggered by a Magic Action once `RewardBalance` on the ER already
/// established correctness, so no proof is needed here.
///
/// Seed is a composite of all four identity fields, not a bare `claim_id`
/// (Design Freeze v1 §3.10/§5 point 9): a per-`RewardBalance` nonce in a
/// global `[claim_receipt, claim_id]` seed would collide across unrelated
/// claimants (every fresh `RewardBalance` starts its nonce at 0), which
/// would permanently block legitimate claims rather than merely fail an
/// `init`. `transaction_reference` from PAD v1.0 §6.10 is deliberately
/// omitted -- a program cannot read its own transaction signature during
/// execution; that linkage is kept off-chain via the `ClaimFinalized` event
/// reward-service persists alongside its own settlement row.
#[account]
#[derive(InitSpace)]
pub struct ClaimReceipt {
    pub claim_nonce: u64,
    pub beneficiary: Pubkey,
    pub node_id_hash: [u8; 32],
    pub wallet_binding_id: u64,
    pub amount: u64,
    pub reward_state_hash: [u8; 32],
    pub settlement_id: u64,
    pub paid_at: i64,
    pub bump: u8,
}

impl ClaimReceipt {
    pub const SEED: &'static [u8] = b"claim_receipt";
}
