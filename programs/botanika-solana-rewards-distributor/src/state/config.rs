use anchor_lang::prelude::*;

#[account]
#[derive(InitSpace)]
pub struct RewardDistributor {
    /// Can rotate any of the roles below. Should be a multisig/timelock in production.
    pub admin_authority: Pubkey,
    /// Can publish new Merkle roots / reward settlements (update_root).
    pub root_authority: Pubkey,
    /// Can execute batch_payout.
    pub payout_authority: Pubkey,
    /// Can pause/unpause the program.
    pub pause_authority: Pubkey,
    /// Can sweep the token vault back to treasury.
    pub treasury_authority: Pubkey,
    pub reward_mint: Pubkey,
    pub current_root: [u8; 32],
    pub epoch_id: u64,
    pub token_vault: Pubkey,
    pub bump: u8,
    pub is_paused: bool,
    pub last_updated_at: i64,
    /// Cumulative amount paid out via self-serve claim_reward.
    pub total_claimed: u64,
    /// Cumulative amount paid out via authority-driven batch_payout.
    pub total_batch_distributed: u64,
    #[max_len(64)]
    pub _reserved: [u8; 64],
}

impl RewardDistributor {
    pub const SEED: &'static [u8] = b"reward_distributor";
}
