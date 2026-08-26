use anchor_lang::prelude::*;

/// Persists batch_id as on-chain state so a batch can only ever be executed
/// once (P1-RWD-05) — re-submitting the same batch_id fails because the PDA
/// already exists.
#[account]
#[derive(InitSpace)]
pub struct BatchRecord {
    pub batch_id: u64,
    pub item_count: u16,
    pub total_amount: u64,
    pub executed_by: Pubkey,
    pub processed_at: i64,
    pub bump: u8,
}

impl BatchRecord {
    pub const SEED: &'static [u8] = b"batch_record";
}
