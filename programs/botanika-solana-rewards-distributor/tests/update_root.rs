use anchor_lang::prelude::*;
use botanika_solana_rewards_distributor::state::*;

mod utils;
use utils::*;

#[tokio::test]
async fn test_update_root() {
    let mut setup = setup_test().await;

    let new_root = [1u8; 32];
    let settlement = dummy_settlement(1, 0, 0);
    let ix = update_root_ix(&setup, new_root, 1, settlement);

    setup.context.process_transaction(&[ix], &[&setup.authority]).await.unwrap();

    // Verify RewardDistributor state
    let account = setup.context.banks_client.get_account(setup.reward_distributor_pda).await.unwrap().unwrap();
    let data = RewardDistributor::try_deserialize(&mut account.data.as_slice()).unwrap();
    assert_eq!(data.current_root, new_root);
    assert_eq!(data.epoch_id, 1);

    // Verify RewardSettlementState was created and linked (P0-RWD-03)
    let settlement_address = settlement_pda(1, &setup.context.program_id);
    let settlement_account = setup.context.banks_client.get_account(settlement_address).await.unwrap().unwrap();
    let settlement_data = RewardSettlementState::try_deserialize(&mut settlement_account.data.as_slice()).unwrap();
    assert_eq!(settlement_data.settlement_id, 1);
    assert_eq!(settlement_data.reward_delta_root, new_root);
}

#[tokio::test]
async fn test_update_root_invalid_settlement_range_fails() {
    let mut setup = setup_test().await;

    let mut settlement = dummy_settlement(1, 0, 0);
    settlement.epoch_from = 5;
    settlement.epoch_to = 1;
    let ix = update_root_ix(&setup, [1u8; 32], 1, settlement);

    let result = setup.context.process_transaction(&[ix], &[&setup.authority]).await;
    assert!(result.is_err());
}
