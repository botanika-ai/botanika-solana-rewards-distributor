use anchor_lang::InstructionData;
use anchor_lang::ToAccountMetas;
use anchor_lang::prelude::*;
use solana_program_test::*;
use solana_sdk::{
    signature::{Keypair, Signer},
    system_program,
    instruction::Instruction,
};
use botanika_solana_rewards_distributor::instructions::AuthorityRole;
use botanika_solana_rewards_distributor::state::*;

mod utils;
use utils::*;

#[tokio::test]
async fn test_set_authority_rotates_single_role() {
    let mut setup = setup_test().await;
    let program_id = setup.context.program_id;
    let new_root_authority = Keypair::new();

    let fund_ix = solana_sdk::system_instruction::transfer(
        &setup.context.payer.pubkey(),
        &new_root_authority.pubkey(),
        10_000_000,
    );
    setup.context.process_transaction(&[fund_ix], &[]).await.unwrap();

    let set_authority_ix = Instruction {
        program_id,
        accounts: botanika_solana_rewards_distributor::accounts::SetAuthority {
            reward_distributor: setup.reward_distributor_pda,
            admin_authority: setup.authority.pubkey(),
        }.to_account_metas(None),
        data: botanika_solana_rewards_distributor::instruction::SetAuthority {
            role: AuthorityRole::Root,
            new_authority: new_root_authority.pubkey(),
        }.data(),
    };

    setup.context.process_transaction(&[set_authority_ix], &[&setup.authority]).await.unwrap();

    // Verify only the Root role moved — the other roles are untouched (P0-RWD-02).
    let account = setup.context.banks_client.get_account(setup.reward_distributor_pda).await.unwrap().unwrap();
    let data = RewardDistributor::try_deserialize(&mut account.data.as_slice()).unwrap();
    assert_eq!(data.root_authority, new_root_authority.pubkey());
    assert_eq!(data.admin_authority, setup.authority.pubkey());
    assert_eq!(data.payout_authority, setup.authority.pubkey());
    assert_eq!(data.pause_authority, setup.authority.pubkey());
    assert_eq!(data.treasury_authority, setup.authority.pubkey());

    // The old root_authority can no longer publish roots.
    let stale_root_ix = update_root_ix(&setup, [1u8; 32], 1, dummy_settlement(1, 0, 0));
    let stale_result = setup.context.process_transaction(&[stale_root_ix], &[&setup.authority]).await;
    assert!(stale_result.is_err());

    // The new root_authority can.
    let fresh_root_ix = Instruction {
        program_id,
        accounts: botanika_solana_rewards_distributor::accounts::UpdateRoot {
            reward_distributor: setup.reward_distributor_pda,
            settlement: settlement_pda(1, &program_id),
            root_authority: new_root_authority.pubkey(),
            system_program: system_program::ID,
        }.to_account_metas(None),
        data: botanika_solana_rewards_distributor::instruction::UpdateRoot {
            new_root: [1u8; 32],
            settlement: dummy_settlement(1, 0, 0),
        }.data(),
    };
    setup.context.process_transaction(&[fresh_root_ix], &[&new_root_authority]).await.unwrap();
}

#[tokio::test]
async fn test_set_authority_unauthorized_fails() {
    let mut setup = setup_test().await;
    let program_id = setup.context.program_id;
    let wrong_admin = Keypair::new();

    let set_authority_ix = Instruction {
        program_id,
        accounts: botanika_solana_rewards_distributor::accounts::SetAuthority {
            reward_distributor: setup.reward_distributor_pda,
            admin_authority: wrong_admin.pubkey(),
        }.to_account_metas(None),
        data: botanika_solana_rewards_distributor::instruction::SetAuthority {
            role: AuthorityRole::Payout,
            new_authority: Keypair::new().pubkey(),
        }.data(),
    };

    let result = setup.context.process_transaction(&[set_authority_ix], &[&wrong_admin]).await;
    assert!(result.is_err());
}

