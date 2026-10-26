// Previously zero test coverage: `BatchRecord` accounts were created once
// per `batch_id` (`batch_payout.rs`) and never closed by any instruction at
// all -- a real rent/account-growth gap, closed this session.
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

/// Seeds a `BatchRecord` PDA directly (bypassing `batch_payout`, same
/// unreachable-without-a-full-token-setup-and-payout-flow convenience
/// pattern as `close_settlement.rs`'s `seed_settlement`) with an explicit
/// `processed_at`.
async fn seed_batch_record(ctx: &mut TestContext, batch_id: u64, processed_at: i64) -> Pubkey {
    let (pda, bump) = Pubkey::find_program_address(&[BatchRecord::SEED, &batch_id.to_le_bytes()], &ctx.program_id);
    let record = BatchRecord {
        batch_id,
        item_count: 1,
        total_amount: 100,
        executed_by: Pubkey::new_unique(),
        processed_at,
        bump,
    };
    let mut bytes = Vec::new();
    record.try_serialize(&mut bytes).unwrap();
    let rent = ctx.banks_client.get_rent().await.unwrap();
    let lamports = rent.minimum_balance(bytes.len());
    let shared = AccountSharedData::create(lamports, bytes, ctx.program_id, false, 0);
    ctx.set_account(&pda, &shared);
    pda
}

fn close_batch_record_ix(setup: &SetupResult, batch_record: Pubkey, admin_authority: Pubkey) -> Instruction {
    Instruction {
        program_id: setup.context.program_id,
        accounts: botanika_solana_rewards_distributor::accounts::CloseBatchRecord {
            reward_distributor: setup.reward_distributor_pda,
            batch_record,
            admin_authority,
        }
        .to_account_metas(None),
        data: botanika_solana_rewards_distributor::instruction::CloseBatchRecord {}.data(),
    }
}

#[tokio::test]
async fn test_close_batch_record_rejects_too_recent() {
    let mut setup = setup_test().await;
    let now = setup.context.banks_client.get_sysvar::<Clock>().await.unwrap().unix_timestamp;
    let batch_record = seed_batch_record(&mut setup.context, 1, now).await;

    let ix = close_batch_record_ix(&setup, batch_record, setup.authority.pubkey());
    let result = setup.context.process_transaction(&[ix], &[&setup.authority]).await;
    assert!(result.is_err(), "a batch_record younger than MIN_SETTLEMENT_RETENTION_SEC must be rejected");

    let account = setup.context.banks_client.get_account(batch_record).await.unwrap();
    assert!(account.is_some(), "a rejected close attempt must not have closed the account");
}

#[tokio::test]
async fn test_close_batch_record_rejects_wrong_signer() {
    let mut setup = setup_test().await;
    let now = setup.context.banks_client.get_sysvar::<Clock>().await.unwrap().unix_timestamp;
    let batch_record = seed_batch_record(&mut setup.context, 1, now - MIN_SETTLEMENT_RETENTION_SEC - 1).await;

    let impostor = Keypair::new();
    let fund_ix = solana_sdk::system_instruction::transfer(&setup.context.payer.pubkey(), &impostor.pubkey(), 10_000_000);
    setup.context.process_transaction(&[fund_ix], &[]).await.unwrap();

    let ix = close_batch_record_ix(&setup, batch_record, impostor.pubkey());
    let result = setup.context.process_transaction(&[ix], &[&impostor]).await;
    assert!(result.is_err(), "a non-admin_authority signer must be rejected");
}

#[tokio::test]
async fn test_close_batch_record_succeeds_after_retention_window() {
    let mut setup = setup_test().await;
    let now = setup.context.banks_client.get_sysvar::<Clock>().await.unwrap().unix_timestamp;
    let batch_record = seed_batch_record(&mut setup.context, 1, now - MIN_SETTLEMENT_RETENTION_SEC - 1).await;

    let ix = close_batch_record_ix(&setup, batch_record, setup.authority.pubkey());
    setup.context.process_transaction(&[ix], &[&setup.authority]).await.unwrap();

    let account = setup.context.banks_client.get_account(batch_record).await.unwrap();
    assert!(account.is_none(), "the account must be closed (rent reclaimed) once old enough");
}
