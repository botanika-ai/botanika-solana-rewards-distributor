use anchor_lang::InstructionData;
use anchor_lang::ToAccountMetas;
use anchor_lang::prelude::*;
use solana_program_test::*;
use solana_sdk::{
    signature::{Keypair, Signer},
    pubkey::Pubkey,
    system_program,
    instruction::Instruction,
};
use solana_test::state::*;

mod utils;
use utils::*;

#[tokio::test]
async fn test_valid_claim() {
    let mut setup = setup_test().await;
    let program_id = setup.context.program_id;

    let amount = 1000u64;
    let leaf = compute_leaf(setup.miner.pubkey(), amount);
    let root = compute_root(vec![leaf]);

    let update_root_ix = Instruction {
        program_id,
        accounts: solana_test::accounts::UpdateRoot {
            reward_distributor: setup.reward_distributor_pda,
            authority: setup.authority.pubkey(),
        }.to_account_metas(None),
        data: solana_test::instruction::UpdateRoot { new_root: root }.data(),
    };

    setup.context.process_transaction(&[update_root_ix], &[&setup.authority]).await.unwrap();

    let proof = get_proof(vec![leaf], 0);

    let claim_reward_ix = Instruction {
        program_id,
        accounts: solana_test::accounts::ClaimReward {
            reward_distributor: setup.reward_distributor_pda,
            claim_status: setup.claim_status_pda,
            miner_token_account: setup.miner_token_account.pubkey(),
            token_vault: setup.token_vault.pubkey(),
            reward_mint: setup.reward_mint.pubkey(),
            miner: setup.miner.pubkey(),
            token_program: anchor_spl::token::ID,
            system_program: system_program::ID,
        }.to_account_metas(None),
        data: solana_test::instruction::ClaimReward {
            cumulative_amount: amount,
            proof,
        }.data(),
    };

    setup.context.process_transaction(&[claim_reward_ix], &[&setup.miner]).await.unwrap();

    // Verification
    let miner_ata = setup.context.banks_client.get_account(setup.miner_token_account.pubkey()).await.unwrap().unwrap();
    let miner_ata_data = anchor_spl::token::TokenAccount::try_deserialize(&mut miner_ata.data.as_slice()).unwrap();
    assert_eq!(miner_ata_data.amount, 1000);
}

#[tokio::test]
async fn test_invalid_proof() {
    let mut setup = setup_test().await;
    let program_id = setup.context.program_id;

    let amount = 1000u64;
    let leaf = compute_leaf(setup.miner.pubkey(), amount);
    let root = compute_root(vec![leaf]);

    let update_root_ix = Instruction {
        program_id,
        accounts: solana_test::accounts::UpdateRoot {
            reward_distributor: setup.reward_distributor_pda,
            authority: setup.authority.pubkey(),
        }.to_account_metas(None),
        data: solana_test::instruction::UpdateRoot { new_root: root }.data(),
    };

    setup.context.process_transaction(&[update_root_ix], &[&setup.authority]).await.unwrap();

    // Use wrong proof
    let wrong_proof = vec![[0u8; 32]];

    let claim_reward_ix = Instruction {
        program_id,
        accounts: solana_test::accounts::ClaimReward {
            reward_distributor: setup.reward_distributor_pda,
            claim_status: setup.claim_status_pda,
            miner_token_account: setup.miner_token_account.pubkey(),
            token_vault: setup.token_vault.pubkey(),
            reward_mint: setup.reward_mint.pubkey(),
            miner: setup.miner.pubkey(),
            token_program: anchor_spl::token::ID,
            system_program: system_program::ID,
        }.to_account_metas(None),
        data: solana_test::instruction::ClaimReward {
            cumulative_amount: amount,
            proof: wrong_proof,
        }.data(),
    };

    let result = setup.context.process_transaction(&[claim_reward_ix], &[&setup.miner]).await;
    assert!(result.is_err());
}

