use anchor_lang::InstructionData;
use anchor_lang::ToAccountMetas;
use anchor_lang::prelude::*;
use solana_program_test::*;
use solana_sdk::{
    instruction::Instruction,
    pubkey::Pubkey,
    signature::{Keypair, Signer},
    system_program,
};

mod utils;
use utils::*;

fn claim_reward_ix(
    setup: &SetupResult,
    node_id_hash: [u8; 32],
    cumulative_amount: u64,
    proof: Vec<[u8; 32]>,
    miner_token_account: Pubkey,
    miner: Pubkey,
) -> Instruction {
    let claim_status = claim_status_pda(
        &node_id_hash,
        &setup.reward_distributor_pda,
        &setup.context.program_id,
    );

    Instruction {
        program_id: setup.context.program_id,
        accounts: botanika_solana_rewards_distributor::accounts::ClaimReward {
            reward_distributor: setup.reward_distributor_pda,
            claim_status,
            miner_token_account,
            token_vault: setup.token_vault.pubkey(),
            reward_mint: setup.reward_mint.pubkey(),
            miner,
            token_program: anchor_spl::token::ID,
            system_program: system_program::ID,
        }
        .to_account_metas(None),
        data: botanika_solana_rewards_distributor::instruction::ClaimReward {
            node_id_hash,
            cumulative_amount,
            proof,
        }
        .data(),
    }
}

#[tokio::test]
async fn test_valid_claim() {
    let mut setup = setup_test().await;
    let node_id_hash = hash_node_id(TEST_NODE_ID);
    let amount = 1000u64;
    let leaf = compute_leaf(&setup, 1, setup.miner.pubkey(), node_id_hash, amount);
    let root = compute_root(vec![leaf]);

    setup
        .context
        .process_transaction(&[update_root_ix(&setup, root, 1, dummy_settlement(1, 1, amount))], &[&setup.authority])
        .await
        .unwrap();

    let proof = get_proof(vec![leaf], 0);
    let claim_ix = claim_reward_ix(
        &setup,
        node_id_hash,
        amount,
        proof,
        setup.miner_token_account.pubkey(),
        setup.miner.pubkey(),
    );

    setup
        .context
        .process_transaction(&[claim_ix], &[&setup.miner])
        .await
        .unwrap();

    let miner_ata = setup
        .context
        .banks_client
        .get_account(setup.miner_token_account.pubkey())
        .await
        .unwrap()
        .unwrap();
    let miner_ata_data =
        anchor_spl::token::TokenAccount::try_deserialize(&mut miner_ata.data.as_slice()).unwrap();
    assert_eq!(miner_ata_data.amount, 1000);
}

#[tokio::test]
async fn test_invalid_proof() {
    let mut setup = setup_test().await;
    let node_id_hash = hash_node_id(TEST_NODE_ID);
    let amount = 1000u64;
    let leaf = compute_leaf(&setup, 1, setup.miner.pubkey(), node_id_hash, amount);
    let root = compute_root(vec![leaf]);

    setup
        .context
        .process_transaction(&[update_root_ix(&setup, root, 1, dummy_settlement(1, 1, amount))], &[&setup.authority])
        .await
        .unwrap();

    let claim_ix = claim_reward_ix(
        &setup,
        node_id_hash,
        amount,
        vec![[0u8; 32]],
        setup.miner_token_account.pubkey(),
        setup.miner.pubkey(),
    );

    let result = setup
        .context
        .process_transaction(&[claim_ix], &[&setup.miner])
        .await;
    assert!(result.is_err());
}

#[tokio::test]
async fn test_duplicate_claim() {
    let mut setup = setup_test().await;
    let node_id_hash = hash_node_id(TEST_NODE_ID);
    let amount = 1000u64;
    let leaf = compute_leaf(&setup, 1, setup.miner.pubkey(), node_id_hash, amount);
    let root = compute_root(vec![leaf]);

    setup
        .context
        .process_transaction(&[update_root_ix(&setup, root, 1, dummy_settlement(1, 1, amount))], &[&setup.authority])
        .await
        .unwrap();

    let proof = get_proof(vec![leaf], 0);
    let claim_ix = claim_reward_ix(
        &setup,
        node_id_hash,
        amount,
        proof,
        setup.miner_token_account.pubkey(),
        setup.miner.pubkey(),
    );

    setup
        .context
        .process_transaction(&[claim_ix.clone()], &[&setup.miner])
        .await
        .unwrap();

    let dummy_ix = solana_sdk::system_instruction::transfer(
        &setup.context.payer.pubkey(),
        &Pubkey::new_unique(),
        0,
    );
    let result = setup
        .context
        .process_transaction(&[claim_ix, dummy_ix], &[&setup.miner])
        .await;
    assert!(result.is_err());
}

