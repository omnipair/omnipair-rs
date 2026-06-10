use anchor_lang::{prelude::*, solana_program::program_option::COption};
use anchor_spl::{
    token::Token,
    token_interface::{Mint, Token2022, TokenAccount},
};

use crate::{
    constants::*,
    errors::ErrorCode,
    events::{MarketEventMetadata, MarketHedgeClosed, MarketHedgeOpened},
    generate_market_seeds,
    shared::{
        account::get_size_with_discriminator,
        token::{
            token_burn, token_mint_to, transfer_from_user_to_vault, transfer_from_vault_to_user,
        },
    },
    v2::state::{HedgePosition, Market},
};

use super::common::{require_fee_free_claim_mint, token_program_for_mint};

#[derive(AnchorSerialize, AnchorDeserialize, Clone)]
pub struct OpenHedgeArgs {
    pub market_side_index: u8,
    pub claim_amount: u64,
    pub min_hedge_amount: u64,
}

#[derive(AnchorSerialize, AnchorDeserialize, Clone)]
pub struct CloseHedgeArgs {
    pub market_side_index: u8,
    pub hedge_amount: u64,
    pub min_claim_amount_out: u64,
}

#[event_cpi]
#[derive(Accounts)]
#[instruction(args: OpenHedgeArgs)]
pub struct OpenHedge<'info> {
    #[account(
        mut,
        seeds = [
            MARKET_SEED_PREFIX,
            market.asset0_mint.as_ref(),
            market.asset1_mint.as_ref(),
            market.params_hash.as_ref(),
        ],
        bump = market.bump
    )]
    pub market: Box<Account<'info, Market>>,

    #[account(mut)]
    pub owner: Signer<'info>,

    pub asset_mint: Box<InterfaceAccount<'info, Mint>>,

    pub claim_mint: Box<InterfaceAccount<'info, Mint>>,

    #[account(mut)]
    pub hedge_mint: Box<InterfaceAccount<'info, Mint>>,

    #[account(mut)]
    pub hedge_vault: Box<InterfaceAccount<'info, TokenAccount>>,

    #[account(mut)]
    pub owner_claim_account: Box<InterfaceAccount<'info, TokenAccount>>,

    #[account(mut)]
    pub owner_hedge_account: Box<InterfaceAccount<'info, TokenAccount>>,

    #[account(
        init_if_needed,
        payer = owner,
        space = get_size_with_discriminator::<HedgePosition>(),
        seeds = [
            HEDGE_POSITION_SEED_PREFIX,
            market.key().as_ref(),
            owner.key().as_ref(),
            asset_mint.key().as_ref(),
        ],
        bump
    )]
    pub hedge_position: Box<Account<'info, HedgePosition>>,

    pub token_program: Program<'info, Token>,
    pub token_2022_program: Program<'info, Token2022>,
    pub system_program: Program<'info, System>,
}

impl<'info> OpenHedge<'info> {
    pub fn validate(&self, args: &OpenHedgeArgs) -> Result<()> {
        self.market.assert_started()?;
        require!(
            self.market.config.hedged_lp_enabled,
            ErrorCode::InvalidMarketConfig
        );
        require!(args.claim_amount > 0, ErrorCode::AmountZero);
        require_gte!(
            self.owner_claim_account.amount,
            args.claim_amount,
            ErrorCode::InsufficientBalance
        );
        validate_hedge_accounts(
            &self.market,
            args.market_side_index,
            self.owner.key(),
            &self.asset_mint,
            &self.claim_mint,
            &self.hedge_mint,
            &self.hedge_vault,
            &self.owner_claim_account,
            &self.owner_hedge_account,
        )?;
        if self.hedge_position.is_initialized() {
            self.hedge_position.assert_position(
                self.owner.key(),
                self.market.key(),
                self.asset_mint.key(),
            )?;
        }
        Ok(())
    }

