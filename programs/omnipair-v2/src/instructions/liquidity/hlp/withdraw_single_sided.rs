use anchor_lang::prelude::*;
use anchor_spl::{
    token::Token,
    token_interface::{Mint, Token2022, TokenAccount},
};

use crate::{
    constants::*,
    errors::ErrorCode,
    events::{HlpClosed, MarketEventMetadata, MarketHealthUpdated},
    generate_market_seeds,
    shared::{
        account::get_size_with_discriminator,
        token::{token_burn, transfer_from_vault_to_user, transfer_from_vault_to_vault},
    },
    state::{FutarchyAuthority, Market, MarketAsset, YieldAccount, YieldTokenKind},
};

use crate::instructions::common::{
    require_supported_asset_mint, token_account_credit, token_program_for_mint,
    validate_interest_accounts, validate_lp_mint, validate_owner_asset_account,
    validate_owner_lp_account, validate_side_vault_accounts,
};

#[derive(AnchorSerialize, AnchorDeserialize, Clone)]
pub struct WithdrawSingleSidedArgs {
    pub hlp_amount: u64,
    pub min_target_amount_out: u64,
}

#[event_cpi]
#[derive(Accounts)]
#[instruction(args: WithdrawSingleSidedArgs)]
pub struct WithdrawSingleSided<'info> {
    #[account(
        mut,
        seeds = [
            MARKET_V2_SEED_PREFIX,
            market.base_mint.as_ref(),
            market.quote_mint.as_ref(),
            market.params_hash.as_ref(),
        ],
        bump = market.bump
    )]
    pub market: Box<Account<'info, Market>>,

    #[account(
        seeds = [FUTARCHY_AUTHORITY_SEED_PREFIX],
        bump = futarchy_authority.bump
    )]
    pub futarchy_authority: Box<Account<'info, FutarchyAuthority>>,

    #[account(mut)]
    pub owner: Signer<'info>,

    pub base_mint: Box<InterfaceAccount<'info, Mint>>,
    pub quote_mint: Box<InterfaceAccount<'info, Mint>>,
    #[account(mut)]
    pub ylp_mint: Box<InterfaceAccount<'info, Mint>>,
    #[account(mut)]
    pub target_hlp_mint: Box<InterfaceAccount<'info, Mint>>,

    #[account(mut)]
    pub base_reserve_vault: Box<InterfaceAccount<'info, TokenAccount>>,
    #[account(mut)]
    pub quote_reserve_vault: Box<InterfaceAccount<'info, TokenAccount>>,

    #[account(mut)]
    pub borrowed_interest_vault: Box<InterfaceAccount<'info, TokenAccount>>,

    #[account(mut)]
    pub owner_target_account: Box<InterfaceAccount<'info, TokenAccount>>,
    #[account(mut)]
    pub owner_hlp_account: Box<InterfaceAccount<'info, TokenAccount>>,

    #[account(
        mut,
        seeds = [
            HLP_YLP_VAULT_SEED_PREFIX,
            market.key().as_ref(),
            target_hlp_mint.key().as_ref(),
            ylp_mint.key().as_ref(),
        ],
        bump,
        token::mint = ylp_mint,
        token::authority = market,
        token::token_program = token_2022_program,
    )]
    pub hlp_ylp_account: Box<InterfaceAccount<'info, TokenAccount>>,

    #[account(
        init_if_needed,
        payer = owner,
        space = get_size_with_discriminator::<YieldAccount>(),
        seeds = [
            YIELD_ACCOUNT_SEED_PREFIX,
            market.key().as_ref(),
            owner.key().as_ref(),
            owner_target_account.mint.as_ref(),
            &[YieldTokenKind::Hlp.code()],
        ],
        bump
    )]
    pub target_yield_account: Box<Account<'info, YieldAccount>>,

    pub token_program: Program<'info, Token>,
    pub token_2022_program: Program<'info, Token2022>,
    pub system_program: Program<'info, System>,
}

