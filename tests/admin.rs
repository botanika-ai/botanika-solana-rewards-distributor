use anchor_lang::InstructionData;
use anchor_lang::ToAccountMetas;
use anchor_lang::prelude::*;
use solana_program_test::*;
use solana_sdk::{
    signature::{Keypair, Signer},
    system_program,
    instruction::Instruction,
};
use solana_test::state::*;

mod utils;
use utils::*;

#[tokio::test]
async fn test_set_authority() {
    let mut setup = setup_test().await;
    let program_id = setup.context.program_id;
    let new_authority = Keypair::new();

    let set_authority_ix = Instruction {
        program_id,
        accounts: solana_test::accounts::SetAuthority {
            reward_distributor: setup.reward_distributor_pda,
            authority: setup.authority.pubkey(),
        }.to_account_metas(None),
        data: solana_test::instruction::SetAuthority { new_authority: new_authority.pubkey() }.data(),
    };

    setup.context.process_transaction(&[set_authority_ix], &[&setup.authority]).await.unwrap();

    // Verify state
    let account = setup.context.banks_client.get_account(setup.reward_distributor_pda).await.unwrap().unwrap();
    let data = RewardDistributor::try_deserialize(&mut account.data.as_slice()).unwrap();
    assert_eq!(data.authority, new_authority.pubkey());
}

#[tokio::test]
async fn test_pause_unpause() {
    let mut setup = setup_test().await;
    let program_id = setup.context.program_id;

    // 1. Pause
    let pause_ix = Instruction {
        program_id,
        accounts: solana_test::accounts::Pause {
            reward_distributor: setup.reward_distributor_pda,
            authority: setup.authority.pubkey(),
        }.to_account_metas(None),
        data: solana_test::instruction::Pause {}.data(),
    };
    setup.context.process_transaction(&[pause_ix], &[&setup.authority]).await.unwrap();

    let account = setup.context.banks_client.get_account(setup.reward_distributor_pda).await.unwrap().unwrap();
    let data = RewardDistributor::try_deserialize(&mut account.data.as_slice()).unwrap();
    assert_eq!(data.is_paused, true);

    // 2. Unpause
    let unpause_ix = Instruction {
        program_id,
        accounts: solana_test::accounts::Unpause {
            reward_distributor: setup.reward_distributor_pda,
            authority: setup.authority.pubkey(),
        }.to_account_metas(None),
        data: solana_test::instruction::Unpause {}.data(),
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

    // 1. Try update_root with wrong authority
    let update_root_ix = Instruction {
        program_id,
        accounts: solana_test::accounts::UpdateRoot {
            reward_distributor: setup.reward_distributor_pda,
            authority: wrong_authority.pubkey(),
        }.to_account_metas(None),
        data: solana_test::instruction::UpdateRoot { new_root: [1u8; 32] }.data(),
    };
    let result = setup.context.process_transaction(&[update_root_ix], &[&wrong_authority]).await;
    assert!(result.is_err());
}