#[tokio::test]
async fn test_duplicate_claim() {
    let mut setup = setup_test().await;
    let program_id = setup.context.program_id;

    let amount = 1000u64;
    let leaf = compute_leaf(setup.miner.pubkey(), amount);
    let root = compute_root(vec![leaf]);

    let update_root_ix = Instruction {
        program_id,
        accounts: solana_test::accounts::UpdateRoot {
            reward_distributor: setup.reward_distributor_pda,
            authority: setup.authority.pubkey(),
        }.to_account_metas(None),
        data: solana_test::instruction::UpdateRoot { new_root: root }.data(),
    };

    setup.context.process_transaction(&[update_root_ix], &[&setup.authority]).await.unwrap();

    let proof = get_proof(vec![leaf], 0);

    let claim_reward_ix = Instruction {
        program_id,
        accounts: solana_test::accounts::ClaimReward {
            reward_distributor: setup.reward_distributor_pda,
            claim_status: setup.claim_status_pda,
            miner_token_account: setup.miner_token_account.pubkey(),
            token_vault: setup.token_vault.pubkey(),
            reward_mint: setup.reward_mint.pubkey(),
            miner: setup.miner.pubkey(),
            token_program: anchor_spl::token::ID,
            system_program: system_program::ID,
        }.to_account_metas(None),
        data: solana_test::instruction::ClaimReward {
            cumulative_amount: amount,
            proof: proof.clone(),
        }.data(),
    };

    setup.context.process_transaction(&[claim_reward_ix.clone()], &[&setup.miner]).await.unwrap();

    // Second claim should fail
    let result = setup.context.process_transaction(&[claim_reward_ix], &[&setup.miner]).await;
    assert!(result.is_err());
}

#[tokio::test]
async fn test_claim_delta() {
    let mut setup = setup_test().await;
    let program_id = setup.context.program_id;

    // 1. First Claim
    let amount1 = 1000u64;
    let leaf1 = compute_leaf(setup.miner.pubkey(), amount1);
    let root1 = compute_root(vec![leaf1]);

    let update_root_ix1 = Instruction {
        program_id,
        accounts: solana_test::accounts::UpdateRoot {
            reward_distributor: setup.reward_distributor_pda,
            authority: setup.authority.pubkey(),
        }.to_account_metas(None),
        data: solana_test::instruction::UpdateRoot { new_root: root1 }.data(),
    };
    setup.context.process_transaction(&[update_root_ix1], &[&setup.authority]).await.unwrap();

    let proof1 = get_proof(vec![leaf1], 0);
    let claim_ix1 = Instruction {
        program_id,
        accounts: solana_test::accounts::ClaimReward {
            reward_distributor: setup.reward_distributor_pda,
            claim_status: setup.claim_status_pda,
            miner_token_account: setup.miner_token_account.pubkey(),
            token_vault: setup.token_vault.pubkey(),
            reward_mint: setup.reward_mint.pubkey(),
            miner: setup.miner.pubkey(),
            token_program: anchor_spl::token::ID,
            system_program: system_program::ID,
        }.to_account_metas(None),
        data: solana_test::instruction::ClaimReward {
            cumulative_amount: amount1,
            proof: proof1,
        }.data(),
    };
    setup.context.process_transaction(&[claim_ix1], &[&setup.miner]).await.unwrap();

    // 2. Second Claim (Incremental)
    let amount2 = 2500u64; // Incremental = 1500
    let leaf2 = compute_leaf(setup.miner.pubkey(), amount2);
    let root2 = compute_root(vec![leaf2]);

    let update_root_ix2 = Instruction {
        program_id,
        accounts: solana_test::accounts::UpdateRoot {
            reward_distributor: setup.reward_distributor_pda,
            authority: setup.authority.pubkey(),
        }.to_account_metas(None),
        data: solana_test::instruction::UpdateRoot { new_root: root2 }.data(),
    };
    setup.context.process_transaction(&[update_root_ix2], &[&setup.authority]).await.unwrap();

    let proof2 = get_proof(vec![leaf2], 0);
    let claim_ix2 = Instruction {
        program_id,
        accounts: solana_test::accounts::ClaimReward {
            reward_distributor: setup.reward_distributor_pda,
            claim_status: setup.claim_status_pda,
            miner_token_account: setup.miner_token_account.pubkey(),
            token_vault: setup.token_vault.pubkey(),
            reward_mint: setup.reward_mint.pubkey(),
            miner: setup.miner.pubkey(),
            token_program: anchor_spl::token::ID,
            system_program: system_program::ID,
        }.to_account_metas(None),
        data: solana_test::instruction::ClaimReward {
            cumulative_amount: amount2,
            proof: proof2,
        }.data(),
    };
    setup.context.process_transaction(&[claim_ix2], &[&setup.miner]).await.unwrap();

    // Assert total claimed = 2500
    let miner_ata = setup.context.banks_client.get_account(setup.miner_token_account.pubkey()).await.unwrap().unwrap();
    let miner_ata_data = anchor_spl::token::TokenAccount::try_deserialize(&mut miner_ata.data.as_slice()).unwrap();
    assert_eq!(miner_ata_data.amount, 2500);
}

