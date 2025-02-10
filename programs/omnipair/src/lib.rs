use anchor_lang::prelude::*;

declare_id!("2P89snMvrN1qRTS9sVE2tNee74A8AVSAgSAwog1Hv7aZ");

#[program]
pub mod omnipair {
    use super::*;

    pub fn initialize(ctx: Context<Initialize>) -> Result<()> {
        msg!("Greetings from: {:?}", ctx.program_id);
        Ok(())
    }
}

#[derive(Accounts)]
pub struct Initialize {}
