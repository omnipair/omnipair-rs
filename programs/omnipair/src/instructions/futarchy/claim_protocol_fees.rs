use anchor_lang::prelude::*;
use anchor_spl::{
    token::{Token, TokenAccount, Mint},
    token_interface::Token2022,
    associated_token::AssociatedToken,
};
use crate::{
    state::*,
    constants::*,
    errors::ErrorCode,
    events::{ClaimProtocolFeesEvent, EventMetadata},
    utils::token::transfer_from_vault_to_vault,
    generate_gamm_pair_seeds,
};

/// Claims protocol fees from a pair and distributes them directly to revenue recipients.
/// 
/// This instruction is permissionless - anyone can call it to trigger fee distribution.
/// Fees are transferred directly from pair reserve vaults to recipient ATAs based on
/// the distribution percentages stored in FutarchyAuthority.
/// 
/// The recipient addresses in FutarchyAuthority are pubkeys not ATAs.
/// ATAs are derived at runtime for each token being claimed.
#[derive(Accounts)]
pub struct ClaimProtocolFees<'info> {
    /// Anyone can call this instruction
    #[account(mut)]
    pub caller: Signer<'info>,

    #[account(
        mut,
        seeds = [PAIR_SEED_PREFIX, pair.token0.as_ref(), pair.token1.as_ref(), pair.params_hash.as_ref()],
        bump = pair.bump
    )]
    pub pair: Account<'info, Pair>,

    #[account(
        mut,
        address = pair.rate_model,
    )]
    pub rate_model: Account<'info, RateModel>,

    #[account(
        seeds = [FUTARCHY_AUTHORITY_SEED_PREFIX],
        bump = futarchy_authority.bump
    )]
    pub futarchy_authority: Account<'info, FutarchyAuthority>,

    // Reserve Vaults (source of fees)
    #[account(
        mut,
        seeds = [
            RESERVE_VAULT_SEED_PREFIX,
            pair.key().as_ref(),
            pair.token0.as_ref(),
        ],
        bump = pair.vault_bumps.reserve0
    )]
    pub reserve0_vault: Account<'info, TokenAccount>,

    #[account(
        mut,
        seeds = [
            RESERVE_VAULT_SEED_PREFIX,
            pair.key().as_ref(),
            pair.token1.as_ref(),
        ],
        bump = pair.vault_bumps.reserve1
    )]
    pub reserve1_vault: Account<'info, TokenAccount>,

    // Token Mints
    #[account(address = pair.token0)]
    pub token0_mint: Box<Account<'info, Mint>>,
    
    #[account(address = pair.token1)]
    pub token1_mint: Box<Account<'info, Mint>>,

    // Futarchy Treasury ATAs
    #[account(
        init_if_needed,
        payer = caller,
        associated_token::mint = token0_mint,
        associated_token::authority = futarchy_treasury,
    )]
    pub futarchy_treasury_token0: Account<'info, TokenAccount>,

    #[account(
        init_if_needed,
        payer = caller,
        associated_token::mint = token1_mint,
        associated_token::authority = futarchy_treasury,
    )]
    pub futarchy_treasury_token1: Account<'info, TokenAccount>,

    /// CHECK: Validated against futarchy_authority.recipients.futarchy_treasury
    #[account(address = futarchy_authority.recipients.futarchy_treasury @ ErrorCode::InvalidRecipient)]
    pub futarchy_treasury: AccountInfo<'info>,

    // Buybacks Vault ATAs
    #[account(
        init_if_needed,
        payer = caller,
        associated_token::mint = token0_mint,
        associated_token::authority = buybacks_vault,
    )]
    pub buybacks_vault_token0: Account<'info, TokenAccount>,

    #[account(
        init_if_needed,
        payer = caller,
        associated_token::mint = token1_mint,
        associated_token::authority = buybacks_vault,
    )]
    pub buybacks_vault_token1: Account<'info, TokenAccount>,

    /// CHECK: Validated against futarchy_authority.recipients.buybacks_vault
    #[account(address = futarchy_authority.recipients.buybacks_vault @ ErrorCode::InvalidRecipient)]
    pub buybacks_vault: AccountInfo<'info>,

    // Team Treasury ATAs
    #[account(
        init_if_needed,
        payer = caller,
        associated_token::mint = token0_mint,
        associated_token::authority = team_treasury,
    )]
    pub team_treasury_token0: Account<'info, TokenAccount>,

    #[account(
        init_if_needed,
        payer = caller,
        associated_token::mint = token1_mint,
        associated_token::authority = team_treasury,
    )]
    pub team_treasury_token1: Account<'info, TokenAccount>,

    /// CHECK: Validated against futarchy_authority.recipients.team_treasury
    #[account(address = futarchy_authority.recipients.team_treasury @ ErrorCode::InvalidRecipient)]
    pub team_treasury: AccountInfo<'info>,

    pub token_program: Program<'info, Token>,
    pub token_2022_program: Program<'info, Token2022>,
    pub associated_token_program: Program<'info, AssociatedToken>,
    pub system_program: Program<'info, System>,
}

