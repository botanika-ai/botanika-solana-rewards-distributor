use anchor_lang::prelude::*;

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
