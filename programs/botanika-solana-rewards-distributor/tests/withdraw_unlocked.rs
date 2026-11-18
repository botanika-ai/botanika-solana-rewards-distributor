use anchor_lang::prelude::*;
use anchor_lang::{InstructionData, ToAccountMetas};
use solana_program_test::*;
use solana_sdk::{
    account::AccountSharedData,
    instruction::Instruction,
    pubkey::Pubkey,
    signature::{Keypair, Signer},
};

use botanika_solana_rewards_distributor::error::RewardError;
use botanika_solana_rewards_distributor::state::*;

mod utils;
use utils::*;

const LOCK_A: u64 = 150 * BONSAI;
const LOCK_B: u64 = 80 * BONSAI;

struct LockFixture {
    setup: SetupResult,
    beneficiary: Keypair,
    destination: Pubkey,
    lock_a: Pubkey,
    lock_b: Pubkey,
}

/// Two finalized claims (A = 150, B = 80 BONSAI) for one beneficiary, plus
/// a destination token account owned by someone else (e.g. the Botanika
/// treasury for VND payouts).
async fn lock_fixture() -> LockFixture {
    let mut setup = setup_test().await;
    prepare_finalize(&mut setup, 3, 1_000 * BONSAI).await;
    let beneficiary = Keypair::new();
    let node_id_hash = hash_node_id(TEST_NODE_ID);
    let lock_vault = setup.lock_vault.pubkey();

    let (ix_a, _, lock_a) =
        finalize_claim_via_escrow_ix(&setup, 1, beneficiary.pubkey(), node_id_hash, 7, LOCK_A, 3, lock_vault);
    let (ix_b, _, lock_b) =
        finalize_claim_via_escrow_ix(&setup, 2, beneficiary.pubkey(), node_id_hash, 7, LOCK_B, 3, lock_vault);
    setup.context.process_transaction(&[ix_a, ix_b], &[]).await.unwrap();

    let destination = Keypair::new();
    let treasury_owner = Pubkey::new_unique();
    let mint = setup.reward_mint.pubkey();
    setup.context.create_token_account(&destination, &mint, &treasury_owner).await.unwrap();

    LockFixture { setup, beneficiary, destination: destination.pubkey(), lock_a, lock_b }
}

async fn withdraw(
    f: &mut LockFixture,
    signer: &Keypair,
    locks: &[Pubkey],
    amounts: Vec<u64>,
) -> std::result::Result<(), BanksClientError> {
    let ix = withdraw_unlocked_ix(&f.setup, signer.pubkey(), f.destination, locks, amounts);
    f.setup.context.process_transaction(&[ix], &[signer]).await
}

#[tokio::test]
async fn test_withdraw_before_unlock_fails() {
    let mut f = lock_fixture().await;
    let beneficiary = Keypair::from_bytes(&f.beneficiary.to_bytes()).unwrap();
    // One second short of the 60-day lock.
    f.setup.context.warp_seconds(LOCK_DURATION_SECS - 1).await;
    let lock_a = f.lock_a;
    let result = withdraw(&mut f, &beneficiary, &[lock_a], vec![LOCK_A]).await;
    assert_program_error(result, RewardError::LockNotUnlocked);
}