#[tokio::test]
async fn test_pause_unpause() {
    let mut setup = setup_test().await;
    let program_id = setup.context.program_id;

    // 1. Pause
    let pause_ix = Instruction {
        program_id,
        accounts: botanika_solana_rewards_distributor::accounts::Pause {
            reward_distributor: setup.reward_distributor_pda,
            pause_authority: setup.authority.pubkey(),
        }.to_account_metas(None),
        data: botanika_solana_rewards_distributor::instruction::Pause {}.data(),
    };
    setup.context.process_transaction(&[pause_ix], &[&setup.authority]).await.unwrap();

    let account = setup.context.banks_client.get_account(setup.reward_distributor_pda).await.unwrap().unwrap();
    let data = RewardDistributor::try_deserialize(&mut account.data.as_slice()).unwrap();
    assert_eq!(data.is_paused, true);

    // 2. Unpause
    let unpause_ix = Instruction {
        program_id,
        accounts: botanika_solana_rewards_distributor::accounts::Unpause {
            reward_distributor: setup.reward_distributor_pda,
            pause_authority: setup.authority.pubkey(),
        }.to_account_metas(None),
        data: botanika_solana_rewards_distributor::instruction::Unpause {}.data(),
    };
    setup.context.process_transaction(&[unpause_ix], &[&setup.authority]).await.unwrap();

    let account = setup.context.banks_client.get_account(setup.reward_distributor_pda).await.unwrap().unwrap();
    let data = RewardDistributor::try_deserialize(&mut account.data.as_slice()).unwrap();
    assert_eq!(data.is_paused, false);
}

#[tokio::test]
async fn test_authority_checks() {
    let mut setup = setup_test().await;
    let program_id = setup.context.program_id;
    let wrong_authority = Keypair::new();

    // Try update_root with wrong root_authority
    let update_root_ix = Instruction {
        program_id,
        accounts: botanika_solana_rewards_distributor::accounts::UpdateRoot {
            reward_distributor: setup.reward_distributor_pda,
            settlement: settlement_pda(1, &program_id),
            root_authority: wrong_authority.pubkey(),
            system_program: system_program::ID,
        }.to_account_metas(None),
        data: botanika_solana_rewards_distributor::instruction::UpdateRoot {
            new_root: [1u8; 32],
            settlement: dummy_settlement(1, 0, 0),
        }.data(),
    };
    let result = setup.context.process_transaction(&[update_root_ix], &[&wrong_authority]).await;
    assert!(result.is_err());
}

#[tokio::test]
async fn test_withdraw_vault_success() {
    let mut setup = setup_test().await;
    let program_id = setup.context.program_id;

    // Create treasury token account owned by the authority
    let treasury_account = Keypair::new();
    setup.context.create_token_account(
        &treasury_account,
        &setup.reward_mint.pubkey(),
        &setup.authority.pubkey(),
    ).await.unwrap();

    // 1. Partial withdrawal: Withdraw 4000 out of 10000 tokens
    let withdraw_ix = Instruction {
        program_id,
        accounts: botanika_solana_rewards_distributor::accounts::WithdrawVault {
            reward_distributor: setup.reward_distributor_pda,
            token_vault: setup.token_vault.pubkey(),
            reward_mint: setup.reward_mint.pubkey(),
            treasury: treasury_account.pubkey(),
            treasury_authority: setup.authority.pubkey(),
            token_program: anchor_spl::token::ID,
        }.to_account_metas(None),
        data: botanika_solana_rewards_distributor::instruction::WithdrawVault { amount: 4000 }.data(),
    };

    setup.context.process_transaction(&[withdraw_ix], &[&setup.authority]).await.unwrap();

    // Verify token vault balance (10000 - 4000 = 6000)
    let vault_account = setup.context.banks_client.get_account(setup.token_vault.pubkey()).await.unwrap().unwrap();
    let vault_data = anchor_spl::token::TokenAccount::try_deserialize(&mut vault_account.data.as_slice()).unwrap();
    assert_eq!(vault_data.amount, 6000);

    // Verify treasury account balance (4000)
    let treasury_account_info = setup.context.banks_client.get_account(treasury_account.pubkey()).await.unwrap().unwrap();
    let treasury_data = anchor_spl::token::TokenAccount::try_deserialize(&mut treasury_account_info.data.as_slice()).unwrap();
    assert_eq!(treasury_data.amount, 4000);

    // 2. Full sweep: Withdraw u64::MAX (should withdraw all remaining 6000 tokens)
    let sweep_ix = Instruction {
        program_id,
        accounts: botanika_solana_rewards_distributor::accounts::WithdrawVault {
            reward_distributor: setup.reward_distributor_pda,
            token_vault: setup.token_vault.pubkey(),
            reward_mint: setup.reward_mint.pubkey(),
            treasury: treasury_account.pubkey(),
            treasury_authority: setup.authority.pubkey(),
            token_program: anchor_spl::token::ID,
        }.to_account_metas(None),
        data: botanika_solana_rewards_distributor::instruction::WithdrawVault { amount: u64::MAX }.data(),
    };

    setup.context.process_transaction(&[sweep_ix], &[&setup.authority]).await.unwrap();

    // Verify token vault balance (0)
    let vault_account = setup.context.banks_client.get_account(setup.token_vault.pubkey()).await.unwrap().unwrap();
    let vault_data = anchor_spl::token::TokenAccount::try_deserialize(&mut vault_account.data.as_slice()).unwrap();
    assert_eq!(vault_data.amount, 0);

    // Verify treasury account balance (10000)
    let treasury_account_info = setup.context.banks_client.get_account(treasury_account.pubkey()).await.unwrap().unwrap();
    let treasury_data = anchor_spl::token::TokenAccount::try_deserialize(&mut treasury_account_info.data.as_slice()).unwrap();
    assert_eq!(treasury_data.amount, 10000);
}

