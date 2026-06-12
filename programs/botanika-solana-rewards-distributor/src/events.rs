use anchor_lang::prelude::*;

#[event]
pub struct RootUpdated {
    pub authority: Pubkey,
    pub new_root: [u8; 32],
    pub root_version: u64,
}

#[event]
pub struct RewardClaimed {
    pub miner: Pubkey,
    pub amount: u64,
    pub cumulative_amount: u64,
    pub timestamp: i64,
}
