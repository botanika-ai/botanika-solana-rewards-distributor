// `finalize_claim` is only ever invoked via a real MagicBlock Magic Action:
// the `escrow` account's `address` constraint derives a PDA of the
// delegation program with no corresponding private key. These tests run it
// through `utils::mock_delegation_entry`, a test-only program registered at
// the delegation program id that `invoke_signed`s the instruction with the
// escrow PDA's seeds -- the same signature a real Magic Action supplies.
use anchor_lang::prelude::*;
use anchor_lang::{InstructionData, ToAccountMetas};
use solana_program_test::*;
use solana_sdk::{instruction::Instruction, pubkey::Pubkey, signature::{Keypair, Signer}, system_program};

use botanika_solana_rewards_distributor::error::RewardError;
use botanika_solana_rewards_distributor::state::*;

mod utils;
use utils::*;

/// Regression guard for the real gap found reviewing this flow:
/// `finalize_claim` previously accepted `settlement_id` as a bare argument,
/// stored on `ClaimReceipt` for audit, but checked against nothing -- a
/// caller (or compromised `commit_reward_balance` on the ER side) could
/// reference a `settlement_id` that was never actually published via
/// `update_root` at all. Mirrors `claim_reward.rs`'s identical `settlement`
/// requirement exactly. Sent directly (no escrow signature): the settlement
/// check (2nd field) fails before the escrow constraints are evaluated.
#[tokio::test]
async fn test_finalize_claim_rejects_nonexistent_settlement() {
    let setup = setup_test().await;
    let program_id = setup.context.program_id;
    let beneficiary = Pubkey::new_unique();
    let escrow = Keypair::new();
    let node_id_hash = [1u8; 32];

    // No settlement published -- settlement_id=1's PDA does not exist.
    let claim_receipt = claim_receipt_pda(&program_id, &beneficiary, &node_id_hash, 1, 1);
    let placeholder = Pubkey::new_unique();
    let ix = Instruction {
        program_id,
        accounts: botanika_solana_rewards_distributor::accounts::FinalizeClaim {
            reward_distributor: setup.reward_distributor_pda,
            settlement: settlement_pda(1, &program_id),
            claim_receipt,
            claim_lock: claim_lock_pda(&program_id, &claim_receipt),
            token_vault: setup.token_vault.pubkey(),
            lock_vault: setup.lock_vault.pubkey(),
            reward_mint: setup.reward_mint.pubkey(),
            token_program: anchor_spl::token::ID,
            system_program: system_program::ID,
            caller_program: placeholder,
            escrow_auth: placeholder,
            escrow: escrow.pubkey(),
        }
        .to_account_metas(None),
        data: botanika_solana_rewards_distributor::instruction::FinalizeClaim {
            claim_nonce: 1,
            beneficiary,
            node_id_hash,
            wallet_binding_id: 1,
            amount: 40,
            reward_state_hash: [0u8; 32],
            settlement_id: 1,
        }
        .data(),
    };

    let mut ctx = setup.context;
    let result = ctx.process_transaction(&[ix], &[&escrow]).await;
    assert!(result.is_err(), "a settlement_id that was never published via update_root must be rejected");
}

#[tokio::test]
async fn test_finalize_claim_locks_into_lock_vault() {
    let mut setup = setup_test().await;
    prepare_finalize(&mut setup, 5, 1_000 * BONSAI).await;
    let program_id = setup.context.program_id;
    let beneficiary = Pubkey::new_unique();
    let node_id_hash = hash_node_id(TEST_NODE_ID);
    let amount = 150 * BONSAI;

    let vault_before = setup.context.token_balance(setup.token_vault.pubkey()).await;
    let lock_vault = setup.lock_vault.pubkey();
    let (ix, claim_receipt, claim_lock) =
        finalize_claim_via_escrow_ix(&setup, 9, beneficiary, node_id_hash, 42, amount, 5, lock_vault);
    let now = setup.context.now().await;
    setup.context.process_transaction(&[ix], &[]).await.unwrap();

    // Tokens moved token_vault -> lock_vault; beneficiary is never paid here.
    assert_eq!(setup.context.token_balance(setup.token_vault.pubkey()).await, vault_before - amount);
    assert_eq!(setup.context.token_balance(lock_vault).await, amount);

    assert_eq!(claim_lock, claim_lock_pda(&program_id, &claim_receipt));
    let lock = setup.context.claim_lock(claim_lock).await;
    assert_eq!(lock.beneficiary, beneficiary);
    assert_eq!(lock.node_id_hash, node_id_hash);
    assert_eq!(lock.wallet_binding_id, 42);
    assert_eq!(lock.claim_nonce, 9);
    assert_eq!(lock.settlement_id, 5);
    assert_eq!(lock.amount, amount);
    assert_eq!(lock.withdrawn, 0);
    assert_eq!(lock.locked_at, now);
    assert_eq!(lock.unlock_at, now + LOCK_DURATION_SECS);
    assert_eq!(LOCK_DURATION_SECS, 5_184_000);
    assert_eq!(lock.claim_receipt, claim_receipt);

    let receipt_account = setup.context.banks_client.get_account(claim_receipt).await.unwrap().unwrap();
    let receipt = ClaimReceipt::try_deserialize(&mut receipt_account.data.as_slice()).unwrap();
    assert_eq!(receipt.amount, amount);
    assert_eq!(receipt.beneficiary, beneficiary);
    assert_eq!(receipt.paid_at, now);

    let distributor = setup.context.reward_distributor(setup.reward_distributor_pda).await;
    assert_eq!(distributor.total_locked, amount);
    assert_eq!(distributor.total_lock_withdrawn, 0);

    // Replaying the same claim tuple must fail (ClaimReceipt already exists).
    setup.context.warp_seconds(1).await;
    let (replay, _, _) =
        finalize_claim_via_escrow_ix(&setup, 9, beneficiary, node_id_hash, 42, amount, 5, lock_vault);
    assert!(setup.context.process_transaction(&[replay], &[]).await.is_err());
    assert_eq!(setup.context.token_balance(lock_vault).await, amount);
}

#[tokio::test]
async fn test_finalize_claim_rejects_wrong_lock_vault() {
    let mut setup = setup_test().await;
    prepare_finalize(&mut setup, 5, 1_000 * BONSAI).await;
    let wrong_lock_vault = Keypair::new();
    let mint = setup.reward_mint.pubkey();
    let pda = setup.reward_distributor_pda;
    setup.context.create_token_account(&wrong_lock_vault, &mint, &pda).await.unwrap();

    let (ix, _, _) = finalize_claim_via_escrow_ix(
        &setup,
        1,
        Pubkey::new_unique(),
        hash_node_id(TEST_NODE_ID),
        1,
        150 * BONSAI,
        5,
        wrong_lock_vault.pubkey(),
    );
    let result = setup.context.process_transaction(&[ix], &[]).await;
    assert_program_error(result, RewardError::InvalidLockVault);
}