impl<'info> ClaimProtocolFees<'info> {
    pub fn update(&mut self) -> Result<()> {
        let pair_key = self.pair.to_account_info().key();
        self.pair.update(&self.rate_model, &self.futarchy_authority, pair_key)?;
        Ok(())
    }

    pub fn handle_claim(ctx: Context<Self>) -> Result<()> {
        let ClaimProtocolFees { 
            pair, 
            reserve0_vault, 
            reserve1_vault, 
            futarchy_authority,
            caller,
            .. 
        } = ctx.accounts;

        // Defensive check: ensure distribution percentages sum to 100%
        require!(
            futarchy_authority.revenue_distribution.is_valid(),
            ErrorCode::InvalidDistribution
        );

        // Calculate claimable amounts (fees accumulated in vaults beyond cash reserves)
        let claimable_amount0 = reserve0_vault.amount.saturating_sub(pair.cash_reserve0);
        let claimable_amount1 = reserve1_vault.amount.saturating_sub(pair.cash_reserve1);

        // Calculate amounts for each recipient (token0)
        let buybacks_amount0 = (claimable_amount0 as u128)
            .checked_mul(futarchy_authority.revenue_distribution.buybacks_vault_bps as u128)
            .ok_or(ErrorCode::FeeMathOverflow)?
            .checked_div(BPS_DENOMINATOR as u128)
            .ok_or(ErrorCode::FeeMathOverflow)? as u64;

        let team_amount0 = (claimable_amount0 as u128)
            .checked_mul(futarchy_authority.revenue_distribution.team_treasury_bps as u128)
            .ok_or(ErrorCode::FeeMathOverflow)?
            .checked_div(BPS_DENOMINATOR as u128)
            .ok_or(ErrorCode::FeeMathOverflow)? as u64;

        // Futarchy treasury gets the remainder (handles rounding dust)
        let futarchy_amount0 = claimable_amount0
            .saturating_sub(buybacks_amount0)
            .saturating_sub(team_amount0);

        // Calculate amounts for each recipient (token1)
        let buybacks_amount1 = (claimable_amount1 as u128)
            .checked_mul(futarchy_authority.revenue_distribution.buybacks_vault_bps as u128)
            .ok_or(ErrorCode::FeeMathOverflow)?
            .checked_div(BPS_DENOMINATOR as u128)
            .ok_or(ErrorCode::FeeMathOverflow)? as u64;

        let team_amount1 = (claimable_amount1 as u128)
            .checked_mul(futarchy_authority.revenue_distribution.team_treasury_bps as u128)
            .ok_or(ErrorCode::FeeMathOverflow)?
            .checked_div(BPS_DENOMINATOR as u128)
            .ok_or(ErrorCode::FeeMathOverflow)? as u64;

        // Futarchy treasury gets the remainder (handles rounding dust)
        let futarchy_amount1 = claimable_amount1
            .saturating_sub(buybacks_amount1)
            .saturating_sub(team_amount1);

        let pair_seeds = generate_gamm_pair_seeds!(pair);
        let signer_seeds = &[&pair_seeds[..]];

        // Determine token programs
        let token0_program = if ctx.accounts.token0_mint.to_account_info().owner == ctx.accounts.token_program.key {
            ctx.accounts.token_program.to_account_info()
        } else {
            ctx.accounts.token_2022_program.to_account_info()
        };

        let token1_program = if ctx.accounts.token1_mint.to_account_info().owner == ctx.accounts.token_program.key {
            ctx.accounts.token_program.to_account_info()
        } else {
            ctx.accounts.token_2022_program.to_account_info()
        };

        // Token0 transfers
        // Transfer to futarchy treasury
        if futarchy_amount0 > 0 {
            transfer_from_vault_to_vault(
                pair.to_account_info(),
                ctx.accounts.reserve0_vault.to_account_info(),
                ctx.accounts.futarchy_treasury_token0.to_account_info(),
                ctx.accounts.token0_mint.to_account_info(),
                token0_program.clone(),
                futarchy_amount0,
                ctx.accounts.token0_mint.decimals,
                signer_seeds,
            )?;
        }

        // Transfer to buybacks vault
        if buybacks_amount0 > 0 {
            transfer_from_vault_to_vault(
                pair.to_account_info(),
                ctx.accounts.reserve0_vault.to_account_info(),
                ctx.accounts.buybacks_vault_token0.to_account_info(),
                ctx.accounts.token0_mint.to_account_info(),
                token0_program.clone(),
                buybacks_amount0,
                ctx.accounts.token0_mint.decimals,
                signer_seeds,
            )?;
        }

        // Transfer to team treasury
        if team_amount0 > 0 {
            transfer_from_vault_to_vault(
                pair.to_account_info(),
                ctx.accounts.reserve0_vault.to_account_info(),
                ctx.accounts.team_treasury_token0.to_account_info(),
                ctx.accounts.token0_mint.to_account_info(),
                token0_program,
                team_amount0,
                ctx.accounts.token0_mint.decimals,
                signer_seeds,
            )?;
        }

        // Token1 transfers
        // Transfer to futarchy treasury
        if futarchy_amount1 > 0 {
            transfer_from_vault_to_vault(
                pair.to_account_info(),
                ctx.accounts.reserve1_vault.to_account_info(),
                ctx.accounts.futarchy_treasury_token1.to_account_info(),
                ctx.accounts.token1_mint.to_account_info(),
                token1_program.clone(),
                futarchy_amount1,
                ctx.accounts.token1_mint.decimals,
                signer_seeds,
            )?;
        }

        // Transfer to buybacks vault
        if buybacks_amount1 > 0 {
            transfer_from_vault_to_vault(
                pair.to_account_info(),
                ctx.accounts.reserve1_vault.to_account_info(),
                ctx.accounts.buybacks_vault_token1.to_account_info(),
                ctx.accounts.token1_mint.to_account_info(),
                token1_program.clone(),
                buybacks_amount1,
                ctx.accounts.token1_mint.decimals,
                signer_seeds,
            )?;
        }

        // Transfer to team treasury
        if team_amount1 > 0 {
            transfer_from_vault_to_vault(
                pair.to_account_info(),
                ctx.accounts.reserve1_vault.to_account_info(),
                ctx.accounts.team_treasury_token1.to_account_info(),
                ctx.accounts.token1_mint.to_account_info(),
                token1_program,
                team_amount1,
                ctx.accounts.token1_mint.decimals,
                signer_seeds,
            )?;
        }

        // Emit event for tracking
        emit!(ClaimProtocolFeesEvent {
            token0: pair.token0,
            token1: pair.token1,
            futarchy_treasury_amount0: futarchy_amount0,
            futarchy_treasury_amount1: futarchy_amount1,
            buybacks_vault_amount0: buybacks_amount0,
            buybacks_vault_amount1: buybacks_amount1,
            team_treasury_amount0: team_amount0,
            team_treasury_amount1: team_amount1,
            metadata: EventMetadata::new(caller.key(), pair.key()),
        });

        Ok(())
    }
}