    pub fn handle_open(ctx: Context<Self>, args: OpenHedgeArgs) -> Result<()> {
        let market_key = ctx.accounts.market.key();
        let owner_key = ctx.accounts.owner.key();
        let asset_mint_key = ctx.accounts.asset_mint.key();

        if !ctx.accounts.hedge_position.is_initialized() {
            ctx.accounts.hedge_position.initialize(
                owner_key,
                market_key,
                asset_mint_key,
                ctx.bumps.hedge_position,
            );
        }
        ctx.accounts
            .hedge_position
            .assert_position(owner_key, market_key, asset_mint_key)?;

        let hedge_vault_before = ctx.accounts.hedge_vault.amount;
        let claim_token_program = token_program_for_mint(
            &ctx.accounts.claim_mint,
            &ctx.accounts.token_program,
            &ctx.accounts.token_2022_program,
        )?;
        transfer_from_user_to_vault(
            ctx.accounts.owner.to_account_info(),
            ctx.accounts.owner_claim_account.to_account_info(),
            ctx.accounts.hedge_vault.to_account_info(),
            ctx.accounts.claim_mint.to_account_info(),
            claim_token_program,
            args.claim_amount,
            ctx.accounts.claim_mint.decimals,
        )?;
        ctx.accounts.hedge_vault.reload()?;
        let claim_credit = ctx
            .accounts
            .hedge_vault
            .amount
            .checked_sub(hedge_vault_before)
            .ok_or(ErrorCode::MarketMathOverflow)?;
        require_gte!(
            claim_credit,
            args.min_hedge_amount,
            ErrorCode::SlippageExceeded
        );
        require!(claim_credit > 0, ErrorCode::AmountZero);

        let hedged_claim_supply = {
            let market_side = ctx.accounts.market.side_mut(args.market_side_index)?;
            market_side.claim_ledger.hedged_claim_supply = market_side
                .claim_ledger
                .hedged_claim_supply
                .checked_add(claim_credit)
                .ok_or(ErrorCode::MarketMathOverflow)?;
            market_side.claim_ledger.hedged_claim_supply
        };
        ctx.accounts.hedge_position.increase(claim_credit)?;

        let hedge_token_program = token_program_for_mint(
            &ctx.accounts.hedge_mint,
            &ctx.accounts.token_program,
            &ctx.accounts.token_2022_program,
        )?;
        token_mint_to(
            ctx.accounts.market.to_account_info(),
            hedge_token_program,
            ctx.accounts.hedge_mint.to_account_info(),
            ctx.accounts.owner_hedge_account.to_account_info(),
            claim_credit,
            &[&generate_market_seeds!(ctx.accounts.market)[..]],
        )?;

        emit_cpi!(MarketHedgeOpened {
            market: market_key,
            owner: owner_key,
            asset_mint: asset_mint_key,
            claim_amount: claim_credit,
            hedge_amount: claim_credit,
            hedged_claim_supply,
            metadata: MarketEventMetadata::new(owner_key, market_key),
        });

        Ok(())
    }
}

#[event_cpi]
#[derive(Accounts)]
#[instruction(args: CloseHedgeArgs)]
pub struct CloseHedge<'info> {
    #[account(
        mut,
        seeds = [
            MARKET_SEED_PREFIX,
            market.asset0_mint.as_ref(),
            market.asset1_mint.as_ref(),
            market.params_hash.as_ref(),
        ],
        bump = market.bump
    )]
    pub market: Box<Account<'info, Market>>,

    #[account(mut)]
    pub owner: Signer<'info>,

    pub asset_mint: Box<InterfaceAccount<'info, Mint>>,

    pub claim_mint: Box<InterfaceAccount<'info, Mint>>,

    #[account(mut)]
    pub hedge_mint: Box<InterfaceAccount<'info, Mint>>,

    #[account(mut)]
    pub hedge_vault: Box<InterfaceAccount<'info, TokenAccount>>,

    #[account(mut)]
    pub owner_claim_account: Box<InterfaceAccount<'info, TokenAccount>>,

    #[account(mut)]
    pub owner_hedge_account: Box<InterfaceAccount<'info, TokenAccount>>,

    #[account(
        mut,
        seeds = [
            HEDGE_POSITION_SEED_PREFIX,
            market.key().as_ref(),
            owner.key().as_ref(),
            asset_mint.key().as_ref(),
        ],
        bump = hedge_position.bump
    )]
    pub hedge_position: Box<Account<'info, HedgePosition>>,

    pub token_program: Program<'info, Token>,
    pub token_2022_program: Program<'info, Token2022>,
}

impl<'info> CloseHedge<'info> {
    pub fn validate(&self, args: &CloseHedgeArgs) -> Result<()> {
        self.market.assert_started()?;
        require!(args.hedge_amount > 0, ErrorCode::AmountZero);
        require_gte!(
            self.owner_hedge_account.amount,
            args.hedge_amount,
            ErrorCode::InsufficientBalance
        );
        validate_hedge_accounts(
            &self.market,
            args.market_side_index,
            self.owner.key(),
            &self.asset_mint,
            &self.claim_mint,
            &self.hedge_mint,
            &self.hedge_vault,
            &self.owner_claim_account,
            &self.owner_hedge_account,
        )?;
        self.hedge_position.assert_position(
            self.owner.key(),
            self.market.key(),
            self.asset_mint.key(),
        )?;
        require_gte!(
            self.hedge_position.hedged_claim_amount,
            args.hedge_amount,
            ErrorCode::InvalidHedgePosition
        );
        Ok(())
    }

