use anchor_lang::prelude::*;
use crate::state::futarchy_authority::FutarchyAuthority;
use crate::constants::FUTARCHY_AUTHORITY_SEED_PREFIX;
use crate::errors::ErrorCode;

#[derive(AnchorSerialize, AnchorDeserialize, Clone)]
pub struct UpdateRevenueRecipientsArgs {
    pub futarchy_treasury: Option<Pubkey>,
    pub buybacks_vault: Option<Pubkey>,
    pub team_treasury: Option<Pubkey>,
}

#[derive(Accounts)]
pub struct UpdateRevenueRecipients<'info> {
    #[account(
        mut,
        address = futarchy_authority.authority @ ErrorCode::InvalidFutarchyAuthority
    )]
    pub authority_signer: Signer<'info>,

    #[account(
        mut,
        seeds = [FUTARCHY_AUTHORITY_SEED_PREFIX],
        bump = futarchy_authority.bump
    )]
    pub futarchy_authority: Account<'info, FutarchyAuthority>,

    pub system_program: Program<'info, System>,
}

impl<'info> UpdateRevenueRecipients<'info> {
    pub fn handle_update(ctx: Context<Self>, args: UpdateRevenueRecipientsArgs) -> Result<()> {
        let futarchy_authority = &mut ctx.accounts.futarchy_authority;

        if let Some(futarchy_treasury) = args.futarchy_treasury {
            futarchy_authority.recipients.futarchy_treasury = futarchy_treasury;
        }
        if let Some(buybacks_vault) = args.buybacks_vault {
            futarchy_authority.recipients.buybacks_vault = buybacks_vault;
        }
        if let Some(team_treasury) = args.team_treasury {
            futarchy_authority.recipients.team_treasury = team_treasury;
        }

        Ok(())
    }
}