#[tokio::test]
async fn test_withdraw_after_unlock_partial_then_remainder() {
    let mut f = lock_fixture().await;
    let beneficiary = Keypair::from_bytes(&f.beneficiary.to_bytes()).unwrap();
    let lock_vault = f.setup.lock_vault.pubkey();
    let (lock_a, lock_b) = (f.lock_a, f.lock_b);

    let result = withdraw(&mut f, &beneficiary, &[lock_a], vec![100 * BONSAI]).await;
    assert_program_error(result, RewardError::LockNotUnlocked);

    f.setup.context.warp_seconds(LOCK_DURATION_SECS).await;

    // Partial: 100 of A's 150.
    withdraw(&mut f, &beneficiary, &[lock_a], vec![100 * BONSAI]).await.unwrap();
    assert_eq!(f.setup.context.token_balance(f.destination).await, 100 * BONSAI);
    assert_eq!(f.setup.context.token_balance(lock_vault).await, LOCK_A + LOCK_B - 100 * BONSAI);
    assert_eq!(f.setup.context.claim_lock(lock_a).await.withdrawn, 100 * BONSAI);
    assert_eq!(
        f.setup.context.reward_distributor(f.setup.reward_distributor_pda).await.total_lock_withdrawn,
        100 * BONSAI
    );

    // Remainder of A (50) + all of B (80) in one instruction.
    withdraw(&mut f, &beneficiary, &[lock_a, lock_b], vec![50 * BONSAI, LOCK_B]).await.unwrap();
    assert_eq!(f.setup.context.token_balance(f.destination).await, LOCK_A + LOCK_B);
    assert_eq!(f.setup.context.token_balance(lock_vault).await, 0);
    let a = f.setup.context.claim_lock(lock_a).await;
    let b = f.setup.context.claim_lock(lock_b).await;
    assert_eq!(a.withdrawn, a.amount);
    assert_eq!(b.withdrawn, b.amount);
    let distributor = f.setup.context.reward_distributor(f.setup.reward_distributor_pda).await;
    assert_eq!(distributor.total_lock_withdrawn, LOCK_A + LOCK_B);
    assert_eq!(distributor.total_locked, LOCK_A + LOCK_B);

    // Fully drained: any further amount exceeds what remains.
    let result = withdraw(&mut f, &beneficiary, &[lock_a, lock_b], vec![60 * BONSAI, 60 * BONSAI]).await;
    assert_program_error(result, RewardError::LockAmountExceeded);
}

#[tokio::test]
async fn test_withdraw_over_remaining_fails() {
    let mut f = lock_fixture().await;
    let beneficiary = Keypair::from_bytes(&f.beneficiary.to_bytes()).unwrap();
    f.setup.context.warp_seconds(LOCK_DURATION_SECS).await;
    let lock_a = f.lock_a;
    let result = withdraw(&mut f, &beneficiary, &[lock_a], vec![LOCK_A + 1]).await;
    assert_program_error(result, RewardError::LockAmountExceeded);
}

#[tokio::test]
async fn test_withdraw_zero_amount_fails() {
    let mut f = lock_fixture().await;
    let beneficiary = Keypair::from_bytes(&f.beneficiary.to_bytes()).unwrap();
    f.setup.context.warp_seconds(LOCK_DURATION_SECS).await;
    let (lock_a, lock_b) = (f.lock_a, f.lock_b);
    let result = withdraw(&mut f, &beneficiary, &[lock_a, lock_b], vec![LOCK_A, 0]).await;
    assert_program_error(result, RewardError::InvalidAmount);
}

#[tokio::test]
async fn test_withdraw_wrong_signer_fails() {
    let mut f = lock_fixture().await;
    f.setup.context.warp_seconds(LOCK_DURATION_SECS).await;
    let intruder = Keypair::new();
    let lock_a = f.lock_a;
    let result = withdraw(&mut f, &intruder, &[lock_a], vec![LOCK_A]).await;
    assert_program_error(result, RewardError::Unauthorized);
    assert_eq!(f.setup.context.token_balance(f.destination).await, 0);
}

#[tokio::test]
async fn test_withdraw_below_minimum_fails() {
    let mut f = lock_fixture().await;
    let beneficiary = Keypair::from_bytes(&f.beneficiary.to_bytes()).unwrap();
    f.setup.context.warp_seconds(LOCK_DURATION_SECS).await;
    let lock_a = f.lock_a;
    let result = withdraw(&mut f, &beneficiary, &[lock_a], vec![MIN_LOCK_WITHDRAW - 1]).await;
    assert_program_error(result, RewardError::BelowMinimumWithdraw);
    // Exactly the minimum passes.
    withdraw(&mut f, &beneficiary, &[lock_a], vec![MIN_LOCK_WITHDRAW]).await.unwrap();
    assert_eq!(f.setup.context.token_balance(f.destination).await, MIN_LOCK_WITHDRAW);
}

