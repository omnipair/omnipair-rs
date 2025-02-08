use anchor_lang::prelude::*;

declare_id!("D9JZvLjZx2zqAKqfLBQB5keRRi8oQYfXPSZwf4rPW1XL");

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
