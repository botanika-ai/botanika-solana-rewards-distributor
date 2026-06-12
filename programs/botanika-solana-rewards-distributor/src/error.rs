use anchor_lang::prelude::*;

#[error_code]
pub enum RewardError {
    #[msg("You are not authorized to perform this action")]
    Unauthorized,
    #[msg("The program is currently paused")]
    Paused,
    #[msg("Invalid Merkle proof")]
    InvalidProof,
    #[msg("The amount has already been claimed")]
    AlreadyClaimed,
    #[msg("Invalid amount")]
    InvalidAmount,
    #[msg("Numerical overflow")]
    Overflow,
    #[msg("Insufficient vault balance")]
    InsufficientFunds,
}
