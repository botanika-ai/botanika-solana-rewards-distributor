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
use botanika_solana_rewards_distributor::state::*;

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

    let authorities = all_roles(&authority);
    let finalize_claim_authority = authorities.finalize_claim_authority;

    let initialize_ix = Instruction {
        program_id,
        accounts: botanika_solana_rewards_distributor::accounts::Initialize {
            reward_distributor: reward_distributor_pda,
            reward_mint: reward_mint.pubkey(),
            token_vault: token_vault.pubkey(),
            payer: context.payer.pubkey(),
            token_program: anchor_spl::token::ID,
            system_program: system_program::ID,
        }.to_account_metas(None),
        data: botanika_solana_rewards_distributor::instruction::Initialize { authorities }.data(),
    };

    context.process_transaction(&[initialize_ix], &[&token_vault]).await.unwrap();

    // Verify initial state
    let account = context.banks_client.get_account(reward_distributor_pda).await.unwrap().unwrap();
    let data = RewardDistributor::try_deserialize(&mut account.data.as_slice()).unwrap();
    assert_eq!(data.admin_authority, authority.pubkey());
    assert_eq!(data.root_authority, authority.pubkey());
    assert_eq!(data.payout_authority, authority.pubkey());
    assert_eq!(data.pause_authority, authority.pubkey());
    assert_eq!(data.treasury_authority, authority.pubkey());
    assert_eq!(
        data.finalize_claim_authority, finalize_claim_authority,
        "finalize_claim_authority must actually be persisted on the account, not just validated -- \
         a real bug where this field was validated non-default but never assigned left it stuck at \
         Pubkey::default() regardless of input, bricking finalize_claim for every deployment"
    );
    assert_eq!(data.reward_mint, reward_mint.pubkey());
    assert_eq!(data.token_vault, token_vault.pubkey());
    assert_eq!(data.current_root, [0u8; 32]);
    assert_eq!(data.epoch_id, 0);
    assert_eq!(data.is_paused, false);
    assert_eq!(data.total_claimed, 0);
    assert_eq!(data.total_batch_distributed, 0);
}

/// Regression guard: `finalize_claim_authority` previously defaulted
/// silently to `admin_authority` (a human key) when left as the zero
/// pubkey -- a forgotten config value in a deploy script would leave
/// `finalize_claim`'s only on-chain gate pointed at a human-signable key
/// instead of the Magic Action escrow PDA, with no error to catch the
/// mistake. `initialize_handler` now rejects the zero pubkey outright.
#[tokio::test]
async fn test_initialize_rejects_default_finalize_claim_authority() {
    let mut context = TestContext::new().await;
    let program_id = context.program_id;
    let authority = Keypair::new();
    let reward_mint = Keypair::new();
    let token_vault = Keypair::new();

    context.create_mint(&reward_mint, &context.payer.pubkey(), 9).await.unwrap();

    let (reward_distributor_pda, _) = Pubkey::find_program_address(&[RewardDistributor::SEED], &program_id);

    let mut authorities = all_roles(&authority);
    authorities.finalize_claim_authority = Pubkey::default();

    let initialize_ix = Instruction {
        program_id,
        accounts: botanika_solana_rewards_distributor::accounts::Initialize {
            reward_distributor: reward_distributor_pda,
            reward_mint: reward_mint.pubkey(),
            token_vault: token_vault.pubkey(),
            payer: context.payer.pubkey(),
            token_program: anchor_spl::token::ID,
            system_program: system_program::ID,
        }
        .to_account_metas(None),
        data: botanika_solana_rewards_distributor::instruction::Initialize { authorities }.data(),
    };

    let result = context.process_transaction(&[initialize_ix], &[&token_vault]).await;
    assert!(result.is_err(), "the zero pubkey must be rejected, not silently fall back to admin_authority");
}

/// Regression guard, same gap class as the test above but for the other
/// five roles: previously only `finalize_claim_authority` was validated at
/// init time -- a zero pubkey in any of `admin_authority`/`root_authority`/
/// `payout_authority`/`pause_authority`/`treasury_authority` would
/// permanently brick whatever that role gates, with no recovery path
/// (worst case: a zeroed `admin_authority` makes `set_authority` itself
/// uncallable forever).
#[tokio::test]
async fn test_initialize_rejects_default_admin_authority() {
    let mut context = TestContext::new().await;
    let program_id = context.program_id;
    let authority = Keypair::new();
    let reward_mint = Keypair::new();
    let token_vault = Keypair::new();

    context.create_mint(&reward_mint, &context.payer.pubkey(), 9).await.unwrap();

    let (reward_distributor_pda, _) = Pubkey::find_program_address(&[RewardDistributor::SEED], &program_id);

    let mut authorities = all_roles(&authority);
    authorities.admin_authority = Pubkey::default();

    let initialize_ix = Instruction {
        program_id,
        accounts: botanika_solana_rewards_distributor::accounts::Initialize {
            reward_distributor: reward_distributor_pda,
            reward_mint: reward_mint.pubkey(),
            token_vault: token_vault.pubkey(),
            payer: context.payer.pubkey(),
            token_program: anchor_spl::token::ID,
            system_program: system_program::ID,
        }
        .to_account_metas(None),
        data: botanika_solana_rewards_distributor::instruction::Initialize { authorities }.data(),
    };

    let result = context.process_transaction(&[initialize_ix], &[&token_vault]).await;
    assert!(result.is_err(), "a zero admin_authority must be rejected -- it would brick set_authority itself");
}
