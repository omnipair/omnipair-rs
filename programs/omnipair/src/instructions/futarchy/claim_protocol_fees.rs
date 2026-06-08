use crate::{
    constants::*,
    errors::ErrorCode,
    events::{ClaimProtocolFeesEvent, EventMetadata},
    generate_gamm_pair_seeds,
    state::*,
    utils::token::{require_supported_mint, token_program_for_mint, transfer_from_vault_to_vault},
};
use anchor_lang::prelude::*;
use anchor_spl::{
    associated_token::{
        create_idempotent, get_associated_token_address_with_program_id, AssociatedToken, Create,
    },
    token::Token,
    token_interface::{Mint, Token2022, TokenAccount},
};

/// Claims protocol fees from a pair and distributes them directly to revenue recipients.
///
/// This instruction is permissionless - anyone can call it to trigger fee distribution.
/// Fees are transferred directly from pair reserve vaults to recipient ATAs based on
/// the distribution percentages stored in FutarchyAuthority.
///
/// The recipient addresses in FutarchyAuthority are pubkeys not ATAs.
/// ATAs are derived at runtime for each token being claimed.
#[event_cpi]
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
    pub pair: Box<Account<'info, Pair>>,

    #[account(
        mut,
        address = pair.rate_model,
    )]
    pub rate_model: Box<Account<'info, RateModel>>,

    #[account(
        seeds = [FUTARCHY_AUTHORITY_SEED_PREFIX],
        bump = futarchy_authority.bump
    )]
    pub futarchy_authority: Box<Account<'info, FutarchyAuthority>>,

    // Reserve Vaults (source of fees) - boxed to reduce stack usage
    #[account(
        mut,
        seeds = [
            RESERVE_VAULT_SEED_PREFIX,
            pair.key().as_ref(),
            pair.token0.as_ref(),
        ],
        bump = pair.vault_bumps.reserve0
    )]
    pub reserve0_vault: Box<InterfaceAccount<'info, TokenAccount>>,

    #[account(
        mut,
        seeds = [
            RESERVE_VAULT_SEED_PREFIX,
            pair.key().as_ref(),
            pair.token1.as_ref(),
        ],
        bump = pair.vault_bumps.reserve1
    )]
    pub reserve1_vault: Box<InterfaceAccount<'info, TokenAccount>>,

    // Token Mints
    #[account(address = pair.token0)]
    pub token0_mint: Box<InterfaceAccount<'info, Mint>>,

    #[account(address = pair.token1)]
    pub token1_mint: Box<InterfaceAccount<'info, Mint>>,

    // Futarchy Treasury ATAs (boxed to reduce stack usage)
    #[account(mut)]
    /// CHECK: derived and created idempotently in handler with token0_mint's token program
    pub futarchy_treasury_token0: UncheckedAccount<'info>,

    #[account(mut)]
    /// CHECK: derived and created idempotently in handler with token1_mint's token program
    pub futarchy_treasury_token1: UncheckedAccount<'info>,

    /// CHECK: Validated against futarchy_authority.recipients.futarchy_treasury
    #[account(address = futarchy_authority.recipients.futarchy_treasury @ ErrorCode::InvalidRecipient)]
    pub futarchy_treasury: AccountInfo<'info>,

    // Buybacks Vault ATAs (boxed to reduce stack usage)
    #[account(mut)]
    /// CHECK: derived and created idempotently in handler with token0_mint's token program
    pub buybacks_vault_token0: UncheckedAccount<'info>,

    #[account(mut)]
    /// CHECK: derived and created idempotently in handler with token1_mint's token program
    pub buybacks_vault_token1: UncheckedAccount<'info>,

    /// CHECK: Validated against futarchy_authority.recipients.buybacks_vault
    #[account(address = futarchy_authority.recipients.buybacks_vault @ ErrorCode::InvalidRecipient)]
    pub buybacks_vault: AccountInfo<'info>,

    // Team Treasury ATAs (boxed to reduce stack usage)
    #[account(mut)]
    /// CHECK: derived and created idempotently in handler with token0_mint's token program
    pub team_treasury_token0: UncheckedAccount<'info>,

    #[account(mut)]
    /// CHECK: derived and created idempotently in handler with token1_mint's token program
    pub team_treasury_token1: UncheckedAccount<'info>,

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
        self.pair.update(
            &self.rate_model,
            &self.futarchy_authority,
            pair_key,
            Some(self.event_authority.to_account_info()),
        )?;
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
        require_supported_mint(&ctx.accounts.token0_mint)?;
        require_supported_mint(&ctx.accounts.token1_mint)?;

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
        let token0_program = token_program_for_mint(
            &ctx.accounts.token0_mint.to_account_info(),
            &ctx.accounts.token_program.to_account_info(),
            &ctx.accounts.token_2022_program.to_account_info(),
        )?;

        let token1_program = token_program_for_mint(
            &ctx.accounts.token1_mint.to_account_info(),
            &ctx.accounts.token_program.to_account_info(),
            &ctx.accounts.token_2022_program.to_account_info(),
        )?;

        create_recipient_ata(
            &caller.to_account_info(),
            &ctx.accounts.futarchy_treasury_token0.to_account_info(),
            &ctx.accounts.futarchy_treasury.to_account_info(),
            &ctx.accounts.token0_mint.to_account_info(),
            &ctx.accounts.system_program.to_account_info(),
            &token0_program,
            &ctx.accounts.associated_token_program.to_account_info(),
        )?;
        create_recipient_ata(
            &caller.to_account_info(),
            &ctx.accounts.futarchy_treasury_token1.to_account_info(),
            &ctx.accounts.futarchy_treasury.to_account_info(),
            &ctx.accounts.token1_mint.to_account_info(),
            &ctx.accounts.system_program.to_account_info(),
            &token1_program,
            &ctx.accounts.associated_token_program.to_account_info(),
        )?;
        create_recipient_ata(
            &caller.to_account_info(),
            &ctx.accounts.buybacks_vault_token0.to_account_info(),
            &ctx.accounts.buybacks_vault.to_account_info(),
            &ctx.accounts.token0_mint.to_account_info(),
            &ctx.accounts.system_program.to_account_info(),
            &token0_program,
            &ctx.accounts.associated_token_program.to_account_info(),
        )?;
        create_recipient_ata(
            &caller.to_account_info(),
            &ctx.accounts.buybacks_vault_token1.to_account_info(),
            &ctx.accounts.buybacks_vault.to_account_info(),
            &ctx.accounts.token1_mint.to_account_info(),
            &ctx.accounts.system_program.to_account_info(),
            &token1_program,
            &ctx.accounts.associated_token_program.to_account_info(),
        )?;
        create_recipient_ata(
            &caller.to_account_info(),
            &ctx.accounts.team_treasury_token0.to_account_info(),
            &ctx.accounts.team_treasury.to_account_info(),
            &ctx.accounts.token0_mint.to_account_info(),
            &ctx.accounts.system_program.to_account_info(),
            &token0_program,
            &ctx.accounts.associated_token_program.to_account_info(),
        )?;
        create_recipient_ata(
            &caller.to_account_info(),
            &ctx.accounts.team_treasury_token1.to_account_info(),
            &ctx.accounts.team_treasury.to_account_info(),
            &ctx.accounts.token1_mint.to_account_info(),
            &ctx.accounts.system_program.to_account_info(),
            &token1_program,
            &ctx.accounts.associated_token_program.to_account_info(),
        )?;

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
        emit_cpi!(ClaimProtocolFeesEvent {
            metadata: EventMetadata::new(caller.key(), pair.key()),
            token0: pair.token0,
            token1: pair.token1,
            futarchy_treasury_amount0: futarchy_amount0,
            futarchy_treasury_amount1: futarchy_amount1,
            buybacks_vault_amount0: buybacks_amount0,
            buybacks_vault_amount1: buybacks_amount1,
            team_treasury_amount0: team_amount0,
            team_treasury_amount1: team_amount1,
        });

        Ok(())
    }
}

fn create_recipient_ata<'info>(
    payer: &AccountInfo<'info>,
    associated_token: &AccountInfo<'info>,
    authority: &AccountInfo<'info>,
    mint: &AccountInfo<'info>,
    system_program: &AccountInfo<'info>,
    token_program: &AccountInfo<'info>,
    associated_token_program: &AccountInfo<'info>,
) -> Result<()> {
    let expected =
        get_associated_token_address_with_program_id(authority.key, mint.key, token_program.key);
    require_keys_eq!(
        expected,
        associated_token.key(),
        ErrorCode::InvalidTokenAccount
    );
    create_idempotent(CpiContext::new(
        associated_token_program.clone(),
        Create {
            payer: payer.clone(),
            associated_token: associated_token.clone(),
            authority: authority.clone(),
            mint: mint.clone(),
            system_program: system_program.clone(),
            token_program: token_program.clone(),
        },
    ))
}