impl<'info> WithdrawSingleSided<'info> {
    pub fn validate(&self, args: &WithdrawSingleSidedArgs) -> Result<()> {
        self.market.assert_started()?;
        require!(args.hlp_amount > 0, ErrorCode::AmountZero);
        validate_side_vault_accounts(
            &self.market,
            MarketAsset::Base,
            &self.base_mint,
            &self.base_reserve_vault,
        )?;
        validate_side_vault_accounts(
            &self.market,
            MarketAsset::Quote,
            &self.quote_mint,
            &self.quote_reserve_vault,
        )?;
        require_keys_eq!(
            self.market.ylp_mint,
            self.ylp_mint.key(),
            ErrorCode::InvalidLpMintKey
        );
        let target_asset = self.market.asset_for_hlp_mint(self.target_hlp_mint.key())?;
        let target_mint = match target_asset {
            MarketAsset::Base => &self.base_mint,
            MarketAsset::Quote => &self.quote_mint,
        };
        let borrowed_asset = target_asset.opposite();
        let borrowed_mint = match borrowed_asset {
            MarketAsset::Base => &self.base_mint,
            MarketAsset::Quote => &self.quote_mint,
        };
        require_keys_eq!(
            self.market.side(target_asset)?.hlp_mint,
            self.target_hlp_mint.key(),
            ErrorCode::InvalidMint
        );
        let interest_asset =
            validate_interest_accounts(&self.market, borrowed_mint, &self.borrowed_interest_vault)?;
        require!(interest_asset == borrowed_asset, ErrorCode::InvalidVault);
        validate_owner_asset_account(self.owner.key(), target_mint, &self.owner_target_account)?;
        validate_owner_lp_account(
            self.owner.key(),
            &self.target_hlp_mint,
            &self.owner_hlp_account,
        )?;
        require_gte!(
            self.owner_hlp_account.amount,
            args.hlp_amount,
            ErrorCode::InsufficientBalance
        );
        validate_lp_mint(
            &self.target_hlp_mint,
            self.market.key(),
            target_mint.decimals,
        )?;
        validate_lp_mint(&self.ylp_mint, self.market.key(), self.base_mint.decimals)?;
        require_keys_eq!(
            self.hlp_ylp_account.mint,
            self.ylp_mint.key(),
            ErrorCode::InvalidTokenAccount
        );
        require_keys_eq!(
            self.hlp_ylp_account.owner,
            self.market.key(),
            ErrorCode::InvalidVault
        );
        require_supported_asset_mint(&self.base_mint)?;
        require_supported_asset_mint(&self.quote_mint)?;
        Ok(())
    }

    pub fn update(&mut self) -> Result<()> {
        self.market.update()
    }

    pub fn update_and_validate(&mut self, args: &WithdrawSingleSidedArgs) -> Result<()> {
        self.update()?;
        self.validate(args)
    }

