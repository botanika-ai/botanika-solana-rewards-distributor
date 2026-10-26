// `finalize_claim` is only ever invoked via a real MagicBlock Magic Action
// (the `escrow` account's `address` constraint derives a PDA with no
// corresponding private key -- `solana-program-test` cannot produce a
// genuine signature for it, same documented boundary as
// `botanika-magicblock-contracts`'s LiteSVM test suite). This file only
// covers what resolves BEFORE that boundary: the new `settlement` account
// requirement, validated during Anchor's `try_accounts` in field
// declaration order -- `settlement` is the second field (right after
// `reward_distributor`), so a missing settlement account fails here,
// before `claim_receipt`/`beneficiary_token_account`/`caller_program`/
// `escrow_auth`/`escrow` (the unreachable-without-a-real-CPI fields) are
// ever resolved.
use anchor_lang::prelude::*;
use anchor_lang::{InstructionData, ToAccountMetas};
use solana_program_test::*;
use solana_sdk::{instruction::Instruction, pubkey::Pubkey, signature::{Keypair, Signer}, system_program};

use botanika_solana_rewards_distributor::state::*;

mod utils;
use utils::*;

#[allow(clippy::too_many_arguments)]
fn finalize_claim_ix(
    setup: &SetupResult,
    claim_nonce: u64,
    beneficiary: Pubkey,
    node_id_hash: [u8; 32],
    wallet_binding_id: u64,
    amount: u64,
    settlement_id: u64,
    escrow: &Keypair,
) -> Instruction {
    let settlement = settlement_pda(settlement_id, &setup.context.program_id);
    let (claim_receipt, _) = Pubkey::find_program_address(
        &[
            ClaimReceipt::SEED,
            beneficiary.as_ref(),
            &node_id_hash,
            &wallet_binding_id.to_le_bytes(),
            &claim_nonce.to_le_bytes(),
        ],
        &setup.context.program_id,
    );
    // Placeholders: never reached -- the settlement check (2nd field)
    // fails first. `caller_program`/`escrow_auth` can be arbitrary
    // (`UncheckedAccount`, no signer requirement). `escrow` has a `signer`
    // constraint, so the transaction needs a real keypair to co-sign it
    // even though the settlement failure means its `address` constraint
    // (a PDA with no real private key -- see module doc comment) is never
    // actually evaluated.
    let placeholder = Pubkey::new_unique();

    Instruction {
        program_id: setup.context.program_id,
        accounts: botanika_solana_rewards_distributor::accounts::FinalizeClaim {
            reward_distributor: setup.reward_distributor_pda,
            settlement,
            claim_receipt,
            beneficiary_token_account: setup.miner_token_account.pubkey(),
            token_vault: setup.token_vault.pubkey(),
            reward_mint: setup.reward_mint.pubkey(),
            token_program: anchor_spl::token::ID,
            system_program: system_program::ID,
            caller_program: placeholder,
            escrow_auth: placeholder,
            escrow: escrow.pubkey(),
        }
        .to_account_metas(None),
        data: botanika_solana_rewards_distributor::instruction::FinalizeClaim {
            claim_nonce,
            beneficiary,
            node_id_hash,
            wallet_binding_id,
            amount,
            reward_state_hash: [0u8; 32],
            settlement_id,
        }
        .data(),
    }
}

/// Regression guard for the real gap found reviewing this flow:
/// `finalize_claim` previously accepted `settlement_id` as a bare argument,
/// stored on `ClaimReceipt` for audit, but checked against nothing -- a
/// caller (or compromised `commit_reward_balance` on the ER side) could
/// reference a `settlement_id` that was never actually published via
/// `update_root` at all. Mirrors `claim_reward.rs`'s identical `settlement`
/// requirement exactly.
#[tokio::test]
async fn test_finalize_claim_rejects_nonexistent_settlement() {
    let setup = setup_test().await;
    let beneficiary = Pubkey::new_unique();
    let escrow = Keypair::new();

    // No update_root/set_reward_distributor_root call -- settlement_id=1
    // has never been published, so its RewardSettlementState PDA does not
    // exist.
    let ix = finalize_claim_ix(&setup, 1, beneficiary, [1u8; 32], 1, 40, 1, &escrow);

    let mut ctx = setup.context;
    let result = ctx.process_transaction(&[ix], &[&escrow]).await;
    assert!(result.is_err(), "a settlement_id that was never published via update_root must be rejected");
}
