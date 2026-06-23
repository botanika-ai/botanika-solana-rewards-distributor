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
async fn test_initialize() {
    let mut context = TestContext::new().await;
    let program_id = context.program_id;
    let authority = Keypair::new();
    let reward_mint = Keypair::new();
    let token_vault = Keypair::new();

    context.create_mint(&reward_mint, &context.payer.pubkey(), 9).await.unwrap();

    let (reward_distributor_pda, _) = Pubkey::find_program_address(
        &[RewardDistributor::SEED],
        &program_id,
    );

    let initialize_ix = Instruction {
        program_id,
        accounts: solana_test::accounts::Initialize {
            reward_distributor: reward_distributor_pda,
            reward_mint: reward_mint.pubkey(),
            token_vault: token_vault.pubkey(),
            payer: context.payer.pubkey(),
            token_program: anchor_spl::token::ID,
            system_program: system_program::ID,
        }.to_account_metas(None),
        data: solana_test::instruction::Initialize { authority: authority.pubkey() }.data(),
    };

    context.process_transaction(&[initialize_ix], &[&token_vault]).await.unwrap();

    // Verify initial state
    let account = context.banks_client.get_account(reward_distributor_pda).await.unwrap().unwrap();
    let data = RewardDistributor::try_deserialize(&mut account.data.as_slice()).unwrap();
    assert_eq!(data.authority, authority.pubkey());
    assert_eq!(data.reward_mint, reward_mint.pubkey());
    assert_eq!(data.token_vault, token_vault.pubkey());
    assert_eq!(data.current_root, [0u8; 32]);
    assert_eq!(data.epoch_id, 0);
    assert_eq!(data.is_paused, false);
}
