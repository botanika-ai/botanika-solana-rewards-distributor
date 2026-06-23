use anchor_lang::prelude::*;

#[account]
#[derive(InitSpace)]
pub struct RewardDistributor {
    pub authority: Pubkey,
    pub reward_mint: Pubkey,
    pub current_root: [u8; 32],
    pub epoch_id: u64,
    pub token_vault: Pubkey,
    pub bump: u8,
    pub is_paused: bool,
    pub last_updated_at: i64,
    pub total_distributed: u64,
    #[max_len(64)]
    pub _reserved: [u8; 64],
}

impl RewardDistributor {
    pub const SEED: &'static [u8] = b"reward_distributor";
}
