use anchor_lang::prelude::*;

use crate::error::RewardError;
use crate::state::{RewardDistributor, RewardSettlementState};

/// Off-chain-computed settlement metadata that must accompany every new root
/// (P0-RWD-03). Binds the published root to the proof epoch / reward policy
/// it was derived from instead of accepting an opaque root value.
#[derive(AnchorSerialize, AnchorDeserialize, Clone)]
pub struct SettlementInput {
    pub epoch_from: u64,
    pub epoch_to: u64,
    pub proof_commitment: [u8; 32],
    pub policy_hash: [u8; 32],
    pub canonical_ledger_hash: [u8; 32],
    pub revision_no: u32,
    pub leaf_count: u32,
    pub total_liability: u64,
}

#[derive(Accounts)]
pub struct UpdateRoot<'info> {
    #[account(
        mut,
        seeds = [RewardDistributor::SEED],
        bump = reward_distributor.bump,
        has_one = root_authority @ RewardError::Unauthorized,
    )]
    pub reward_distributor: Account<'info, RewardDistributor>,

    #[account(
        init,
        payer = root_authority,
        space = 8 + RewardSettlementState::INIT_SPACE,
        seeds = [
            RewardSettlementState::SEED,
            &(reward_distributor.epoch_id + 1).to_le_bytes(),
        ],
        bump
    )]
    pub settlement: Account<'info, RewardSettlementState>,

    #[account(mut)]
    pub root_authority: Signer<'info>,

    pub system_program: Program<'info, System>,
}

pub fn update_root_handler(
    ctx: Context<UpdateRoot>,
    new_root: [u8; 32],
    settlement: SettlementInput,
) -> Result<()> {
    require!(
        settlement.epoch_from <= settlement.epoch_to,
        RewardError::InvalidSettlementRange
    );

    let reward_distributor = &mut ctx.accounts.reward_distributor;
    reward_distributor.current_root = new_root;
    reward_distributor.epoch_id = reward_distributor
        .epoch_id
        .checked_add(1)
        .ok_or(RewardError::Overflow)?;
    reward_distributor.last_updated_at = Clock::get()?.unix_timestamp;

    let settlement_id = reward_distributor.epoch_id;
    let settled_at = reward_distributor.last_updated_at;

    let settlement_account = &mut ctx.accounts.settlement;
    settlement_account.settlement_id = settlement_id;
    settlement_account.epoch_from = settlement.epoch_from;
    settlement_account.epoch_to = settlement.epoch_to;
    settlement_account.proof_commitment = settlement.proof_commitment;
    settlement_account.policy_hash = settlement.policy_hash;
    settlement_account.canonical_ledger_hash = settlement.canonical_ledger_hash;
    settlement_account.revision_no = settlement.revision_no;
    settlement_account.reward_delta_root = new_root;
    settlement_account.leaf_count = settlement.leaf_count;
    settlement_account.total_liability = settlement.total_liability;
    settlement_account.settled_at = settled_at;
    settlement_account.bump = ctx.bumps.settlement;

    emit!(crate::events::RootUpdated {
        authority: ctx.accounts.root_authority.key(),
        new_root,
        epoch_id: settlement_id,
        settlement_id,
        proof_commitment: settlement.proof_commitment,
        policy_hash: settlement.policy_hash,
        total_liability: settlement.total_liability,
    });

    Ok(())
}
