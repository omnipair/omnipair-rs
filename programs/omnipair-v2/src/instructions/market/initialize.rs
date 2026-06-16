use anchor_lang::prelude::*;
use anchor_spl::{
    token::Token,
    token_interface::{Mint, Token2022},
};

use crate::{
    constants::*,
    errors::ErrorCode,
    events::{MarketCreated, MarketEventMetadata},
    shared::account::get_size_with_discriminator,
    shared::token::create_token_account,
    state::{Market, MarketConfig, MarketSide},
    tokens::{claim_token::validate_claim_token_mint, hedge_token::validate_hedge_token_mint},
};

use crate::instructions::common::{require_supported_asset_mint, token_program_for_mint};

#[derive(AnchorSerialize, AnchorDeserialize, Clone)]
pub struct InitializeMarketArgs {
    pub operator: Pubkey,
    pub manager: Pubkey,
    pub config: MarketConfig,
    pub params_hash: [u8; 32],
}

#[event_cpi]
#[derive(Accounts)]
#[instruction(args: InitializeMarketArgs)]
pub struct InitializeMarket<'info> {
    #[account(mut)]
    pub payer: Signer<'info>,

    pub base_mint: Box<InterfaceAccount<'info, Mint>>,
    pub quote_mint: Box<InterfaceAccount<'info, Mint>>,

    #[account(
        init,
        payer = payer,
        space = get_size_with_discriminator::<Market>(),
        seeds = [
            MARKET_V2_SEED_PREFIX,
            base_mint.key().as_ref(),
            quote_mint.key().as_ref(),
            args.params_hash.as_ref(),
        ],
        bump
    )]
    pub market: Box<Account<'info, Market>>,

    pub base_claim_token_mint: Box<InterfaceAccount<'info, Mint>>,
    pub quote_claim_token_mint: Box<InterfaceAccount<'info, Mint>>,
    pub base_hedge_token_mint: Box<InterfaceAccount<'info, Mint>>,
    pub quote_hedge_token_mint: Box<InterfaceAccount<'info, Mint>>,
    /// CHECK: Claim escrow PDA for h-omLP base wrappers.
    #[account(
        mut,
        seeds = [
            HEDGE_VAULT_SEED_PREFIX,
            market.key().as_ref(),
            base_claim_token_mint.key().as_ref(),
        ],
        bump
    )]
    pub base_hedge_vault: UncheckedAccount<'info>,
    /// CHECK: Claim escrow PDA for h-omLP quote wrappers.
    #[account(
        mut,
        seeds = [
            HEDGE_VAULT_SEED_PREFIX,
            market.key().as_ref(),
            quote_claim_token_mint.key().as_ref(),
        ],
        bump
    )]
    pub quote_hedge_vault: UncheckedAccount<'info>,
    /// CHECK: Reserve vault PDA for the base asset.
    #[account(
        mut,
        seeds = [
            MARKET_RESERVE_VAULT_SEED_PREFIX,
            market.key().as_ref(),
            base_mint.key().as_ref(),
        ],
        bump
    )]
    pub base_reserve_vault: UncheckedAccount<'info>,
    /// CHECK: Reserve vault PDA for the quote asset.
    #[account(
        mut,
        seeds = [
            MARKET_RESERVE_VAULT_SEED_PREFIX,
            market.key().as_ref(),
            quote_mint.key().as_ref(),
        ],
        bump
    )]
    pub quote_reserve_vault: UncheckedAccount<'info>,
    /// CHECK: Collateral vault PDA for the base asset.
    #[account(
        mut,
        seeds = [
            MARKET_COLLATERAL_VAULT_SEED_PREFIX,
            market.key().as_ref(),
            base_mint.key().as_ref(),
        ],
        bump
    )]
    pub base_collateral_vault: UncheckedAccount<'info>,
    /// CHECK: Collateral vault PDA for the quote asset.
    #[account(
        mut,
        seeds = [
            MARKET_COLLATERAL_VAULT_SEED_PREFIX,
            market.key().as_ref(),
            quote_mint.key().as_ref(),
        ],
        bump
    )]
    pub quote_collateral_vault: UncheckedAccount<'info>,
    /// CHECK: Junior insurance reserve vault PDA for the base asset.
    #[account(
        mut,
        seeds = [
            INSURANCE_RESERVE_SEED_PREFIX,
            market.key().as_ref(),
            base_mint.key().as_ref(),
        ],
        bump
    )]
    pub base_insurance_vault: UncheckedAccount<'info>,
    /// CHECK: Junior insurance reserve vault PDA for the quote asset.
    #[account(
        mut,
        seeds = [
            INSURANCE_RESERVE_SEED_PREFIX,
            market.key().as_ref(),
            quote_mint.key().as_ref(),
        ],
        bump
    )]
    pub quote_insurance_vault: UncheckedAccount<'info>,
    /// CHECK: Non-compounding fee vault PDA for the base asset.
    #[account(
        mut,
        seeds = [
            MARKET_FEE_VAULT_SEED_PREFIX,
            market.key().as_ref(),
            base_mint.key().as_ref(),
        ],
        bump
    )]
    pub base_fee_vault: UncheckedAccount<'info>,
    /// CHECK: Non-compounding fee vault PDA for the quote asset.
    #[account(
        mut,
        seeds = [
            MARKET_FEE_VAULT_SEED_PREFIX,
            market.key().as_ref(),
            quote_mint.key().as_ref(),
        ],
        bump
    )]
    pub quote_fee_vault: UncheckedAccount<'info>,
    /// CHECK: Staked claim escrow PDA for the base claim token.
    #[account(
        mut,
        seeds = [
            MARKET_STAKE_VAULT_SEED_PREFIX,
            market.key().as_ref(),
            base_claim_token_mint.key().as_ref(),
        ],
        bump
    )]
    pub base_stake_vault: UncheckedAccount<'info>,
    /// CHECK: Staked claim escrow PDA for the quote claim token.
    #[account(
        mut,
        seeds = [
            MARKET_STAKE_VAULT_SEED_PREFIX,
            market.key().as_ref(),
            quote_claim_token_mint.key().as_ref(),
        ],
        bump
    )]
    pub quote_stake_vault: UncheckedAccount<'info>,

    pub system_program: Program<'info, System>,
    pub token_program: Program<'info, Token>,
    pub token_2022_program: Program<'info, Token2022>,
}