    pub fn handle_withdraw(ctx: Context<Self>, args: WithdrawSingleSidedArgs) -> Result<()> {
        let market_key = ctx.accounts.market.key();
        let owner_key = ctx.accounts.owner.key();
        let target_asset = ctx
            .accounts
            .market
            .asset_for_hlp_mint(ctx.accounts.target_hlp_mint.key())?;
        let target_mint_key = match target_asset {
            MarketAsset::Base => ctx.accounts.base_mint.key(),
            MarketAsset::Quote => ctx.accounts.quote_mint.key(),
        };

        initialize_or_validate_hlp_yield_account(
            &mut ctx.accounts.target_yield_account,
            owner_key,
            market_key,
            target_mint_key,
            ctx.bumps.target_yield_account,
        )?;
        ctx.accounts
            .market
            .checkpoint_hlp_yield_from_ylp(target_asset)?;
        let (swap_fee_growth_index_nad, interest_growth_index_nad) =
            hlp_yield_growth_indexes(&ctx.accounts.market, target_asset);
        ctx.accounts.target_yield_account.accrue(
            ctx.accounts.owner_hlp_account.amount,
            swap_fee_growth_index_nad,
            interest_growth_index_nad,
        )?;

        let hlp_program = token_program_for_mint(
            &ctx.accounts.target_hlp_mint,
            &ctx.accounts.token_program,
            &ctx.accounts.token_2022_program,
        )?;
        token_burn(
            ctx.accounts.owner.to_account_info(),
            hlp_program,
            ctx.accounts.target_hlp_mint.to_account_info(),
            ctx.accounts.owner_hlp_account.to_account_info(),
            args.hlp_amount,
            &[],
        )?;

        let receipt = ctx
            .accounts
            .market
            .close_hedge(target_asset, args.hlp_amount)?;
        if receipt.interest_paid > 0 {
            let borrowed_asset = target_asset.opposite();
            let (borrowed_reserve_vault, borrowed_mint, borrowed_decimals) = match borrowed_asset {
                MarketAsset::Base => (
                    ctx.accounts.base_reserve_vault.to_account_info(),
                    ctx.accounts.base_mint.to_account_info(),
                    ctx.accounts.base_mint.decimals,
                ),
                MarketAsset::Quote => (
                    ctx.accounts.quote_reserve_vault.to_account_info(),
                    ctx.accounts.quote_mint.to_account_info(),
                    ctx.accounts.quote_mint.decimals,
                ),
            };
            let borrowed_token_program = token_program_for_mint(
                match borrowed_asset {
                    MarketAsset::Base => &ctx.accounts.base_mint,
                    MarketAsset::Quote => &ctx.accounts.quote_mint,
                },
                &ctx.accounts.token_program,
                &ctx.accounts.token_2022_program,
            )?;
            transfer_from_vault_to_vault(
                ctx.accounts.market.to_account_info(),
                borrowed_reserve_vault,
                ctx.accounts.borrowed_interest_vault.to_account_info(),
                borrowed_mint,
                borrowed_token_program,
                receipt.interest_paid,
                borrowed_decimals,
                &[&generate_market_seeds!(ctx.accounts.market)[..]],
            )?;
            ctx.accounts.borrowed_interest_vault.reload()?;
            let manager_fee_bps = ctx.accounts.market.config.manager_fee_bps;
            ctx.accounts
                .market
                .side_mut(borrowed_asset)?
                .record_interest_credit(
                    receipt.interest_paid,
                    manager_fee_bps,
                    ctx.accounts.futarchy_authority.revenue_share.interest_bps,
                    ctx.accounts.futarchy_authority.protocol_auction_split,
                )?;
        }

        let ylp_program = token_program_for_mint(
            &ctx.accounts.ylp_mint,
            &ctx.accounts.token_program,
            &ctx.accounts.token_2022_program,
        )?;
        token_burn(
            ctx.accounts.market.to_account_info(),
            ylp_program,
            ctx.accounts.ylp_mint.to_account_info(),
            ctx.accounts.hlp_ylp_account.to_account_info(),
            receipt.ylp_amount,
            &[&generate_market_seeds!(ctx.accounts.market)[..]],
        )?;

        let target_balance_before = ctx.accounts.owner_target_account.amount;
        let (target_reserve_vault, target_mint, target_decimals) = match target_asset {
            MarketAsset::Base => (
                ctx.accounts.base_reserve_vault.to_account_info(),
                ctx.accounts.base_mint.to_account_info(),
                ctx.accounts.base_mint.decimals,
            ),
            MarketAsset::Quote => (
                ctx.accounts.quote_reserve_vault.to_account_info(),
                ctx.accounts.quote_mint.to_account_info(),
                ctx.accounts.quote_mint.decimals,
            ),
        };
        let target_token_program = token_program_for_mint(
            match target_asset {
                MarketAsset::Base => &ctx.accounts.base_mint,
                MarketAsset::Quote => &ctx.accounts.quote_mint,
            },
            &ctx.accounts.token_program,
            &ctx.accounts.token_2022_program,
        )?;
        transfer_from_vault_to_user(
            ctx.accounts.market.to_account_info(),
            target_reserve_vault,
            ctx.accounts.owner_target_account.to_account_info(),
            target_mint,
            target_token_program,
            receipt.target_amount_out,
            target_decimals,
            &[&generate_market_seeds!(ctx.accounts.market)[..]],
        )?;
        ctx.accounts.owner_target_account.reload()?;
        let target_credit =
            token_account_credit(target_balance_before, &ctx.accounts.owner_target_account)?;
        require_gte!(
            target_credit,
            args.min_target_amount_out,
            ErrorCode::SlippageExceeded
        );

        emit_cpi!(HlpClosed {
            market: market_key,
            owner: owner_key,
            asset_mint: target_mint_key,
            hlp_amount: receipt.hlp_amount,
            ylp_amount: receipt.ylp_amount,
            target_amount_out: receipt.target_amount_out,
            debt_repaid: receipt.debt_repaid,
            interest_paid: receipt.interest_paid,
            hlp_supply: receipt.hlp_supply,
            metadata: MarketEventMetadata::new(owner_key, market_key)?,
        });
        emit_cpi!(MarketHealthUpdated {
            market: market_key,
            recognized_base_collateral_for_quote_debt: ctx
                .accounts
                .market
                .health
                .recognized_base_collateral_for_quote_debt,
            recognized_quote_collateral_for_base_debt: ctx
                .accounts
                .market
                .health
                .recognized_quote_collateral_for_base_debt,
            effective_base_debt_nad: ctx.accounts.market.health.effective_base_debt_nad,
            effective_quote_debt_nad: ctx.accounts.market.health.effective_quote_debt_nad,
            base_debt_health_bps: ctx.accounts.market.health.base_debt_health_bps,
            quote_debt_health_bps: ctx.accounts.market.health.quote_debt_health_bps,
            metadata: MarketEventMetadata::new(owner_key, market_key)?,
        });

        Ok(())
    }
}

fn initialize_or_validate_hlp_yield_account(
    yield_account: &mut Account<YieldAccount>,
    owner: Pubkey,
    market: Pubkey,
    asset_mint: Pubkey,
    bump: u8,
) -> Result<()> {
    if yield_account.owner == Pubkey::default() {
        yield_account.initialize(owner, market, asset_mint, YieldTokenKind::Hlp, owner, bump);
    }
    yield_account.assert_account(owner, market, asset_mint, YieldTokenKind::Hlp)
}

fn hlp_yield_growth_indexes(market: &Market, market_asset: MarketAsset) -> (u128, u128) {
    match market_asset {
        MarketAsset::Base => market
            .base_hlp_vault
            .yield_growth_indexes(MarketAsset::Base),
        MarketAsset::Quote => market
            .quote_hlp_vault
            .yield_growth_indexes(MarketAsset::Quote),
    }
}
