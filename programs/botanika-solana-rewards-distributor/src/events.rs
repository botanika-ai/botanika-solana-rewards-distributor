use anchor_lang::prelude::*;

#[event]
pub struct RootUpdated {
    pub authority: Pubkey,
    pub new_root: [u8; 32],
    pub epoch_id: u64,
}

#[event]
pub struct RewardClaimed {
    pub miner: Pubkey,
    pub node_id_hash: [u8; 32],
    pub amount: u64,
    pub cumulative_amount: u64,
    pub timestamp: i64,
}

#[event]
pub struct VaultWithdrawn {
    pub authority: Pubkey,
    pub treasury: Pubkey,
    pub amount: u64,
    pub remaining: u64,
    pub timestamp: i64,
}

#[event]
pub struct PayoutProcessed {
    pub batch_id: u64,
    pub recipient: Pubkey,
    pub node_id_hash: [u8; 32],
    pub amount: u64,
    pub cumulative_amount: u64,
    pub timestamp: i64,
}