#[tokio::test]
async fn test_claim_delta() {
    let mut setup = setup_test().await;
    let node_id_hash = hash_node_id(TEST_NODE_ID);

    let amount1 = 1000u64;
    let leaf1 = compute_leaf(&setup, 1, setup.miner.pubkey(), node_id_hash, amount1);
    let root1 = compute_root(vec![leaf1]);

    setup
        .context
        .process_transaction(&[update_root_ix(&setup, root1, 1, dummy_settlement(1, 1, amount1))], &[&setup.authority])
        .await
        .unwrap();

    let proof1 = get_proof(vec![leaf1], 0);
    let claim_ix1 = claim_reward_ix(
        &setup,
        node_id_hash,
        amount1,
        proof1,
        setup.miner_token_account.pubkey(),
        setup.miner.pubkey(),
    );
    setup
        .context
        .process_transaction(&[claim_ix1], &[&setup.miner])
        .await
        .unwrap();

    let amount2 = 2500u64;
    let leaf2 = compute_leaf(&setup, 2, setup.miner.pubkey(), node_id_hash, amount2);
    let root2 = compute_root(vec![leaf2]);

    setup
        .context
        .process_transaction(&[update_root_ix(&setup, root2, 2, dummy_settlement(2, 1, amount2))], &[&setup.authority])
        .await
        .unwrap();

    let proof2 = get_proof(vec![leaf2], 0);
    let claim_ix2 = claim_reward_ix(
        &setup,
        node_id_hash,
        amount2,
        proof2,
        setup.miner_token_account.pubkey(),
        setup.miner.pubkey(),
    );
    setup
        .context
        .process_transaction(&[claim_ix2], &[&setup.miner])
        .await
        .unwrap();

    let miner_ata = setup
        .context
        .banks_client
        .get_account(setup.miner_token_account.pubkey())
        .await
        .unwrap()
        .unwrap();
    let miner_ata_data =
        anchor_spl::token::TokenAccount::try_deserialize(&mut miner_ata.data.as_slice()).unwrap();
    assert_eq!(miner_ata_data.amount, 2500);
}

#[tokio::test]
async fn test_claim_two_nodes_same_miner() {
    let mut setup = setup_test().await;
    let node_a = hash_node_id(TEST_NODE_ID);
    let node_b = hash_node_id(TEST_NODE_ID_2);

    let amount_a = 800u64;
    let amount_b = 1200u64;
    let leaf_a = compute_leaf(&setup, 1, setup.miner.pubkey(), node_a, amount_a);
    let leaf_b = compute_leaf(&setup, 1, setup.miner.pubkey(), node_b, amount_b);
    let mut leaves = vec![leaf_a, leaf_b];
    leaves.sort();
    let root = compute_root(leaves.clone());

    setup
        .context
        .process_transaction(
            &[update_root_ix(&setup, root, 1, dummy_settlement(1, 2, amount_a + amount_b))],
            &[&setup.authority],
        )
        .await
        .unwrap();

    let index_a = leaves.iter().position(|l| *l == leaf_a).unwrap();
    let index_b = leaves.iter().position(|l| *l == leaf_b).unwrap();

    let claim_a = claim_reward_ix(
        &setup,
        node_a,
        amount_a,
        get_proof(leaves.clone(), index_a),
        setup.miner_token_account.pubkey(),
        setup.miner.pubkey(),
    );
    let claim_b = claim_reward_ix(
        &setup,
        node_b,
        amount_b,
        get_proof(leaves, index_b),
        setup.miner_token_account.pubkey(),
        setup.miner.pubkey(),
    );

    setup
        .context
        .process_transaction(&[claim_a], &[&setup.miner])
        .await
        .unwrap();
    setup
        .context
        .process_transaction(&[claim_b], &[&setup.miner])
        .await
        .unwrap();

    let miner_ata = setup
        .context
        .banks_client
        .get_account(setup.miner_token_account.pubkey())
        .await
        .unwrap()
        .unwrap();
    let miner_ata_data =
        anchor_spl::token::TokenAccount::try_deserialize(&mut miner_ata.data.as_slice()).unwrap();
    assert_eq!(miner_ata_data.amount, amount_a + amount_b);
}

