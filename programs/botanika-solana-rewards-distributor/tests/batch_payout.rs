use anchor_lang::InstructionData;
use anchor_lang::ToAccountMetas;
use anchor_lang::prelude::*;
use solana_program_test::*;
use solana_sdk::{
    instruction::{Instruction, AccountMeta},
    pubkey::Pubkey,
    signature::{Keypair, Signer},
    system_program,
};
use botanika_solana_rewards_distributor::state::*;
use botanika_solana_rewards_distributor::instructions::batch_payout::PayoutItem;

mod utils;
use utils::*;

fn batch_payout_ix(
    setup: &SetupResult,
    batch_id: u64,
    payouts: Vec<PayoutItem>,
) -> Instruction {
    let program_id = setup.context.program_id;
    let mut accounts = botanika_solana_rewards_distributor::accounts::BatchPayout {
        reward_distributor: setup.reward_distributor_pda,
        token_vault: setup.token_vault.pubkey(),
        reward_mint: setup.reward_mint.pubkey(),
        authority: setup.authority.pubkey(),
        token_program: anchor_spl::token::ID,
        system_program: system_program::ID,
    }
    .to_account_metas(None);

    for item in &payouts {
        let claim_status = claim_status_pda(
            &item.node_id_hash,
            &setup.reward_distributor_pda,
            &program_id,
        );
        accounts.push(AccountMeta::new(claim_status, false));
        accounts.push(AccountMeta::new(item.recipient, false));
    }

    Instruction {
        program_id,
        accounts,
        data: botanika_solana_rewards_distributor::instruction::BatchPayout {
            batch_id,
            payouts,
        }
        .data(),
    }
}

#[tokio::test]
async fn test_batch_payout_success() {
    let mut setup = setup_test().await;
    let node_id_hash = hash_node_id(TEST_NODE_ID);
    let amount = 1000u64;

    let recipient = Keypair::new();
    let recipient_token_account = Keypair::new();
    setup
        .context
        .create_token_account(&recipient_token_account, &setup.reward_mint.pubkey(), &recipient.pubkey())
        .await
        .unwrap();

    let payouts = vec![PayoutItem {
        recipient: recipient_token_account.pubkey(),
        node_id_hash,
        amount,
    }];

    let ix = batch_payout_ix(&setup, 1, payouts);
    setup
        .context
        .process_transaction(&[ix], &[&setup.authority])
        .await
        .unwrap();

    // Verify recipient ATA balance
    let recipient_ata = setup
        .context
        .banks_client
        .get_account(recipient_token_account.pubkey())
        .await
        .unwrap()
        .unwrap();
    let recipient_ata_data =
        anchor_spl::token::TokenAccount::try_deserialize(&mut recipient_ata.data.as_slice()).unwrap();
    assert_eq!(recipient_ata_data.amount, 1000);

    // Verify ClaimStatus PDA
    let claim_status_address = claim_status_pda(
        &node_id_hash,
        &setup.reward_distributor_pda,
        &setup.context.program_id,
    );
    let claim_status_account = setup
        .context
        .banks_client
        .get_account(claim_status_address)
        .await
        .unwrap()
        .unwrap();
    let claim_status_data =
        ClaimStatus::try_deserialize(&mut claim_status_account.data.as_slice()).unwrap();
    assert_eq!(claim_status_data.node_id_hash, node_id_hash);
    assert_eq!(claim_status_data.amount_claimed, 1000);

    // Verify RewardDistributor state
    let distributor_account = setup
        .context
        .banks_client
        .get_account(setup.reward_distributor_pda)
        .await
        .unwrap()
        .unwrap();
    let distributor_data =
        RewardDistributor::try_deserialize(&mut distributor_account.data.as_slice()).unwrap();
    assert_eq!(distributor_data.total_distributed, 1000);
}

#[tokio::test]
async fn test_batch_payout_multiple_recipients() {
    let mut setup = setup_test().await;
    
    let node_a = hash_node_id(TEST_NODE_ID);
    let node_b = hash_node_id(TEST_NODE_ID_2);
    let amount_a = 800u64;
    let amount_b = 1200u64;

    let recipient_a = Keypair::new();
    let recipient_token_account_a = Keypair::new();
    setup
        .context
        .create_token_account(&recipient_token_account_a, &setup.reward_mint.pubkey(), &recipient_a.pubkey())
        .await
        .unwrap();

    let recipient_b = Keypair::new();
    let recipient_token_account_b = Keypair::new();
    setup
        .context
        .create_token_account(&recipient_token_account_b, &setup.reward_mint.pubkey(), &recipient_b.pubkey())
        .await
        .unwrap();

    let payouts = vec![
        PayoutItem {
            recipient: recipient_token_account_a.pubkey(),
            node_id_hash: node_a,
            amount: amount_a,
        },
        PayoutItem {
            recipient: recipient_token_account_b.pubkey(),
            node_id_hash: node_b,
            amount: amount_b,
        },
    ];

    let ix = batch_payout_ix(&setup, 1, payouts);
    setup
        .context
        .process_transaction(&[ix], &[&setup.authority])
        .await
        .unwrap();

    // Verify recipient A balance
    let ata_a = setup
        .context
        .banks_client
        .get_account(recipient_token_account_a.pubkey())
        .await
        .unwrap()
        .unwrap();
    let ata_a_data =
        anchor_spl::token::TokenAccount::try_deserialize(&mut ata_a.data.as_slice()).unwrap();
    assert_eq!(ata_a_data.amount, amount_a);

    // Verify recipient B balance
    let ata_b = setup
        .context
        .banks_client
        .get_account(recipient_token_account_b.pubkey())
        .await
        .unwrap()
        .unwrap();
    let ata_b_data =
        anchor_spl::token::TokenAccount::try_deserialize(&mut ata_b.data.as_slice()).unwrap();
    assert_eq!(ata_b_data.amount, amount_b);
}

