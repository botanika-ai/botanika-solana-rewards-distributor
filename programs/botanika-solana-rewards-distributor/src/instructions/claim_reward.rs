use anchor_lang::prelude::*;
use anchor_lang::solana_program::keccak;
use anchor_spl::token_interface::{transfer_checked, Mint, TokenAccount, TokenInterface, TransferChecked};

use crate::error::RewardError;
use crate::state::{ClaimStatus, RewardDistributor, RewardSettlementState};
use crate::utils::merkle;

/// Domain separation tag for reward Merkle leaves (P1-RWD-07). Leaves are
/// additionally bound to program_id, distributor PDA, reward_mint and
/// epoch_id so a leaf/proof cannot be replayed across distributors,
/// clusters, mints, or stale epochs.
const LEAF_DOMAIN: &[u8] = b"BOTANIKA_REWARD_LEAF_V1";

/// Verifies against the specific settlement's own `reward_delta_root`
/// (closes P1-RWD-04 "only the current root is stored, stale-proof handling
/// undefined"). Before this, verification always read
/// `reward_distributor.current_root` and rebuilt the leaf from
/// `reward_distributor.epoch_id` -- both mutate on every `update_root` call,
/// so a proof generated for settlement N became unverifiable the instant
/// settlement N+1 published, with no grace period or history at all.
///
/// `RewardSettlementState` already persists one account per settlement
/// (created in `update_root_handler`, never closed) -- PAD's own required
/// action for P1-RWD-04 explicitly allows "root history **or settlement
/// account**" as the fix, and the settlement account already existed for
/// an unrelated reason (P0-RWD-03). Binding `claim_reward` to a specific
/// `settlement_id` instead of "whatever `current_root` happens to be right
/// now" means a proof remains valid indefinitely (as long as that
/// settlement's account hasn't been closed for rent reclaim), not just
/// during a short grace window.
#[derive(Accounts)]
#[instruction(node_id_hash: [u8; 32], settlement_id: u64)]
pub struct ClaimReward<'info> {
    #[account(
        mut,
        seeds = [RewardDistributor::SEED],
        bump = reward_distributor.bump,
        constraint = !reward_distributor.is_paused @ RewardError::Paused,
    )]
    // Boxed: adding `settlement` pushed `try_accounts` for this struct over
    // the SBF VM's 4 KiB stack frame (confirmed via `cargo build-sbf`),
    // same fix as `finalize_claim.rs`'s identical comment.
    pub reward_distributor: Box<Account<'info, RewardDistributor>>,

    #[account(
        seeds = [RewardSettlementState::SEED, &settlement_id.to_le_bytes()],
        bump = settlement.bump,
        constraint = settlement.settlement_id == settlement_id @ RewardError::SettlementIdMismatch,
    )]
    pub settlement: Account<'info, RewardSettlementState>,

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
    settlement_id: u64,
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

    let leaf = compute_leaf(
        ctx.program_id,
        &reward_distributor.key(),
        &ctx.accounts.reward_mint.key(),
        settlement_id,
        &ctx.accounts.miner.key(),
        &node_id_hash,
        cumulative_amount,
    );

    if !merkle::verify_proof(ctx.accounts.settlement.reward_delta_root, leaf, proof) {
        return Err(RewardError::InvalidProof.into());
    }

    let amount_to_claim = cumulative_amount
        .checked_sub(claim_status.amount_claimed)
        .ok_or(RewardError::Overflow)?;

    claim_status.amount_claimed = cumulative_amount;
    claim_status.last_claim = Clock::get()?.unix_timestamp;

    reward_distributor.total_claimed = reward_distributor
        .total_claimed
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
        settlement_id,
        amount: amount_to_claim,
        cumulative_amount,
        timestamp: claim_status.last_claim,
    });

    Ok(())
}

/// Pure leaf-hashing helper, pulled out of the handler so the exact
/// production formula is directly unit-testable without a full Anchor
/// `Context` (see `tests` module below).
fn compute_leaf(
    program_id: &Pubkey,
    distributor: &Pubkey,
    reward_mint: &Pubkey,
    settlement_id: u64,
    miner: &Pubkey,
    node_id_hash: &[u8; 32],
    cumulative_amount: u64,
) -> [u8; 32] {
    keccak::hashv(&[
        LEAF_DOMAIN,
        program_id.as_ref(),
        distributor.as_ref(),
        reward_mint.as_ref(),
        &settlement_id.to_le_bytes(),
        miner.as_ref(),
        node_id_hash,
        &cumulative_amount.to_le_bytes(),
    ])
    .0
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Closes P1-RWD-04: a proof built for settlement N must remain valid
    /// forever (verified against settlement N's own immutable
    /// `reward_delta_root`), even after settlement N+1 publishes a
    /// completely different root -- there is no "current root" to go stale
    /// against anymore, since verification never reads a mutable field.
    #[test]
    fn proof_for_old_settlement_still_verifies_after_a_newer_settlement_exists() {
        let program_id = Pubkey::new_unique();
        let distributor = Pubkey::new_unique();
        let reward_mint = Pubkey::new_unique();
        let miner = Pubkey::new_unique();
        let node_id_hash = [7u8; 32];

        // Settlement 1 publishes first; its leaf *is* its root (single-leaf
        // tree, empty proof) -- this is "the old proof" a user holds.
        let old_settlement_id = 1u64;
        let old_cumulative_amount = 1_000u64;
        let old_leaf = compute_leaf(&program_id, &distributor, &reward_mint, old_settlement_id, &miner, &node_id_hash, old_cumulative_amount);
        let old_root = old_leaf;

        // Settlement 2 publishes afterward with an unrelated root -- this
        // is what used to make the old proof unverifiable under the
        // pre-fix `current_root`-based check.
        let new_settlement_id = 2u64;
        let new_cumulative_amount = 2_500u64;
        let new_leaf = compute_leaf(&program_id, &distributor, &reward_mint, new_settlement_id, &miner, &node_id_hash, new_cumulative_amount);
        let new_root = new_leaf;
        assert_ne!(old_root, new_root, "test setup sanity: the two settlements must have different roots");

        // The old proof, checked against settlement 1's own root, still
        // verifies -- this is the fix. (Checking it against settlement 2's
        // root, as the old code effectively did via `current_root`, is the
        // bug this replaces -- shown failing below.)
        assert!(merkle::verify_proof(old_root, old_leaf, vec![]));
        assert!(!merkle::verify_proof(new_root, old_leaf, vec![]), "old proof must not verify against a different settlement's root");
    }

    /// A relayer cannot reuse a valid proof from one settlement against a
    /// different settlement_id by just changing the instruction argument --
    /// settlement_id is baked into the leaf itself, so the leaf (and hence
    /// the proof) no longer matches.
    #[test]
    fn leaf_is_bound_to_its_settlement_id() {
        let program_id = Pubkey::new_unique();
        let distributor = Pubkey::new_unique();
        let reward_mint = Pubkey::new_unique();
        let miner = Pubkey::new_unique();
        let node_id_hash = [3u8; 32];
        let cumulative_amount = 500u64;

        let leaf_settlement_1 = compute_leaf(&program_id, &distributor, &reward_mint, 1, &miner, &node_id_hash, cumulative_amount);
        let leaf_settlement_2 = compute_leaf(&program_id, &distributor, &reward_mint, 2, &miner, &node_id_hash, cumulative_amount);

        assert_ne!(leaf_settlement_1, leaf_settlement_2);
    }
}