#[tokio::test]
async fn test_wrong_node_id_hash() {
    let mut setup = setup_test().await;
    let node_id_hash = hash_node_id(TEST_NODE_ID);
    let wrong_node_id_hash = hash_node_id("wrong-node");
    let amount = 1000u64;
    let leaf = compute_leaf(&setup, 1, setup.miner.pubkey(), node_id_hash, amount);
    let root = compute_root(vec![leaf]);

    setup
        .context
        .process_transaction(&[update_root_ix(&setup, root, 1, dummy_settlement(1, 1, amount))], &[&setup.authority])
        .await
        .unwrap();

    let claim_ix = claim_reward_ix(
        &setup,
        wrong_node_id_hash,
        amount,
        get_proof(vec![leaf], 0),
        setup.miner_token_account.pubkey(),
        setup.miner.pubkey(),
    );

    let result = setup
        .context
        .process_transaction(&[claim_ix], &[&setup.miner])
        .await;
    assert!(result.is_err());
}

#[tokio::test]
async fn test_wrong_recipient() {
    let mut setup = setup_test().await;
    let node_id_hash = hash_node_id(TEST_NODE_ID);
    let amount = 1000u64;
    let leaf = compute_leaf(&setup, 1, setup.miner.pubkey(), node_id_hash, amount);
    let root = compute_root(vec![leaf]);

    setup
        .context
        .process_transaction(&[update_root_ix(&setup, root, 1, dummy_settlement(1, 1, amount))], &[&setup.authority])
        .await
        .unwrap();

    let proof = get_proof(vec![leaf], 0);
    let wrong_miner = Keypair::new();

    let claim_ix = claim_reward_ix(
        &setup,
        node_id_hash,
        amount,
        proof,
        setup.miner_token_account.pubkey(),
        wrong_miner.pubkey(),
    );

    let result = setup
        .context
        .process_transaction(&[claim_ix], &[&wrong_miner])
        .await;
    assert!(result.is_err());
}

#[tokio::test]
async fn test_wrong_mint_vault() {
    let mut setup = setup_test().await;
    let wrong_mint = Keypair::new();
    setup
        .context
        .create_mint(&wrong_mint, &setup.context.payer.pubkey(), 9)
        .await
        .unwrap();

    let node_id_hash = hash_node_id(TEST_NODE_ID);
    let amount = 1000u64;
    let leaf = compute_leaf(&setup, 0, setup.miner.pubkey(), node_id_hash, amount);
    let proof = get_proof(vec![leaf], 0);

    let claim_status = claim_status_pda(
        &node_id_hash,
        &setup.reward_distributor_pda,
        &setup.context.program_id,
    );

    let claim_ix = Instruction {
        program_id: setup.context.program_id,
        accounts: botanika_solana_rewards_distributor::accounts::ClaimReward {
            reward_distributor: setup.reward_distributor_pda,
            claim_status,
            miner_token_account: setup.miner_token_account.pubkey(),
            token_vault: setup.token_vault.pubkey(),
            reward_mint: wrong_mint.pubkey(),
            miner: setup.miner.pubkey(),
            token_program: anchor_spl::token::ID,
            system_program: system_program::ID,
        }
        .to_account_metas(None),
        data: botanika_solana_rewards_distributor::instruction::ClaimReward {
            node_id_hash,
            cumulative_amount: amount,
            proof,
        }
        .data(),
    };

    let result = setup
        .context
        .process_transaction(&[claim_ix], &[&setup.miner])
        .await;
    assert!(result.is_err());
}

