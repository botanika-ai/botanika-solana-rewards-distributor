use anchor_lang::InstructionData;
use anchor_lang::ToAccountMetas;
use anchor_lang::prelude::*;
use solana_program_test::*;
use solana_sdk::{
    signature::{Signer},
    instruction::Instruction,
};
use botanika_solana_rewards_distributor::state::*;

mod utils;
use utils::*;

#[tokio::test]
async fn test_update_root() {
    let mut setup = setup_test().await;
    let program_id = setup.context.program_id;

    let new_root = [1u8; 32];
    let update_root_ix = Instruction {
        program_id,
        accounts: botanika_solana_rewards_distributor::accounts::UpdateRoot {
            reward_distributor: setup.reward_distributor_pda,
            authority: setup.authority.pubkey(),
        }.to_account_metas(None),
        data: botanika_solana_rewards_distributor::instruction::UpdateRoot { new_root }.data(),
    };

    setup.context.process_transaction(&[update_root_ix], &[&setup.authority]).await.unwrap();

    // Verify state
    let account = setup.context.banks_client.get_account(setup.reward_distributor_pda).await.unwrap().unwrap();
    let data = RewardDistributor::try_deserialize(&mut account.data.as_slice()).unwrap();
    assert_eq!(data.current_root, new_root);
    assert_eq!(data.epoch_id, 1);
}
