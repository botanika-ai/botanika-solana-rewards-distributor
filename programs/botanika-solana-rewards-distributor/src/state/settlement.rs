use anchor_lang::prelude::*;

/// Links a published reward Merkle root to the off-chain proof epoch it was
/// computed from (P0-RWD-03). One account per epoch, created in update_root.
#[account]
#[derive(InitSpace)]
pub struct RewardSettlementState {
    /// Equal to RewardDistributor.epoch_id after this settlement is applied.
    pub settlement_id: u64,
    pub epoch_from: u64,
    pub epoch_to: u64,
    /// Commitment to the raw proof set this settlement was computed from.
    pub proof_commitment: [u8; 32],
    /// Hash of the reward policy/config version used for this settlement.
    pub policy_hash: [u8; 32],
    /// Hash of the canonical off-chain reward ledger snapshot.
    pub canonical_ledger_hash: [u8; 32],
    pub revision_no: u32,
    /// Same value as RewardDistributor.current_root after this settlement.
    pub reward_delta_root: [u8; 32],
    pub leaf_count: u32,
    pub total_liability: u64,
    pub settled_at: i64,
    pub bump: u8,
}

impl RewardSettlementState {
    pub const SEED: &'static [u8] = b"settlement";
}
