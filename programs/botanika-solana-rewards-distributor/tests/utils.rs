use anchor_lang::prelude::*;
use anchor_lang::{InstructionData, ToAccountMetas};
use anchor_lang::solana_program::keccak;
use anchor_spl::token_interface;
use solana_program_test::*;
use solana_sdk::{
    signature::{Keypair, Signer},
    transaction::Transaction,
    instruction::Instruction,
    system_program,
};
use botanika_solana_rewards_distributor::instructions::*;
use botanika_solana_rewards_distributor::state::*;

fn entry_wrapper(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    instruction_data: &[u8],
) -> std::result::Result<(), solana_sdk::program_error::ProgramError> {
    let coerced_accounts = unsafe {
        std::mem::transmute::<&[AccountInfo], &[AccountInfo]>(accounts)
    };
    botanika_solana_rewards_distributor::entry(program_id, coerced_accounts, instruction_data)
        .map_err(|e| e.into())
}

pub struct TestContext {
    pub banks_client: BanksClient,
    pub payer: Keypair,
    pub recent_blockhash: solana_sdk::hash::Hash,
    pub program_id: Pubkey,
}

impl TestContext {
    pub async fn new() -> Self {
        let program_id = botanika_solana_rewards_distributor::ID;
        let mut program_test = ProgramTest::new(
            "botanika_solana_rewards_distributor",
            program_id,
            processor!(entry_wrapper),
        );


        let (banks_client, payer, recent_blockhash) = program_test.start().await;

        Self {
            banks_client,
            payer,
            recent_blockhash,
            program_id,
        }
    }

    pub async fn get_latest_blockhash(&mut self) -> solana_sdk::hash::Hash {
        self.banks_client
            .get_latest_blockhash()
            .await
            .unwrap()
    }

    pub async fn process_transaction(&mut self, ixs: &[Instruction], signers: &[&Keypair]) -> std::result::Result<(), BanksClientError> {
        let blockhash = self.get_latest_blockhash().await;
        let mut all_signers = vec![&self.payer];
        for s in signers {
            all_signers.push(s);
        }

        let tx = Transaction::new_signed_with_payer(
            ixs,
            Some(&self.payer.pubkey()),
            &all_signers,
            blockhash,
        );

        self.banks_client.process_transaction(tx).await
    }

    pub async fn create_mint(&mut self, mint: &Keypair, authority: &Pubkey, decimals: u8) -> std::result::Result<(), BanksClientError> {
        let rent = self.banks_client.get_rent().await.unwrap();
        let lamports = rent.minimum_balance(anchor_spl::token::Mint::LEN);

        let ixs = [
            solana_sdk::system_instruction::create_account(
                &self.payer.pubkey(),
                &mint.pubkey(),
                lamports,
                anchor_spl::token::Mint::LEN as u64,
                &anchor_spl::token::ID,
            ),
            anchor_spl::token::spl_token::instruction::initialize_mint(
                &anchor_spl::token::ID,
                &mint.pubkey(),
                authority,
                None,
                decimals,
            ).unwrap(),
        ];

        self.process_transaction(&ixs, &[mint]).await
    }

    pub async fn create_token_account(&mut self, account: &Keypair, mint: &Pubkey, owner: &Pubkey) -> std::result::Result<(), BanksClientError> {
        let rent = self.banks_client.get_rent().await.unwrap();
        let lamports = rent.minimum_balance(anchor_spl::token::TokenAccount::LEN);

        let ixs = [
            solana_sdk::system_instruction::create_account(
                &self.payer.pubkey(),
                &account.pubkey(),
                lamports,
                anchor_spl::token::TokenAccount::LEN as u64,
                &anchor_spl::token::ID,
            ),
            anchor_spl::token::spl_token::instruction::initialize_account(
                &anchor_spl::token::ID,
                &account.pubkey(),
                mint,
                owner,
            ).unwrap(),
        ];

        self.process_transaction(&ixs, &[account]).await
    }

    pub async fn mint_to(&mut self, mint: &Pubkey, to: &Pubkey, authority: &Keypair, amount: u64) -> std::result::Result<(), BanksClientError> {
        let ix = anchor_spl::token::spl_token::instruction::mint_to(
            &anchor_spl::token::ID,
            mint,
            to,
            &authority.pubkey(),
            &[],
            amount,
        ).unwrap();

        self.process_transaction(&[ix], &[authority]).await
    }
}

