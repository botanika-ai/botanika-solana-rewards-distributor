use anchor_lang::prelude::*;

#[error_code]
pub enum RewardError {
    #[msg("You are not authorized to perform this action")]
    Unauthorized,
    #[msg("The program is currently paused")]
    Paused,
    #[msg("Invalid Merkle proof")]
    InvalidProof,
    #[msg("Node ID does not match claim status")]
    InvalidNodeId,
    #[msg("The amount has already been claimed")]
    AlreadyClaimed,
    #[msg("Invalid amount")]
    InvalidAmount,
    #[msg("Numerical overflow")]
    Overflow,
    #[msg("Insufficient vault balance")]
    InsufficientFunds,
    #[msg("Invalid reward token decimals, expected 9")]
    InvalidDecimals,
    #[msg("Invalid recipient token account, must be owned by the miner")]
    InvalidRecipient,
    #[msg("Provided reward mint does not match configured mint")]
    InvalidMint,
    #[msg("Provided token vault does not match configured vault")]
    InvalidVault,
    #[msg("Batch exceeds maximum payout size or is empty")]
    BatchTooLarge,
    #[msg("Remaining accounts length mismatch (expected 2 * payouts.len())")]
    InvalidRemainingAccounts,
    #[msg("Invalid ClaimStatus PDA")]
    InvalidClaimStatusPda,
    #[msg("Settlement epoch_from must be <= epoch_to")]
    InvalidSettlementRange,
    #[msg("Settlement account's settlement_id does not match the id requested")]
    SettlementIdMismatch,
    #[msg("finalize_claim_authority must be set explicitly at initialize time, not left as the default pubkey (which would otherwise silently fall back to admin_authority -- a human key, not the Magic Action escrow PDA)")]
    FinalizeClaimAuthorityRequired,
    #[msg("Settlement account is not yet old enough to close (minimum retention window not elapsed)")]
    SettlementTooRecentToClose,
    #[msg("An authority role cannot be set to the zero pubkey -- this would permanently brick whatever that role gates (e.g. a zeroed admin_authority can never again call set_authority to fix itself)")]
    AuthorityCannotBeDefault,
    #[msg("BatchRecord is not yet old enough to close (minimum retention window not elapsed)")]
    BatchRecordTooRecentToClose,
}
