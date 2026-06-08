use anchor_lang::prelude::*;
use anchor_lang::solana_program::bpf_loader_upgradeable::UpgradeableLoaderState;
use bincode::Options;
use crate::state::futarchy_authority::FutarchyAuthority;
use crate::constants::{FUTARCHY_AUTHORITY_SEED_PREFIX, BPS_DENOMINATOR};
use crate::utils::account::get_size_with_discriminator;
use crate::errors::ErrorCode;

#[derive(AnchorSerialize, AnchorDeserialize, Clone)]
pub struct InitFutarchyAuthorityArgs {
    pub authority: Pubkey,
    pub swap_bps: u16,
    pub interest_bps: u16,
    pub futarchy_treasury: Pubkey,
    pub futarchy_treasury_bps: u16,
    pub buybacks_vault: Pubkey,
    pub buybacks_vault_bps: u16,
    pub team_treasury: Pubkey,
    pub team_treasury_bps: u16,
}


#[derive(Accounts)]
pub struct InitFutarchyAuthority<'info> {
    #[account(mut)]
    pub deployer: Signer<'info>,

    #[account(
        init,
        payer = deployer,
        space = get_size_with_discriminator::<FutarchyAuthority>(),
        seeds = [FUTARCHY_AUTHORITY_SEED_PREFIX],
        bump
    )]
    pub futarchy_authority: Account<'info, FutarchyAuthority>,

    /// CHECK: Safe - PDA derivation enforced by seeds, owner validated in handle_init
    #[account(
        seeds = [crate::ID.as_ref()],
        bump,
        seeds::program = anchor_lang::solana_program::bpf_loader_upgradeable::ID
    )]
    pub program_data: AccountInfo<'info>,

    pub system_program: Program<'info, System>,
}

impl<'info> InitFutarchyAuthority<'info> {
    pub fn handle_init(ctx: Context<Self>, args: InitFutarchyAuthorityArgs) -> Result<()> {
        // Validate deployer is the program's upgrade authority
        let program_data = &ctx.accounts.program_data;
        
        // Explicit owner check - ensures program_data is owned by BPF Loader Upgradeable
        require_keys_eq!(
            *program_data.owner,
            anchor_lang::solana_program::bpf_loader_upgradeable::ID,
            ErrorCode::InvalidDeployer
        );
        
        let data = program_data.try_borrow_data()?;
        
        // Deserialize using Solana's UpgradeableLoaderState type
        // Use allow_trailing_bytes() because ProgramData accounts may have padding
        let loader_state: UpgradeableLoaderState = bincode::DefaultOptions::new()
            .with_fixint_encoding()
            .allow_trailing_bytes()
            .deserialize(&data)
            .map_err(|_| ErrorCode::InvalidDeployer)?;
        
        // Extract upgrade authority from ProgramData variant
        let upgrade_authority = match loader_state {
            UpgradeableLoaderState::ProgramData { upgrade_authority_address, .. } => {
                // If upgrade_authority_address is None, program is immutable
                upgrade_authority_address.ok_or(ErrorCode::InvalidDeployer)?
            }
            _ => return Err(ErrorCode::InvalidDeployer.into()),
        };
        
        require_keys_eq!(
            ctx.accounts.deployer.key(),
            upgrade_authority,
            ErrorCode::InvalidDeployer
        );

        // Validate protocol fees are within bounds
        require_gte!(BPS_DENOMINATOR, args.swap_bps, ErrorCode::InvalidSwapFeeBps);
        require_gte!(BPS_DENOMINATOR, args.interest_bps, ErrorCode::InvalidInterestFeeBps);

        // Validate percentages sum to 100%
        let total_percentage = args.futarchy_treasury_bps
            .checked_add(args.buybacks_vault_bps)
            .ok_or(ErrorCode::FeeMathOverflow)?
            .checked_add(args.team_treasury_bps)
            .ok_or(ErrorCode::FeeMathOverflow)?;

        require_eq!(
            total_percentage,
            BPS_DENOMINATOR,
            ErrorCode::InvalidDistribution
        );

        let futarchy_authority = &mut ctx.accounts.futarchy_authority;
        
        let authority = FutarchyAuthority::initialize(
            args.authority,
            args.swap_bps,
            args.interest_bps,
            args.futarchy_treasury,
            args.buybacks_vault,
            args.team_treasury,
            args.futarchy_treasury_bps,
            args.buybacks_vault_bps,
            args.team_treasury_bps,
            ctx.bumps.futarchy_authority,
        )?;
        
        futarchy_authority.set_inner(authority);

        Ok(())
    }
}
