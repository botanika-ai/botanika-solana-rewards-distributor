use anchor_lang::prelude::*;
use ephemeral_rollups_sdk::anchor::action;
use ephemeral_rollups_sdk::pda::ephemeral_balance_pda_from_payer;

use crate::error::RewardError;
use crate::state::{RewardDistributor, RewardSettlementState};

/// `ActionArgs::new`'s default `escrow_index` (see `ephemeral_rollups_sdk`'s
/// `magicblock_magic_program_api::args::ActionArgs::new`) -- must match what
/// `botanika-magicblock-contracts`'s `commit_epoch_state` schedules with.
pub const ACTION_ESCROW_INDEX: u8 = 255;

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

/// The only way this instruction can execute (Design Freeze v1 §5.1/§6.7):
/// a Magic Action scheduled by `botanika-magicblock-contracts`'s
/// `commit_epoch_state`, never a human keypair. There is no `root_authority:
/// Signer` the way a human-initiated instruction would have, since a Magic
/// Action's replayed CPI cannot mark an arbitrary program-chosen account
/// `is_signer`; only the derived `escrow` PDA is ever a real signer here.
///
/// Field order ground-truthed 2026-10-06 by inspecting the actual inner CPI
/// of a failing devnet tx (`getTransaction` with `innerInstructions`), not
/// assumed from docs: `[reward_distributor, settlement, system_program,
/// caller_program, escrow_auth, escrow]` -- our 3 explicit accounts (in the
/// order `commit_epoch_state` lists them) come first, THEN the delegation
/// program appends **three** accounts, not two: the calling program's own
/// id (`botanika-magicblock-contracts`, new in `ephemeral-rollups-sdk`
/// 0.17.x -- the prior "final two" comment and the troubleshooting doc's
/// "first two" both predate this and are wrong for this SDK version),
/// then `escrow_auth`, then `escrow`.
#[action]
#[derive(Accounts)]
pub struct UpdateRoot<'info> {
    #[account(mut, seeds = [RewardDistributor::SEED], bump = reward_distributor.bump)]
    pub reward_distributor: Account<'info, RewardDistributor>,

    #[account(
        init,
        payer = escrow,
        space = 8 + RewardSettlementState::INIT_SPACE,
        seeds = [
            RewardSettlementState::SEED,
            &(reward_distributor.epoch_id + 1).to_le_bytes(),
        ],
        bump
    )]
    pub settlement: Account<'info, RewardSettlementState>,

    pub system_program: Program<'info, System>,

    /// CHECK: the scheduling program's own id (`botanika-magicblock-contracts`),
    /// appended by the delegation program ahead of `escrow_auth`/`escrow` in
    /// `ephemeral-rollups-sdk` 0.17.x. Not used by this handler; declared
    /// only because the real CPI account list includes it positionally
    /// (ground-truthed from a live devnet tx, see struct doc comment).
    pub caller_program: UncheckedAccount<'info>,

    /// CHECK: bound to `reward_distributor.root_authority` (rotatable via
    /// `set_authority`) -- this is what actually restricts who can drive
    /// this instruction, since `escrow` alone only proves *some* Magic
    /// Action scheduled it, not which program's.
    #[account(address = reward_distributor.root_authority @ RewardError::Unauthorized)]
    pub escrow_auth: UncheckedAccount<'info>,

    /// CHECK: delegation-program-owned ephemeral balance PDA; only the
    /// delegation program can sign for it (via `invoke_signed` when it
    /// dispatches the post-commit action), which is the real authentication
    /// anchor for "this call arrived through a real Magic Action" -- see
    /// `escrow_auth` for who scheduled it.
    #[account(
        mut,
        signer,
        address = ephemeral_balance_pda_from_payer(&escrow_auth.key(), ACTION_ESCROW_INDEX),
    )]
    pub escrow: UncheckedAccount<'info>,
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
        authority: ctx.accounts.escrow_auth.key(),
        new_root,
        epoch_id: settlement_id,
        settlement_id,
        proof_commitment: settlement.proof_commitment,
        policy_hash: settlement.policy_hash,
        total_liability: settlement.total_liability,
    });

    Ok(())
}
