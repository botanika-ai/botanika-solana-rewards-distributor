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
    /// Signs `finalize_claim` (Design Freeze v1 §5.1/§7.5). Set once via
    /// `set_authority(FinalizeClaim, escrow_pda)` to the escrow PDA derived
    /// from `botanika-magicblock-contracts`'s own program -- from then on
    /// only a Magic Action from that program can produce a valid signature
    /// for `finalize_claim`, no human keypair can.
    pub finalize_claim_authority: Pubkey,
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
    /// Gates the legacy Merkle payout paths (`claim_reward`, `batch_payout`).
    /// Zero-initialized (`false`) at `initialize`: the MagicBlock
    /// `finalize_claim` flow is the only payout path unless
    /// `admin_authority` re-enables it via `set_legacy_claims_enabled`.
    /// Those legacy paths pay `token_vault -> miner` directly and therefore
    /// bypass the per-claim `ClaimLock`.
    pub legacy_claims_enabled: bool,
    /// Holds every finalized claim until its `ClaimLock.unlock_at`
    /// (`finalize_claim` moves `token_vault -> lock_vault`). Authority is
    /// this PDA; only `withdraw_unlocked` (signed by the lock's beneficiary)
    /// moves funds out -- `withdraw_vault` cannot touch it.
    pub lock_vault: Pubkey,
    /// Cumulative amount moved into `lock_vault` by `finalize_claim`.
    pub total_locked: u64,
    /// Cumulative amount moved out of `lock_vault` by `withdraw_unlocked`.
    pub total_lock_withdrawn: u64,
    #[max_len(15)]
    pub _reserved: [u8; 15],
}

impl RewardDistributor {
    pub const SEED: &'static [u8] = b"reward_distributor";
}
