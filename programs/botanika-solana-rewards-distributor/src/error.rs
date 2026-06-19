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
}
