use anchor_lang::prelude::*;
use anchor_lang::solana_program::keccak;
use anchor_spl::token_interface::{transfer_checked, Mint, TokenAccount, TokenInterface, TransferChecked};

use crate::error::RewardError;
use crate::state::{ClaimStatus, RewardDistributor};
use crate::utils::merkle;

#[derive(Accounts)]
#[instruction(node_id_hash: [u8; 32])]
pub struct ClaimReward<'info> {
    #[account(
        mut,
        seeds = [RewardDistributor::SEED],
        bump = reward_distributor.bump,
        constraint = !reward_distributor.is_paused @ RewardError::Paused,
    )]
    pub reward_distributor: Account<'info, RewardDistributor>,

    #[account(
        init_if_needed,
        payer = miner,
        space = 8 + ClaimStatus::INIT_SPACE,
        seeds = [
            ClaimStatus::SEED,
            &node_id_hash,
            reward_distributor.key().as_ref(),
        ],
        bump
    )]
    pub claim_status: Account<'info, ClaimStatus>,

    #[account(
        mut,
        token::mint = reward_mint,
        constraint = miner_token_account.owner == miner.key() @ RewardError::InvalidRecipient,
    )]
    pub miner_token_account: InterfaceAccount<'info, TokenAccount>,

    #[account(
        mut,
        constraint = token_vault.key() == reward_distributor.token_vault @ RewardError::InvalidVault,
        token::mint = reward_mint,
        token::authority = reward_distributor,
    )]
    pub token_vault: InterfaceAccount<'info, TokenAccount>,

    #[account(
        constraint = reward_mint.key() == reward_distributor.reward_mint @ RewardError::InvalidMint,
    )]
    pub reward_mint: InterfaceAccount<'info, Mint>,

    #[account(mut)]
    pub miner: Signer<'info>,

    pub token_program: Interface<'info, TokenInterface>,
    pub system_program: Program<'info, System>,
}

pub fn claim_reward_handler(
    ctx: Context<ClaimReward>,
    node_id_hash: [u8; 32],
    cumulative_amount: u64,
    proof: Vec<[u8; 32]>,
) -> Result<()> {
    let reward_distributor = &mut ctx.accounts.reward_distributor;
    let claim_status = &mut ctx.accounts.claim_status;

    if claim_status.node_id_hash == [0u8; 32] {
        claim_status.node_id_hash = node_id_hash;
        claim_status.bump = ctx.bumps.claim_status;
    } else {
        require!(
            claim_status.node_id_hash == node_id_hash,
            RewardError::InvalidNodeId
        );
    }

    if claim_status.amount_claimed >= cumulative_amount {
        return Err(RewardError::AlreadyClaimed.into());
    }

    // Leaf = keccak256(miner_pubkey, node_id_hash, cumulative_amount)
    let leaf = keccak::hashv(&[
        ctx.accounts.miner.key().as_ref(),
        &node_id_hash,
        &cumulative_amount.to_le_bytes(),
    ])
    .0;

    if !merkle::verify_proof(reward_distributor.current_root, leaf, proof) {
        return Err(RewardError::InvalidProof.into());
    }

    let amount_to_claim = cumulative_amount
        .checked_sub(claim_status.amount_claimed)
        .ok_or(RewardError::Overflow)?;

    claim_status.amount_claimed = cumulative_amount;
    claim_status.last_claim = Clock::get()?.unix_timestamp;

    reward_distributor.total_distributed = reward_distributor
        .total_distributed
        .checked_add(amount_to_claim)
        .ok_or(RewardError::Overflow)?;

    let seeds = &[RewardDistributor::SEED, &[reward_distributor.bump]];
    let signer_seeds = &[&seeds[..]];

    transfer_checked(
        CpiContext::new_with_signer(
            ctx.accounts.token_program.to_account_info(),
            TransferChecked {
                from: ctx.accounts.token_vault.to_account_info(),
                mint: ctx.accounts.reward_mint.to_account_info(),
                to: ctx.accounts.miner_token_account.to_account_info(),
                authority: reward_distributor.to_account_info(),
            },
            signer_seeds,
        ),
        amount_to_claim,
        ctx.accounts.reward_mint.decimals,
    )?;

    emit!(crate::events::RewardClaimed {
        miner: ctx.accounts.miner.key(),
        node_id_hash,
        amount: amount_to_claim,
        cumulative_amount,
        timestamp: claim_status.last_claim,
    });

    Ok(())
}