#[tokio::test]
async fn test_withdraw_vault_unauthorized() {
    let mut setup = setup_test().await;
    let program_id = setup.context.program_id;
    let wrong_authority = Keypair::new();

    let treasury_account = Keypair::new();
    setup.context.create_token_account(
        &treasury_account,
        &setup.reward_mint.pubkey(),
        &setup.authority.pubkey(),
    ).await.unwrap();

    let withdraw_ix = Instruction {
        program_id,
        accounts: botanika_solana_rewards_distributor::accounts::WithdrawVault {
            reward_distributor: setup.reward_distributor_pda,
            token_vault: setup.token_vault.pubkey(),
            reward_mint: setup.reward_mint.pubkey(),
            treasury: treasury_account.pubkey(),
            treasury_authority: wrong_authority.pubkey(),
            token_program: anchor_spl::token::ID,
        }.to_account_metas(None),
        data: botanika_solana_rewards_distributor::instruction::WithdrawVault { amount: 1000 }.data(),
    };

    let result = setup.context.process_transaction(&[withdraw_ix], &[&wrong_authority]).await;
    assert!(result.is_err());
}

#[tokio::test]
async fn test_withdraw_vault_insufficient_funds() {
    let mut setup = setup_test().await;
    let program_id = setup.context.program_id;

    let treasury_account = Keypair::new();
    setup.context.create_token_account(
        &treasury_account,
        &setup.reward_mint.pubkey(),
        &setup.authority.pubkey(),
    ).await.unwrap();

    let withdraw_ix = Instruction {
        program_id,
        accounts: botanika_solana_rewards_distributor::accounts::WithdrawVault {
            reward_distributor: setup.reward_distributor_pda,
            token_vault: setup.token_vault.pubkey(),
            reward_mint: setup.reward_mint.pubkey(),
            treasury: treasury_account.pubkey(),
            treasury_authority: setup.authority.pubkey(),
            token_program: anchor_spl::token::ID,
        }.to_account_metas(None),
        data: botanika_solana_rewards_distributor::instruction::WithdrawVault { amount: 10001 }.data(), // 10000 in vault initially
    };

    let result = setup.context.process_transaction(&[withdraw_ix], &[&setup.authority]).await;
    assert!(result.is_err());
}

#[tokio::test]
async fn test_withdraw_vault_invalid_amount() {
    let mut setup = setup_test().await;
    let program_id = setup.context.program_id;

    let treasury_account = Keypair::new();
    setup.context.create_token_account(
        &treasury_account,
        &setup.reward_mint.pubkey(),
        &setup.authority.pubkey(),
    ).await.unwrap();

    let withdraw_ix = Instruction {
        program_id,
        accounts: botanika_solana_rewards_distributor::accounts::WithdrawVault {
            reward_distributor: setup.reward_distributor_pda,
            token_vault: setup.token_vault.pubkey(),
            reward_mint: setup.reward_mint.pubkey(),
            treasury: treasury_account.pubkey(),
            treasury_authority: setup.authority.pubkey(),
            token_program: anchor_spl::token::ID,
        }.to_account_metas(None),
        data: botanika_solana_rewards_distributor::instruction::WithdrawVault { amount: 0 }.data(),
    };

    let result = setup.context.process_transaction(&[withdraw_ix], &[&setup.authority]).await;
    assert!(result.is_err());
}