pub struct SetupResult {
    pub context: TestContext,
    pub authority: Keypair,
    pub reward_mint: Keypair,
    pub token_vault: Keypair,
    pub miner: Keypair,
    pub miner_token_account: Keypair,
    pub reward_distributor_pda: Pubkey,
}

pub const TEST_NODE_ID: &str = "node-test-001";
pub const TEST_NODE_ID_2: &str = "node-test-002";

pub async fn setup_test() -> SetupResult {
    let mut context = TestContext::new().await;
    let program_id = context.program_id;

    let authority = Keypair::new();
    let reward_mint = Keypair::new();
    let token_vault = Keypair::new();
    let miner = Keypair::new();
    let miner_token_account = Keypair::new();

    context.create_mint(&reward_mint, &context.payer.pubkey(), 9).await.unwrap();

    let transfer_ix = solana_sdk::system_instruction::transfer(
        &context.payer.pubkey(),
        &miner.pubkey(),
        10_000_000,
    );
    context.process_transaction(&[transfer_ix], &[]).await.unwrap();

    let (reward_distributor_pda, _) = Pubkey::find_program_address(
        &[RewardDistributor::SEED],
        &program_id,
    );

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
        data: botanika_solana_rewards_distributor::instruction::Initialize { authority: authority.pubkey() }.data(),
    };

    context.process_transaction(&[initialize_ix], &[&token_vault]).await.unwrap();

    let payer = Keypair::from_bytes(&context.payer.to_bytes()).unwrap();
    context.mint_to(&reward_mint.pubkey(), &token_vault.pubkey(), &payer, 10000).await.unwrap();
    context.create_token_account(&miner_token_account, &reward_mint.pubkey(), &miner.pubkey()).await.unwrap();

    SetupResult {
        context,
        authority,
        reward_mint,
        token_vault,
        miner,
        miner_token_account,
        reward_distributor_pda,
    }
}

pub fn hash_node_id(node_id: &str) -> [u8; 32] {
    keccak::hash(node_id.as_bytes()).0
}

pub fn claim_status_pda(
    node_id_hash: &[u8; 32],
    reward_distributor: &Pubkey,
    program_id: &Pubkey,
) -> Pubkey {
    Pubkey::find_program_address(
        &[
            ClaimStatus::SEED,
            node_id_hash,
            reward_distributor.as_ref(),
        ],
        program_id,
    )
    .0
}

pub fn compute_leaf(miner: Pubkey, node_id_hash: [u8; 32], amount: u64) -> [u8; 32] {
    keccak::hashv(&[
        miner.as_ref(),
        &node_id_hash,
        &amount.to_le_bytes(),
    ])
    .0
}

pub fn compute_root(leaves: Vec<[u8; 32]>) -> [u8; 32] {
    if leaves.is_empty() {
        return [0u8; 32];
    }
    let mut current_level = leaves;
    while current_level.len() > 1 {
        let mut next_level = Vec::new();
        for i in (0..current_level.len()).step_by(2) {
            if i + 1 < current_level.len() {
                let mut combined = [0u8; 64];
                let (left, right) = if current_level[i] < current_level[i + 1] {
                     (current_level[i], current_level[i+1])
                } else {
                     (current_level[i+1], current_level[i])
                };
                combined[..32].copy_from_slice(&left);
                combined[32..].copy_from_slice(&right);
                next_level.push(keccak::hash(&combined).0);
            } else {
                next_level.push(current_level[i]);
            }
        }
        current_level = next_level;
    }
    current_level[0]
}

pub fn get_proof(leaves: Vec<[u8; 32]>, index: usize) -> Vec<[u8; 32]> {
    let mut proof = Vec::new();
    let mut current_level = leaves;
    let mut current_index = index;
    
    while current_level.len() > 1 {
        let sibling_index = if current_index % 2 == 0 {
            current_index + 1
        } else {
            current_index - 1
        };

        if sibling_index < current_level.len() {
            proof.push(current_level[sibling_index]);
        }

        let mut next_level = Vec::new();
        for i in (0..current_level.len()).step_by(2) {
            if i + 1 < current_level.len() {
                let mut combined = [0u8; 64];
                let (left, right) = if current_level[i] < current_level[i + 1] {
                     (current_level[i], current_level[i+1])
                } else {
                     (current_level[i+1], current_level[i])
                };
                combined[..32].copy_from_slice(&left);
                combined[32..].copy_from_slice(&right);
                next_level.push(keccak::hash(&combined).0);
            } else {
                next_level.push(current_level[i]);
            }
        }
        current_level = next_level;
        current_index /= 2;
    }
    proof
}