#[tokio::test]
async fn test_invalid_recipient_owner() {
    let mut setup = setup_test().await;
    let node_id_hash = hash_node_id(TEST_NODE_ID);
    let amount = 1000u64;
    let leaf = compute_leaf(&setup, 1, setup.miner.pubkey(), node_id_hash, amount);
    let root = compute_root(vec![leaf]);

    setup
        .context
        .process_transaction(&[update_root_ix(&setup, root, 1, dummy_settlement(1, 1, amount))], &[&setup.authority])
        .await
        .unwrap();

    let proof = get_proof(vec![leaf], 0);
    let random_owner = Keypair::new();
    let wrong_token_account = Keypair::new();
    setup
        .context
        .create_token_account(
            &wrong_token_account,
            &setup.reward_mint.pubkey(),
            &random_owner.pubkey(),
        )
        .await
        .unwrap();

    let claim_ix = claim_reward_ix(
        &setup,
        node_id_hash,
        amount,
        proof,
        wrong_token_account.pubkey(),
        setup.miner.pubkey(),
    );

    let result = setup
        .context
        .process_transaction(&[claim_ix], &[&setup.miner])
        .await;
    assert!(result.is_err());
}

#[tokio::test]
async fn test_wallet_change_claims_delta_only() {
    let mut setup = setup_test().await;
    let node_id_hash = hash_node_id(TEST_NODE_ID);

    let first_miner = setup.miner.insecure_clone();
    let second_miner = Keypair::new();
    let second_miner_token_account = Keypair::new();

    let transfer_ix = solana_sdk::system_instruction::transfer(
        &setup.context.payer.pubkey(),
        &second_miner.pubkey(),
        10_000_000,
    );
    setup
        .context
        .process_transaction(&[transfer_ix], &[])
        .await
        .unwrap();
    setup
        .context
        .create_token_account(
            &second_miner_token_account,
            &setup.reward_mint.pubkey(),
            &second_miner.pubkey(),
        )
        .await
        .unwrap();

    let amount1 = 100u64;
    let leaf1 = compute_leaf(&setup, 1, first_miner.pubkey(), node_id_hash, amount1);
    let root1 = compute_root(vec![leaf1]);

    setup
        .context
        .process_transaction(&[update_root_ix(&setup, root1, 1, dummy_settlement(1, 1, amount1))], &[&setup.authority])
        .await
        .unwrap();

    let claim_ix1 = claim_reward_ix(
        &setup,
        node_id_hash,
        amount1,
        get_proof(vec![leaf1], 0),
        setup.miner_token_account.pubkey(),
        first_miner.pubkey(),
    );
    setup
        .context
        .process_transaction(&[claim_ix1], &[&first_miner])
        .await
        .unwrap();

    let amount2 = 110u64;
    let leaf2 = compute_leaf(&setup, 2, second_miner.pubkey(), node_id_hash, amount2);
    let root2 = compute_root(vec![leaf2]);

    setup
        .context
        .process_transaction(&[update_root_ix(&setup, root2, 2, dummy_settlement(2, 1, amount2))], &[&setup.authority])
        .await
        .unwrap();

    let claim_ix2 = claim_reward_ix(
        &setup,
        node_id_hash,
        amount2,
        get_proof(vec![leaf2], 0),
        second_miner_token_account.pubkey(),
        second_miner.pubkey(),
    );
    setup
        .context
        .process_transaction(&[claim_ix2], &[&second_miner])
        .await
        .unwrap();

    let first_ata = setup
        .context
        .banks_client
        .get_account(setup.miner_token_account.pubkey())
        .await
        .unwrap()
        .unwrap();
    let first_ata_data =
        anchor_spl::token::TokenAccount::try_deserialize(&mut first_ata.data.as_slice()).unwrap();
    assert_eq!(first_ata_data.amount, 100);

    let second_ata = setup
        .context
        .banks_client
        .get_account(second_miner_token_account.pubkey())
        .await
        .unwrap()
        .unwrap();
    let second_ata_data =
        anchor_spl::token::TokenAccount::try_deserialize(&mut second_ata.data.as_slice()).unwrap();
    assert_eq!(second_ata_data.amount, 10);

    let old_wallet_claim = claim_reward_ix(
        &setup,
        node_id_hash,
        amount2,
        get_proof(vec![leaf2], 0),
        setup.miner_token_account.pubkey(),
        first_miner.pubkey(),
    );
    let old_wallet_result = setup
        .context
        .process_transaction(&[old_wallet_claim], &[&first_miner])
        .await;
    assert!(old_wallet_result.is_err());
}