#[tokio::test]
async fn test_withdraw_duplicate_lock_fails() {
    let mut f = lock_fixture().await;
    let beneficiary = Keypair::from_bytes(&f.beneficiary.to_bytes()).unwrap();
    f.setup.context.warp_seconds(LOCK_DURATION_SECS).await;
    let lock_a = f.lock_a;
    let result = withdraw(&mut f, &beneficiary, &[lock_a, lock_a], vec![75 * BONSAI, 75 * BONSAI]).await;
    assert_program_error(result, RewardError::DuplicateClaimLock);
}

#[tokio::test]
async fn test_withdraw_amounts_length_mismatch_fails() {
    let mut f = lock_fixture().await;
    let beneficiary = Keypair::from_bytes(&f.beneficiary.to_bytes()).unwrap();
    f.setup.context.warp_seconds(LOCK_DURATION_SECS).await;
    let (lock_a, lock_b) = (f.lock_a, f.lock_b);
    let result = withdraw(&mut f, &beneficiary, &[lock_a, lock_b], vec![LOCK_A]).await;
    assert_program_error(result, RewardError::InvalidRemainingAccounts);
}

#[tokio::test]
async fn test_withdraw_too_many_locks_fails() {
    let mut f = lock_fixture().await;
    let beneficiary = Keypair::from_bytes(&f.beneficiary.to_bytes()).unwrap();
    let locks: Vec<Pubkey> = (0..17).map(|_| Pubkey::new_unique()).collect();
    let result = withdraw(&mut f, &beneficiary, &locks, vec![BONSAI; 17]).await;
    assert_program_error(result, RewardError::TooManyLocks);
}

/// A program-owned account carrying a copy of a real ClaimLock's data at a
/// non-PDA address must be rejected (prevents forging a second lock).
#[tokio::test]
async fn test_withdraw_forged_lock_fails() {
    let mut f = lock_fixture().await;
    let beneficiary = Keypair::from_bytes(&f.beneficiary.to_bytes()).unwrap();
    f.setup.context.warp_seconds(LOCK_DURATION_SECS).await;

    let real = f.setup.context.banks_client.get_account(f.lock_a).await.unwrap().unwrap();
    let forged = Pubkey::new_unique();
    let program_id = f.setup.context.program_id;
    f.setup.context.set_account(
        &forged,
        &AccountSharedData::from(solana_sdk::account::Account {
            lamports: real.lamports,
            data: real.data.clone(),
            owner: program_id,
            executable: false,
            rent_epoch: 0,
        }),
    );
    let result = withdraw(&mut f, &beneficiary, &[forged], vec![LOCK_A]).await;
    assert_program_error(result, RewardError::InvalidClaimLock);
}

#[tokio::test]
async fn test_withdraw_while_paused_fails() {
    let mut f = lock_fixture().await;
    let beneficiary = Keypair::from_bytes(&f.beneficiary.to_bytes()).unwrap();
    f.setup.context.warp_seconds(LOCK_DURATION_SECS).await;

    let authority = Keypair::from_bytes(&f.setup.authority.to_bytes()).unwrap();
    let pause_ix = Instruction {
        program_id: f.setup.context.program_id,
        accounts: botanika_solana_rewards_distributor::accounts::Pause {
            reward_distributor: f.setup.reward_distributor_pda,
            pause_authority: authority.pubkey(),
        }
        .to_account_metas(None),
        data: botanika_solana_rewards_distributor::instruction::Pause {}.data(),
    };
    f.setup.context.process_transaction(&[pause_ix], &[&authority]).await.unwrap();

    let lock_a = f.lock_a;
    let result = withdraw(&mut f, &beneficiary, &[lock_a], vec![LOCK_A]).await;
    assert_program_error(result, RewardError::Paused);
}

