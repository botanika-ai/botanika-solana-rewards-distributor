use anchor_lang::prelude::*;

#[account]
#[derive(InitSpace)]
pub struct ClaimStatus {
    pub miner: Pubkey,
    pub amount_claimed: u64,
    pub last_claim: i64,
    pub bump: u8,
    #[max_len(32)]
    pub _reserved: [u8; 32],
}

impl ClaimStatus {
    pub const SEED: &'static [u8] = b"claim_status";
}