#[tokio::test]
async fn test_wrong_recipient() {
    let mut setup = setup_test().await;
    let program_id = setup.context.program_id;

    let amount = 1000u64;
    let leaf = compute_leaf(setup.miner.pubkey(), amount);
    let root = compute_root(vec![leaf]);

    let update_root_ix = Instruction {
        program_id,
        accounts: solana_test::accounts::UpdateRoot {
            reward_distributor: setup.reward_distributor_pda,
            authority: setup.authority.pubkey(),
        }.to_account_metas(None),
        data: solana_test::instruction::UpdateRoot { new_root: root }.data(),
    };

    setup.context.process_transaction(&[update_root_ix], &[&setup.authority]).await.unwrap();

    let proof = get_proof(vec![leaf], 0);
    let wrong_miner = Keypair::new();

    let claim_reward_ix = Instruction {
        program_id,
        accounts: solana_test::accounts::ClaimReward {
            reward_distributor: setup.reward_distributor_pda,
            claim_status: setup.claim_status_pda,
            miner_token_account: setup.miner_token_account.pubkey(),
            token_vault: setup.token_vault.pubkey(),
            reward_mint: setup.reward_mint.pubkey(),
            miner: wrong_miner.pubkey(),
            token_program: anchor_spl::token::ID,
            system_program: system_program::ID,
        }.to_account_metas(None),
        data: solana_test::instruction::ClaimReward {
            cumulative_amount: amount,
            proof,
        }.data(),
    };

    let result = setup.context.process_transaction(&[claim_reward_ix], &[&wrong_miner]).await;
    assert!(result.is_err());
}

#[tokio::test]
async fn test_wrong_mint_vault() {
    let mut setup = setup_test().await;
    let program_id = setup.context.program_id;
    let wrong_mint = Keypair::new();
    setup.context.create_mint(&wrong_mint, &setup.context.payer.pubkey(), 9).await.unwrap();

    let amount = 1000u64;
    let leaf = compute_leaf(setup.miner.pubkey(), amount);
    let root = compute_root(vec![leaf]);
    let proof = get_proof(vec![leaf], 0);

    // 1. Wrong mint
    let claim_reward_ix = Instruction {
        program_id,
        accounts: solana_test::accounts::ClaimReward {
            reward_distributor: setup.reward_distributor_pda,
            claim_status: setup.claim_status_pda,
            miner_token_account: setup.miner_token_account.pubkey(),
            token_vault: setup.token_vault.pubkey(),
            reward_mint: wrong_mint.pubkey(),
            miner: setup.miner.pubkey(),
            token_program: anchor_spl::token::ID,
            system_program: system_program::ID,
        }.to_account_metas(None),
        data: solana_test::instruction::ClaimReward {
            cumulative_amount: amount,
            proof,
        }.data(),
    };

    let result = setup.context.process_transaction(&[claim_reward_ix], &[&setup.miner]).await;
    assert!(result.is_err());
}