#[tokio::test]
async fn test_withdraw_wrong_lock_vault_fails() {
    let mut f = lock_fixture().await;
    let beneficiary = Keypair::from_bytes(&f.beneficiary.to_bytes()).unwrap();
    f.setup.context.warp_seconds(LOCK_DURATION_SECS).await;
    let mut ix = withdraw_unlocked_ix(&f.setup, beneficiary.pubkey(), f.destination, &[f.lock_a], vec![LOCK_A]);
    // accounts[2] is lock_vault; point it at token_vault instead.
    ix.accounts[2].pubkey = f.setup.token_vault.pubkey();
    let result = f.setup.context.process_transaction(&[ix], &[&beneficiary]).await;
    assert_program_error(result, RewardError::InvalidLockVault);
}

#[tokio::test]
async fn test_withdraw_to_lock_vault_fails() {
    let mut f = lock_fixture().await;
    let beneficiary = Keypair::from_bytes(&f.beneficiary.to_bytes()).unwrap();
    f.setup.context.warp_seconds(LOCK_DURATION_SECS).await;
    let lock_vault = f.setup.lock_vault.pubkey();
    let ix = withdraw_unlocked_ix(&f.setup, beneficiary.pubkey(), lock_vault, &[f.lock_a], vec![LOCK_A]);
    let result = f.setup.context.process_transaction(&[ix], &[&beneficiary]).await;
    assert_program_error(result, RewardError::InvalidWithdrawDestination);
}

#[tokio::test]
async fn test_withdraw_to_token_vault_fails() {
    let mut f = lock_fixture().await;
    let beneficiary = Keypair::from_bytes(&f.beneficiary.to_bytes()).unwrap();
    f.setup.context.warp_seconds(LOCK_DURATION_SECS).await;
    let token_vault = f.setup.token_vault.pubkey();
    let ix = withdraw_unlocked_ix(&f.setup, beneficiary.pubkey(), token_vault, &[f.lock_a], vec![LOCK_A]);
    let result = f.setup.context.process_transaction(&[ix], &[&beneficiary]).await;
    assert_program_error(result, RewardError::InvalidWithdrawDestination);
}

/// `withdraw_vault` (treasury sweep) must never be able to move locked funds.
#[tokio::test]
async fn test_withdraw_vault_cannot_drain_lock_vault() {
    let mut f = lock_fixture().await;
    let authority = Keypair::from_bytes(&f.setup.authority.to_bytes()).unwrap();
    let lock_vault = f.setup.lock_vault.pubkey();
    let treasury = Keypair::new();
    let mint = f.setup.reward_mint.pubkey();
    f.setup.context.create_token_account(&treasury, &mint, &authority.pubkey()).await.unwrap();

    let ix = Instruction {
        program_id: f.setup.context.program_id,
        accounts: botanika_solana_rewards_distributor::accounts::WithdrawVault {
            reward_distributor: f.setup.reward_distributor_pda,
            token_vault: lock_vault,
            reward_mint: mint,
            treasury: treasury.pubkey(),
            treasury_authority: authority.pubkey(),
            token_program: anchor_spl::token::ID,
        }
        .to_account_metas(None),
        data: botanika_solana_rewards_distributor::instruction::WithdrawVault { amount: u64::MAX }.data(),
    };
    let result = f.setup.context.process_transaction(&[ix], &[&authority]).await;
    // The explicit `!= lock_vault` guard fires (Anchor evaluates `constraint`
    // before `address`); `address == token_vault` would reject it as well.
    assert_program_error(result, RewardError::InvalidLockVault);
    assert_eq!(f.setup.context.token_balance(lock_vault).await, LOCK_A + LOCK_B);
    assert_eq!(f.setup.context.token_balance(treasury.pubkey()).await, 0);
}