impl<'info> InitializeMarket<'info> {
    pub fn validate(&self, args: &InitializeMarketArgs) -> Result<()> {
        require_keys_neq!(
            self.base_mint.key(),
            self.quote_mint.key(),
            ErrorCode::InvalidMint
        );
        require_keys_neq!(
            args.operator,
            Pubkey::default(),
            ErrorCode::InvalidMarketConfig
        );
        require_keys_neq!(
            args.manager,
            Pubkey::default(),
            ErrorCode::InvalidMarketConfig
        );
        require_supported_asset_mint(&self.base_mint)?;
        require_supported_asset_mint(&self.quote_mint)?;
        let market = self.market.key();
        validate_claim_token_mint(&self.base_claim_token_mint, market, self.base_mint.decimals)?;
        validate_claim_token_mint(
            &self.quote_claim_token_mint,
            market,
            self.quote_mint.decimals,
        )?;
        validate_hedge_token_mint(&self.base_hedge_token_mint, market, self.base_mint.decimals)?;
        validate_hedge_token_mint(
            &self.quote_hedge_token_mint,
            market,
            self.quote_mint.decimals,
        )?;
        args.config.validate()
    }

    pub fn handle_initialize(ctx: Context<Self>, args: InitializeMarketArgs) -> Result<()> {
        let current_slot = Clock::get()?.slot;
        let market_key = ctx.accounts.market.key();

        Self::create_vault_accounts(&ctx)?;

        let market = &mut ctx.accounts.market;
        market.version = MARKET_VERSION;
        market.base_mint = ctx.accounts.base_mint.key();
        market.quote_mint = ctx.accounts.quote_mint.key();
        market.operator = args.operator;
        market.manager = args.manager;
        market.base_side = MarketSide {
            asset_mint: ctx.accounts.base_mint.key(),
            asset_decimals: ctx.accounts.base_mint.decimals,
            claim_token_mint: ctx.accounts.base_claim_token_mint.key(),
            hedge_token_mint: ctx.accounts.base_hedge_token_mint.key(),
            hedge_vault: ctx.accounts.base_hedge_vault.key(),
            reserve_vault: ctx.accounts.base_reserve_vault.key(),
            collateral_vault: ctx.accounts.base_collateral_vault.key(),
            fee_vault: ctx.accounts.base_fee_vault.key(),
            stake_vault: ctx.accounts.base_stake_vault.key(),
            buffer_ledger: crate::state::BufferLedger {
                buffer_ratio_bps: args.config.buffer_ratio_bps,
                ..crate::state::BufferLedger::default()
            },
            ..MarketSide::default()
        };
        market.quote_side = MarketSide {
            asset_mint: ctx.accounts.quote_mint.key(),
            asset_decimals: ctx.accounts.quote_mint.decimals,
            claim_token_mint: ctx.accounts.quote_claim_token_mint.key(),
            hedge_token_mint: ctx.accounts.quote_hedge_token_mint.key(),
            hedge_vault: ctx.accounts.quote_hedge_vault.key(),
            reserve_vault: ctx.accounts.quote_reserve_vault.key(),
            collateral_vault: ctx.accounts.quote_collateral_vault.key(),
            fee_vault: ctx.accounts.quote_fee_vault.key(),
            stake_vault: ctx.accounts.quote_stake_vault.key(),
            buffer_ledger: crate::state::BufferLedger {
                buffer_ratio_bps: args.config.buffer_ratio_bps,
                ..crate::state::BufferLedger::default()
            },
            ..MarketSide::default()
        };
        market.insurance_reserve.base_vault = ctx.accounts.base_insurance_vault.key();
        market.insurance_reserve.quote_vault = ctx.accounts.quote_insurance_vault.key();
        market.config = args.config;
        market.debt_book = crate::state::DebtBook {
            base_borrow_index_nad: NAD as u128,
            quote_borrow_index_nad: NAD as u128,
            ..crate::state::DebtBook::default()
        };
        market.risk_book = crate::state::RiskBook {
            last_snapshot_slot: current_slot,
            ..crate::state::RiskBook::default()
        };
        market.health = crate::state::MarketHealth::default();
        market.recognition_ledger = crate::state::RecognitionLedger {
            last_recognition_slot: current_slot,
            ..crate::state::RecognitionLedger::default()
        };
        market.params_hash = args.params_hash;
        market.last_update_slot = current_slot;
        market.reduce_only = false;
        market.bump = ctx.bumps.market;

        emit_cpi!(MarketCreated {
            market: market_key,
            base_mint: ctx.accounts.base_mint.key(),
            quote_mint: ctx.accounts.quote_mint.key(),
            base_claim_token_mint: ctx.accounts.base_claim_token_mint.key(),
            quote_claim_token_mint: ctx.accounts.quote_claim_token_mint.key(),
            base_stake_vault: ctx.accounts.base_stake_vault.key(),
            quote_stake_vault: ctx.accounts.quote_stake_vault.key(),
            base_collateral_vault: ctx.accounts.base_collateral_vault.key(),
            quote_collateral_vault: ctx.accounts.quote_collateral_vault.key(),
            base_insurance_vault: ctx.accounts.base_insurance_vault.key(),
            quote_insurance_vault: ctx.accounts.quote_insurance_vault.key(),
            base_hedge_token_mint: ctx.accounts.base_hedge_token_mint.key(),
            quote_hedge_token_mint: ctx.accounts.quote_hedge_token_mint.key(),
            base_hedge_vault: ctx.accounts.base_hedge_vault.key(),
            quote_hedge_vault: ctx.accounts.quote_hedge_vault.key(),
            operator: args.operator,
            manager: args.manager,
            buffer_ratio_bps: args.config.buffer_ratio_bps,
            swap_fee_bps: args.config.swap_fee_bps,
            protocol_fee_bps: args.config.protocol_fee_bps,
            params_hash: args.params_hash,
            version: MARKET_VERSION,
            metadata: MarketEventMetadata::new(ctx.accounts.payer.key(), market_key)?,
        });

        Ok(())
    }

