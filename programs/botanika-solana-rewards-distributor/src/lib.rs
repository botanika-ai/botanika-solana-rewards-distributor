use anchor_lang::prelude::*;

declare_id!("EgXM56PxY7JNSDwwBu2S4LXQV8JMJaJcQq2KTJzg5RQg");

#[program]
pub mod botanika_solana_rewards_distributor {
    use super::*;

    pub fn initialize(ctx: Context<Initialize>) -> Result<()> {
        msg!("Greetings from: {:?}", ctx.program_id);
        Ok(())
    }
}

#[derive(Accounts)]
pub struct Initialize {}
