use anchor_lang::prelude::*;

pub mod error;
pub mod events;
pub mod instructions;
pub mod state;
pub mod utils;

use instructions::*;

declare_id!("82iqRG1Wh4ndMk64DrEgUWCykfwbUdhobFkJ1ppJUnQ4");

#[program]
pub mod botanika_solana_rewards_distributor {
    use super::*;

    pub fn initialize(ctx: Context<Initialize>, authorities: InitializeAuthorities) -> Result<()> {
        initialize_handler(ctx, authorities)
    }

    pub fn update_root(
        ctx: Context<UpdateRoot>,
        new_root: [u8; 32],
        settlement: SettlementInput,
    ) -> Result<()> {
        update_root_handler(ctx, new_root, settlement)
    }

    pub fn batch_payout<'a, 'b, 'c, 'info>(
        ctx: Context<'a, 'b, 'c, 'info, BatchPayout<'info>>,
        batch_id: u64,
        payouts: Vec<PayoutItem>,
    ) -> Result<()> {
        batch_payout_handler(ctx, batch_id, payouts)
    }

    pub fn claim_reward(
        ctx: Context<ClaimReward>,
        node_id_hash: [u8; 32],
        settlement_id: u64,
        cumulative_amount: u64,
        proof: Vec<[u8; 32]>,
    ) -> Result<()> {
        claim_reward_handler(ctx, node_id_hash, settlement_id, cumulative_amount, proof)
    }

    pub fn pause(ctx: Context<Pause>) -> Result<()> {
        pause_handler(ctx)
    }

    pub fn unpause(ctx: Context<Unpause>) -> Result<()> {
        unpause_handler(ctx)
    }

    pub fn set_authority(
        ctx: Context<SetAuthority>,
        role: AuthorityRole,
        new_authority: Pubkey,
    ) -> Result<()> {
        set_authority_handler(ctx, role, new_authority)
    }

    pub fn withdraw_vault(ctx: Context<WithdrawVault>, amount: u64) -> Result<()> {
        withdraw_vault_handler(ctx, amount)
    }

    /// Closes a `RewardSettlementState` no longer needed, reclaiming its
    /// rent -- see `close_settlement.rs`'s doc comment for the real gap
    /// this closes and the retention-window safety reasoning.
    pub fn close_settlement(ctx: Context<CloseSettlement>) -> Result<()> {
        close_settlement_handler(ctx)
    }

    /// Closes a `BatchRecord` no longer needed, reclaiming its rent --
    /// see `close_batch_record.rs`'s doc comment for why this is safe
    /// (unlike `ClaimReceipt`/`ClaimStatus`, deliberately not given a
    /// close instruction).
    pub fn close_batch_record(ctx: Context<CloseBatchRecord>) -> Result<()> {
        close_batch_record_handler(ctx)
    }

    /// Target claim flow (Design Freeze v1 §3.10/§6.4): signer is the
    /// escrow PDA a Magic Action from `botanika-magicblock-contracts`
    /// signs for, not a human keypair. `claim_reward` above is kept
    /// unmodified as the PAD v1.0 §11.3 cold fallback.
    #[allow(clippy::too_many_arguments)]
    pub fn finalize_claim(
        ctx: Context<FinalizeClaim>,
        claim_nonce: u64,
        beneficiary: Pubkey,
        node_id_hash: [u8; 32],
        wallet_binding_id: u64,
        amount: u64,
        reward_state_hash: [u8; 32],
        settlement_id: u64,
    ) -> Result<()> {
        finalize_claim_handler(
            ctx,
            claim_nonce,
            beneficiary,
            node_id_hash,
            wallet_binding_id,
            amount,
            reward_state_hash,
            settlement_id,
        )
    }
}