    fn create_vault_accounts(ctx: &Context<Self>) -> Result<()> {
        let base_token_program = token_program_for_mint(
            &ctx.accounts.base_mint,
            &ctx.accounts.token_program,
            &ctx.accounts.token_2022_program,
        )?;
        let quote_token_program = token_program_for_mint(
            &ctx.accounts.quote_mint,
            &ctx.accounts.token_program,
            &ctx.accounts.token_2022_program,
        )?;
        let base_claim_token_program = token_program_for_mint(
            &ctx.accounts.base_claim_token_mint,
            &ctx.accounts.token_program,
            &ctx.accounts.token_2022_program,
        )?;
        let quote_claim_token_program = token_program_for_mint(
            &ctx.accounts.quote_claim_token_mint,
            &ctx.accounts.token_program,
            &ctx.accounts.token_2022_program,
        )?;

        create_vault_token_account(
            &ctx.accounts.market,
            &ctx.accounts.payer,
            &ctx.accounts.base_reserve_vault,
            &ctx.accounts.base_mint,
            &ctx.accounts.system_program,
            &base_token_program,
            MARKET_RESERVE_VAULT_SEED_PREFIX,
            ctx.bumps.base_reserve_vault,
        )?;
        create_vault_token_account(
            &ctx.accounts.market,
            &ctx.accounts.payer,
            &ctx.accounts.quote_reserve_vault,
            &ctx.accounts.quote_mint,
            &ctx.accounts.system_program,
            &quote_token_program,
            MARKET_RESERVE_VAULT_SEED_PREFIX,
            ctx.bumps.quote_reserve_vault,
        )?;
        create_vault_token_account(
            &ctx.accounts.market,
            &ctx.accounts.payer,
            &ctx.accounts.base_collateral_vault,
            &ctx.accounts.base_mint,
            &ctx.accounts.system_program,
            &base_token_program,
            MARKET_COLLATERAL_VAULT_SEED_PREFIX,
            ctx.bumps.base_collateral_vault,
        )?;
        create_vault_token_account(
            &ctx.accounts.market,
            &ctx.accounts.payer,
            &ctx.accounts.quote_collateral_vault,
            &ctx.accounts.quote_mint,
            &ctx.accounts.system_program,
            &quote_token_program,
            MARKET_COLLATERAL_VAULT_SEED_PREFIX,
            ctx.bumps.quote_collateral_vault,
        )?;
        create_vault_token_account(
            &ctx.accounts.market,
            &ctx.accounts.payer,
            &ctx.accounts.base_insurance_vault,
            &ctx.accounts.base_mint,
            &ctx.accounts.system_program,
            &base_token_program,
            INSURANCE_RESERVE_SEED_PREFIX,
            ctx.bumps.base_insurance_vault,
        )?;
        create_vault_token_account(
            &ctx.accounts.market,
            &ctx.accounts.payer,
            &ctx.accounts.quote_insurance_vault,
            &ctx.accounts.quote_mint,
            &ctx.accounts.system_program,
            &quote_token_program,
            INSURANCE_RESERVE_SEED_PREFIX,
            ctx.bumps.quote_insurance_vault,
        )?;
        create_vault_token_account(
            &ctx.accounts.market,
            &ctx.accounts.payer,
            &ctx.accounts.base_fee_vault,
            &ctx.accounts.base_mint,
            &ctx.accounts.system_program,
            &base_token_program,
            MARKET_FEE_VAULT_SEED_PREFIX,
            ctx.bumps.base_fee_vault,
        )?;
        create_vault_token_account(
            &ctx.accounts.market,
            &ctx.accounts.payer,
            &ctx.accounts.quote_fee_vault,
            &ctx.accounts.quote_mint,
            &ctx.accounts.system_program,
            &quote_token_program,
            MARKET_FEE_VAULT_SEED_PREFIX,
            ctx.bumps.quote_fee_vault,
        )?;
        create_vault_token_account(
            &ctx.accounts.market,
            &ctx.accounts.payer,
            &ctx.accounts.base_hedge_vault,
            &ctx.accounts.base_claim_token_mint,
            &ctx.accounts.system_program,
            &base_claim_token_program,
            HEDGE_VAULT_SEED_PREFIX,
            ctx.bumps.base_hedge_vault,
        )?;
        create_vault_token_account(
            &ctx.accounts.market,
            &ctx.accounts.payer,
            &ctx.accounts.quote_hedge_vault,
            &ctx.accounts.quote_claim_token_mint,
            &ctx.accounts.system_program,
            &quote_claim_token_program,
            HEDGE_VAULT_SEED_PREFIX,
            ctx.bumps.quote_hedge_vault,
        )?;
        create_vault_token_account(
            &ctx.accounts.market,
            &ctx.accounts.payer,
            &ctx.accounts.base_stake_vault,
            &ctx.accounts.base_claim_token_mint,
            &ctx.accounts.system_program,
            &base_claim_token_program,
            MARKET_STAKE_VAULT_SEED_PREFIX,
            ctx.bumps.base_stake_vault,
        )?;
        create_vault_token_account(
            &ctx.accounts.market,
            &ctx.accounts.payer,
            &ctx.accounts.quote_stake_vault,
            &ctx.accounts.quote_claim_token_mint,
            &ctx.accounts.system_program,
            &quote_claim_token_program,
            MARKET_STAKE_VAULT_SEED_PREFIX,
            ctx.bumps.quote_stake_vault,
        )
    }
}

fn create_vault_token_account<'info>(
    market: &Account<'info, Market>,
    payer: &Signer<'info>,
    vault: &UncheckedAccount<'info>,
    mint: &InterfaceAccount<'info, Mint>,
    system_program: &Program<'info, System>,
    token_program: &AccountInfo<'info>,
    seed_prefix: &[u8],
    bump: u8,
) -> Result<()> {
    let market_key = market.key();
    let mint_key = mint.key();
    let bump_seed = [bump];
    let market_info = market.to_account_info();
    let payer_info = payer.to_account_info();
    let vault_info = vault.to_account_info();
    let mint_info = mint.to_account_info();
    let system_program_info = system_program.to_account_info();

    create_token_account(
        &market_info,
        &payer_info,
        &vault_info,
        &mint_info,
        &system_program_info,
        token_program,
        &[
            seed_prefix,
            market_key.as_ref(),
            mint_key.as_ref(),
            &bump_seed,
        ],
    )
}