#[tokio::test]
async fn test_batch_payout_cumulative() {
    let mut setup = setup_test().await;
    let node_id_hash = hash_node_id(TEST_NODE_ID);
    
    let recipient = Keypair::new();
    let recipient_token_account = Keypair::new();
    setup
        .context
        .create_token_account(&recipient_token_account, &setup.reward_mint.pubkey(), &recipient.pubkey())
        .await
        .unwrap();

    // First payout
    let payouts_1 = vec![PayoutItem {
        recipient: recipient_token_account.pubkey(),
        node_id_hash,
        amount: 1000u64,
    }];
    let ix1 = batch_payout_ix(&setup, 1, payouts_1);
    setup
        .context
        .process_transaction(&[ix1], &[&setup.authority])
        .await
        .unwrap();

    // Second payout for same node
    let payouts_2 = vec![PayoutItem {
        recipient: recipient_token_account.pubkey(),
        node_id_hash,
        amount: 1500u64,
    }];
    let ix2 = batch_payout_ix(&setup, 2, payouts_2);
    setup
        .context
        .process_transaction(&[ix2], &[&setup.authority])
        .await
        .unwrap();

    // Verify total balance
    let recipient_ata = setup
        .context
        .banks_client
        .get_account(recipient_token_account.pubkey())
        .await
        .unwrap()
        .unwrap();
    let recipient_ata_data =
        anchor_spl::token::TokenAccount::try_deserialize(&mut recipient_ata.data.as_slice()).unwrap();
    assert_eq!(recipient_ata_data.amount, 2500);

    // Verify ClaimStatus PDA cumulative amount
    let claim_status_address = claim_status_pda(
        &node_id_hash,
        &setup.reward_distributor_pda,
        &setup.context.program_id,
    );
    let claim_status_account = setup
        .context
        .banks_client
        .get_account(claim_status_address)
        .await
        .unwrap()
        .unwrap();
    let claim_status_data =
        ClaimStatus::try_deserialize(&mut claim_status_account.data.as_slice()).unwrap();
    assert_eq!(claim_status_data.amount_claimed, 2500);
}

#[tokio::test]
async fn test_batch_payout_empty_fails() {
    let mut setup = setup_test().await;
    let ix = batch_payout_ix(&setup, 1, vec![]);
    let result = setup
        .context
        .process_transaction(&[ix], &[&setup.authority])
        .await;
    assert!(result.is_err());
}

#[tokio::test]
async fn test_batch_payout_too_large_fails() {
    let mut setup = setup_test().await;
    let mut payouts = vec![];
    for _ in 0..11 {
        payouts.push(PayoutItem {
            recipient: Pubkey::new_unique(),
            node_id_hash: [0u8; 32],
            amount: 100,
        });
    }
    let ix = batch_payout_ix(&setup, 1, payouts);
    let result = setup
        .context
        .process_transaction(&[ix], &[&setup.authority])
        .await;
    assert!(result.is_err());
}

#[tokio::test]
async fn test_batch_payout_unauthorized_fails() {
    let mut setup = setup_test().await;
    let wrong_authority = Keypair::new();
    
    let payouts = vec![PayoutItem {
        recipient: Pubkey::new_unique(),
        node_id_hash: [0u8; 32],
        amount: 100,
    }];
    
    let program_id = setup.context.program_id;
    let mut accounts = botanika_solana_rewards_distributor::accounts::BatchPayout {
        reward_distributor: setup.reward_distributor_pda,
        token_vault: setup.token_vault.pubkey(),
        reward_mint: setup.reward_mint.pubkey(),
        authority: wrong_authority.pubkey(), // non-authority signer
        token_program: anchor_spl::token::ID,
        system_program: system_program::ID,
    }
    .to_account_metas(None);

    let claim_status = claim_status_pda(
        &payouts[0].node_id_hash,
        &setup.reward_distributor_pda,
        &program_id,
    );
    accounts.push(AccountMeta::new(claim_status, false));
    accounts.push(AccountMeta::new(payouts[0].recipient, false));

    let ix = Instruction {
        program_id,
        accounts,
        data: botanika_solana_rewards_distributor::instruction::BatchPayout {
            batch_id: 1,
            payouts,
        }
        .data(),
    };

    let result = setup
        .context
        .process_transaction(&[ix], &[&wrong_authority])
        .await;
    assert!(result.is_err());
}

#[tokio::test]
async fn test_batch_payout_paused_fails() {
    let mut setup = setup_test().await;
    let program_id = setup.context.program_id;

    // Pause distributor
    let pause_ix = Instruction {
        program_id,
        accounts: botanika_solana_rewards_distributor::accounts::Pause {
            reward_distributor: setup.reward_distributor_pda,
            authority: setup.authority.pubkey(),
        }.to_account_metas(None),
        data: botanika_solana_rewards_distributor::instruction::Pause {}.data(),
    };
    setup.context.process_transaction(&[pause_ix], &[&setup.authority]).await.unwrap();

    let payouts = vec![PayoutItem {
        recipient: Pubkey::new_unique(),
        node_id_hash: [0u8; 32],
        amount: 100,
    }];
    let ix = batch_payout_ix(&setup, 1, payouts);
    let result = setup
        .context
        .process_transaction(&[ix], &[&setup.authority])
        .await;
    assert!(result.is_err());
}
