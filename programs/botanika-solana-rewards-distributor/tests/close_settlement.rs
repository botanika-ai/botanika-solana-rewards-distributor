// Previously zero test coverage: `RewardSettlementState` accounts were
// created once per `update_root` call and never closed by any instruction
// at all -- a real rent/account-growth gap, closed this session.
use anchor_lang::prelude::*;
use anchor_lang::{InstructionData, ToAccountMetas};
use solana_program_test::*;
use solana_sdk::{
    account::{AccountSharedData, WritableAccount},
    instruction::Instruction,
    pubkey::Pubkey,
    signature::{Keypair, Signer},
};

use botanika_solana_rewards_distributor::instructions::MIN_SETTLEMENT_RETENTION_SEC;
use botanika_solana_rewards_distributor::state::*;

mod utils;
use utils::*;

/// Seeds a `RewardSettlementState` PDA directly (bypassing `update_root`,
/// same unreachable-without-a-real-Magic-Action boundary documented
/// throughout this suite) with an explicit `settled_at`, so both the
/// "too recent" and "old enough" cases are independently testable.
async fn seed_settlement(ctx: &mut TestContext, settlement_id: u64, settled_at: i64) -> Pubkey {
    let (pda, bump) = Pubkey::find_program_address(&[RewardSettlementState::SEED, &settlement_id.to_le_bytes()], &ctx.program_id);
    let settlement = RewardSettlementState {
        settlement_id,
        epoch_from: settlement_id,
        epoch_to: settlement_id,
        proof_commitment: [0u8; 32],
        policy_hash: [0u8; 32],
        canonical_ledger_hash: [0u8; 32],
        revision_no: 0,
        reward_delta_root: [0u8; 32],
        leaf_count: 0,
        total_liability: 0,
        settled_at,
        bump,
    };
    let mut bytes = Vec::new();
    settlement.try_serialize(&mut bytes).unwrap();
    let rent = ctx.banks_client.get_rent().await.unwrap();
    let lamports = rent.minimum_balance(bytes.len());
    let shared = AccountSharedData::create(lamports, bytes, ctx.program_id, false, 0);
    ctx.set_account(&pda, &shared);
    pda
}

fn close_settlement_ix(setup: &SetupResult, settlement: Pubkey, admin_authority: Pubkey) -> Instruction {
    Instruction {
        program_id: setup.context.program_id,
        accounts: botanika_solana_rewards_distributor::accounts::CloseSettlement {
            reward_distributor: setup.reward_distributor_pda,
            settlement,
            admin_authority,
        }
        .to_account_metas(None),
        data: botanika_solana_rewards_distributor::instruction::CloseSettlement {}.data(),
    }
}

#[tokio::test]
async fn test_close_settlement_rejects_too_recent() {
    let mut setup = setup_test().await;
    let now = setup.context.banks_client.get_sysvar::<Clock>().await.unwrap().unix_timestamp;
    let settlement = seed_settlement(&mut setup.context, 1, now).await;

    let ix = close_settlement_ix(&setup, settlement, setup.authority.pubkey());
    let result = setup.context.process_transaction(&[ix], &[&setup.authority]).await;
    assert!(result.is_err(), "a settlement younger than MIN_SETTLEMENT_RETENTION_SEC must be rejected");

    // The account must still exist (not closed) after a rejected attempt.
    let account = setup.context.banks_client.get_account(settlement).await.unwrap();
    assert!(account.is_some(), "a rejected close attempt must not have closed the account");
}

#[tokio::test]
async fn test_close_settlement_rejects_wrong_signer() {
    let mut setup = setup_test().await;
    let now = setup.context.banks_client.get_sysvar::<Clock>().await.unwrap().unix_timestamp;
    let settlement = seed_settlement(&mut setup.context, 1, now - MIN_SETTLEMENT_RETENTION_SEC - 1).await;

    let impostor = Keypair::new();
    let fund_ix = solana_sdk::system_instruction::transfer(&setup.context.payer.pubkey(), &impostor.pubkey(), 10_000_000);
    setup.context.process_transaction(&[fund_ix], &[]).await.unwrap();

    let ix = close_settlement_ix(&setup, settlement, impostor.pubkey());
    let result = setup.context.process_transaction(&[ix], &[&impostor]).await;
    assert!(result.is_err(), "a non-admin_authority signer must be rejected");
}

#[tokio::test]
async fn test_close_settlement_succeeds_after_retention_window() {
    let mut setup = setup_test().await;
    let now = setup.context.banks_client.get_sysvar::<Clock>().await.unwrap().unix_timestamp;
    let settlement = seed_settlement(&mut setup.context, 1, now - MIN_SETTLEMENT_RETENTION_SEC - 1).await;

    let ix = close_settlement_ix(&setup, settlement, setup.authority.pubkey());
    setup.context.process_transaction(&[ix], &[&setup.authority]).await.unwrap();

    let account = setup.context.banks_client.get_account(settlement).await.unwrap();
    assert!(account.is_none(), "the account must be closed (rent reclaimed) once old enough");
}