    pub fn handle_close(ctx: Context<Self>, args: CloseHedgeArgs) -> Result<()> {
        let market_key = ctx.accounts.market.key();
        let owner_key = ctx.accounts.owner.key();
        let asset_mint_key = ctx.accounts.asset_mint.key();

        let hedge_token_program = token_program_for_mint(
            &ctx.accounts.hedge_mint,
            &ctx.accounts.token_program,
            &ctx.accounts.token_2022_program,
        )?;
        token_burn(
            ctx.accounts.owner.to_account_info(),
            hedge_token_program,
            ctx.accounts.hedge_mint.to_account_info(),
            ctx.accounts.owner_hedge_account.to_account_info(),
            args.hedge_amount,
            &[],
        )?;

        let hedged_claim_supply = {
            let market_side = ctx.accounts.market.side_mut(args.market_side_index)?;
            market_side.claim_ledger.hedged_claim_supply = market_side
                .claim_ledger
                .hedged_claim_supply
                .checked_sub(args.hedge_amount)
                .ok_or(ErrorCode::MarketMathOverflow)?;
            market_side.claim_ledger.hedged_claim_supply
        };
        ctx.accounts.hedge_position.decrease(args.hedge_amount)?;

        let claim_token_program = token_program_for_mint(
            &ctx.accounts.claim_mint,
            &ctx.accounts.token_program,
            &ctx.accounts.token_2022_program,
        )?;
        transfer_from_vault_to_user(
            ctx.accounts.market.to_account_info(),
            ctx.accounts.hedge_vault.to_account_info(),
            ctx.accounts.owner_claim_account.to_account_info(),
            ctx.accounts.claim_mint.to_account_info(),
            claim_token_program,
            args.hedge_amount,
            ctx.accounts.claim_mint.decimals,
            &[&generate_market_seeds!(ctx.accounts.market)[..]],
        )?;
        require_gte!(
            args.hedge_amount,
            args.min_claim_amount_out,
            ErrorCode::SlippageExceeded
        );

        emit_cpi!(MarketHedgeClosed {
            market: market_key,
            owner: owner_key,
            asset_mint: asset_mint_key,
            hedge_amount: args.hedge_amount,
            claim_amount: args.hedge_amount,
            hedged_claim_supply,
            metadata: MarketEventMetadata::new(owner_key, market_key),
        });

        Ok(())
    }
}

fn validate_hedge_accounts<'info>(
    market: &Account<'info, Market>,
    market_side_index: u8,
    owner: Pubkey,
    asset_mint: &InterfaceAccount<'info, Mint>,
    claim_mint: &InterfaceAccount<'info, Mint>,
    hedge_mint: &InterfaceAccount<'info, Mint>,
    hedge_vault: &InterfaceAccount<'info, TokenAccount>,
    owner_claim_account: &InterfaceAccount<'info, TokenAccount>,
    owner_hedge_account: &InterfaceAccount<'info, TokenAccount>,
) -> Result<()> {
    let market_side = market.side(market_side_index)?;
    require_keys_eq!(
        market_side.asset_mint,
        asset_mint.key(),
        ErrorCode::InvalidMint
    );
    require_keys_eq!(
        market_side.claim_mint,
        claim_mint.key(),
        ErrorCode::InvalidClaimMint
    );
    require_keys_eq!(
        market_side.hedge_mint,
        hedge_mint.key(),
        ErrorCode::InvalidMint
    );
    require_keys_eq!(
        market_side.hedge_vault,
        hedge_vault.key(),
        ErrorCode::InvalidVault
    );
    require_keys_eq!(hedge_vault.mint, claim_mint.key(), ErrorCode::InvalidVault);
    require_keys_eq!(hedge_vault.owner, market.key(), ErrorCode::InvalidVault);
    require_keys_eq!(
        owner_claim_account.mint,
        claim_mint.key(),
        ErrorCode::InvalidTokenAccount
    );
    require_keys_eq!(
        owner_claim_account.owner,
        owner,
        ErrorCode::InvalidTokenAccount
    );
    require_keys_eq!(
        owner_hedge_account.mint,
        hedge_mint.key(),
        ErrorCode::InvalidTokenAccount
    );
    require_keys_eq!(
        owner_hedge_account.owner,
        owner,
        ErrorCode::InvalidTokenAccount
    );
    require!(
        hedge_mint.mint_authority == COption::Some(market.key()),
        ErrorCode::InvalidMint
    );
    require_fee_free_claim_mint(claim_mint)?;
    require_fee_free_claim_mint(hedge_mint)
}
