use anchor_lang::{prelude::*, solana_program::program_option::COption};
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
    v2::state::{Market, MarketConfig, MarketSide},
};

use crate::v2::instructions::common::{
    require_fee_free_claim_mint, require_supported_asset_mint, token_program_for_mint,
};

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

    pub asset0_mint: Box<InterfaceAccount<'info, Mint>>,
    pub asset1_mint: Box<InterfaceAccount<'info, Mint>>,

    #[account(
        init,
        payer = payer,
        space = get_size_with_discriminator::<Market>(),
        seeds = [
            MARKET_V2_SEED_PREFIX,
            asset0_mint.key().as_ref(),
            asset1_mint.key().as_ref(),
            args.params_hash.as_ref(),
        ],
        bump
    )]
    pub market: Box<Account<'info, Market>>,

    pub claim0_mint: Box<InterfaceAccount<'info, Mint>>,
    pub claim1_mint: Box<InterfaceAccount<'info, Mint>>,
    pub hedge0_mint: Box<InterfaceAccount<'info, Mint>>,
    pub hedge1_mint: Box<InterfaceAccount<'info, Mint>>,
    /// CHECK: Canonical base claim escrow PDA for h-omLP asset0 wrappers.
    #[account(
        mut,
        seeds = [
            HEDGE_VAULT_SEED_PREFIX,
            market.key().as_ref(),
            claim0_mint.key().as_ref(),
        ],
        bump
    )]
    pub hedge0_vault: UncheckedAccount<'info>,
    /// CHECK: Canonical base claim escrow PDA for h-omLP asset1 wrappers.
    #[account(
        mut,
        seeds = [
            HEDGE_VAULT_SEED_PREFIX,
            market.key().as_ref(),
            claim1_mint.key().as_ref(),
        ],
        bump
    )]
    pub hedge1_vault: UncheckedAccount<'info>,
    /// CHECK: Canonical reserve vault PDA for asset0.
    #[account(
        mut,
        seeds = [
            MARKET_RESERVE_VAULT_SEED_PREFIX,
            market.key().as_ref(),
            asset0_mint.key().as_ref(),
        ],
        bump
    )]
    pub reserve0_vault: UncheckedAccount<'info>,
    /// CHECK: Canonical reserve vault PDA for asset1.
    #[account(
        mut,
        seeds = [
            MARKET_RESERVE_VAULT_SEED_PREFIX,
            market.key().as_ref(),
            asset1_mint.key().as_ref(),
        ],
        bump
    )]
    pub reserve1_vault: UncheckedAccount<'info>,
    /// CHECK: Canonical collateral vault PDA for asset0.
    #[account(
        mut,
        seeds = [
            MARKET_COLLATERAL_VAULT_SEED_PREFIX,
            market.key().as_ref(),
            asset0_mint.key().as_ref(),
        ],
        bump
    )]
    pub collateral0_vault: UncheckedAccount<'info>,
    /// CHECK: Canonical collateral vault PDA for asset1.
    #[account(
        mut,
        seeds = [
            MARKET_COLLATERAL_VAULT_SEED_PREFIX,
            market.key().as_ref(),
            asset1_mint.key().as_ref(),
        ],
        bump
    )]
    pub collateral1_vault: UncheckedAccount<'info>,
    /// CHECK: Canonical junior insurance reserve vault PDA for asset0.
    #[account(
        mut,
        seeds = [
            INSURANCE_RESERVE_SEED_PREFIX,
            market.key().as_ref(),
            asset0_mint.key().as_ref(),
        ],
        bump
    )]
    pub insurance0_vault: UncheckedAccount<'info>,
    /// CHECK: Canonical junior insurance reserve vault PDA for asset1.
    #[account(
        mut,
        seeds = [
            INSURANCE_RESERVE_SEED_PREFIX,
            market.key().as_ref(),
            asset1_mint.key().as_ref(),
        ],
        bump
    )]
    pub insurance1_vault: UncheckedAccount<'info>,
    /// CHECK: Canonical non-compounding fee vault PDA for asset0.
    #[account(
        mut,
        seeds = [
            MARKET_FEE_VAULT_SEED_PREFIX,
            market.key().as_ref(),
            asset0_mint.key().as_ref(),
        ],
        bump
    )]
    pub fee0_vault: UncheckedAccount<'info>,
    /// CHECK: Canonical non-compounding fee vault PDA for asset1.
    #[account(
        mut,
        seeds = [
            MARKET_FEE_VAULT_SEED_PREFIX,
            market.key().as_ref(),
            asset1_mint.key().as_ref(),
        ],
        bump
    )]
    pub fee1_vault: UncheckedAccount<'info>,
    /// CHECK: Canonical staked claim escrow PDA for asset0.
    #[account(
        mut,
        seeds = [
            MARKET_STAKE_VAULT_SEED_PREFIX,
            market.key().as_ref(),
            claim0_mint.key().as_ref(),
        ],
        bump
    )]
    pub claim0_stake_vault: UncheckedAccount<'info>,
    /// CHECK: Canonical staked claim escrow PDA for asset1.
    #[account(
        mut,
        seeds = [
            MARKET_STAKE_VAULT_SEED_PREFIX,
            market.key().as_ref(),
            claim1_mint.key().as_ref(),
        ],
        bump
    )]
    pub claim1_stake_vault: UncheckedAccount<'info>,

    pub system_program: Program<'info, System>,
    pub token_program: Program<'info, Token>,
    pub token_2022_program: Program<'info, Token2022>,
}

impl<'info> InitializeMarket<'info> {
    pub fn validate(&self, args: &InitializeMarketArgs) -> Result<()> {
        require_gt!(
            self.asset1_mint.key(),
            self.asset0_mint.key(),
            ErrorCode::InvalidTokenOrder
        );
        require_keys_neq!(
            args.operator,
            Pubkey::default(),
            ErrorCode::InvalidMarketConfig
        );
        require_supported_asset_mint(&self.asset0_mint)?;
        require_supported_asset_mint(&self.asset1_mint)?;
        self.validate_claim_mint(&self.claim0_mint, self.asset0_mint.decimals)?;
        self.validate_claim_mint(&self.claim1_mint, self.asset1_mint.decimals)?;
        self.validate_claim_mint(&self.hedge0_mint, self.asset0_mint.decimals)?;
        self.validate_claim_mint(&self.hedge1_mint, self.asset1_mint.decimals)?;
        args.config.validate()
    }

    fn validate_claim_mint(&self, mint: &InterfaceAccount<Mint>, asset_decimals: u8) -> Result<()> {
        require_fee_free_claim_mint(mint)?;
        require_eq!(mint.decimals, asset_decimals, ErrorCode::InvalidClaimMint);
        require!(
            mint.mint_authority == COption::Some(self.market.key()),
            ErrorCode::InvalidClaimMint
        );
        Ok(())
    }

    pub fn handle_initialize(ctx: Context<Self>, args: InitializeMarketArgs) -> Result<()> {
        let current_slot = Clock::get()?.slot;
        let market_key = ctx.accounts.market.key();

        Self::create_vault_accounts(&ctx)?;

        let market = &mut ctx.accounts.market;
        market.version = MARKET_VERSION;
        market.asset0_mint = ctx.accounts.asset0_mint.key();
        market.asset1_mint = ctx.accounts.asset1_mint.key();
        market.operator = args.operator;
        market.manager = args.manager;
        market.side0 = MarketSide {
            asset_mint: ctx.accounts.asset0_mint.key(),
            asset_decimals: ctx.accounts.asset0_mint.decimals,
            claim_mint: ctx.accounts.claim0_mint.key(),
            hedge_mint: ctx.accounts.hedge0_mint.key(),
            hedge_vault: ctx.accounts.hedge0_vault.key(),
            reserve_vault: ctx.accounts.reserve0_vault.key(),
            collateral_vault: ctx.accounts.collateral0_vault.key(),
            fee_vault: ctx.accounts.fee0_vault.key(),
            stake_vault: ctx.accounts.claim0_stake_vault.key(),
            buffer_book: crate::v2::state::BufferBook {
                buffer_ratio_bps: args.config.buffer_ratio_bps,
                ..crate::v2::state::BufferBook::default()
            },
            ..MarketSide::default()
        };
        market.side1 = MarketSide {
            asset_mint: ctx.accounts.asset1_mint.key(),
            asset_decimals: ctx.accounts.asset1_mint.decimals,
            claim_mint: ctx.accounts.claim1_mint.key(),
            hedge_mint: ctx.accounts.hedge1_mint.key(),
            hedge_vault: ctx.accounts.hedge1_vault.key(),
            reserve_vault: ctx.accounts.reserve1_vault.key(),
            collateral_vault: ctx.accounts.collateral1_vault.key(),
            fee_vault: ctx.accounts.fee1_vault.key(),
            stake_vault: ctx.accounts.claim1_stake_vault.key(),
            buffer_book: crate::v2::state::BufferBook {
                buffer_ratio_bps: args.config.buffer_ratio_bps,
                ..crate::v2::state::BufferBook::default()
            },
            ..MarketSide::default()
        };
        market.insurance_reserve.vault0 = ctx.accounts.insurance0_vault.key();
        market.insurance_reserve.vault1 = ctx.accounts.insurance1_vault.key();
        market.config = args.config;
        market.debt_book = crate::v2::state::DebtBook {
            borrow_index0_nad: NAD as u128,
            borrow_index1_nad: NAD as u128,
            ..crate::v2::state::DebtBook::default()
        };
        market.risk_book = crate::v2::state::RiskBook {
            last_snapshot_slot: current_slot,
            ..crate::v2::state::RiskBook::default()
        };
        market.health = crate::v2::state::MarketHealth::default();
        market.recognition_ledger = crate::v2::state::RecognitionLedger {
            last_recognition_slot: current_slot,
            ..crate::v2::state::RecognitionLedger::default()
        };
        market.params_hash = args.params_hash;
        market.last_update_slot = current_slot;
        market.reduce_only = false;
        market.bump = ctx.bumps.market;

        emit_cpi!(MarketCreated {
            market: market_key,
            asset0_mint: ctx.accounts.asset0_mint.key(),
            asset1_mint: ctx.accounts.asset1_mint.key(),
            claim0_mint: ctx.accounts.claim0_mint.key(),
            claim1_mint: ctx.accounts.claim1_mint.key(),
            claim0_stake_vault: ctx.accounts.claim0_stake_vault.key(),
            claim1_stake_vault: ctx.accounts.claim1_stake_vault.key(),
            collateral0_vault: ctx.accounts.collateral0_vault.key(),
            collateral1_vault: ctx.accounts.collateral1_vault.key(),
            insurance0_vault: ctx.accounts.insurance0_vault.key(),
            insurance1_vault: ctx.accounts.insurance1_vault.key(),
            hedge0_mint: ctx.accounts.hedge0_mint.key(),
            hedge1_mint: ctx.accounts.hedge1_mint.key(),
            hedge0_vault: ctx.accounts.hedge0_vault.key(),
            hedge1_vault: ctx.accounts.hedge1_vault.key(),
            operator: args.operator,
            manager: args.manager,
            buffer_ratio_bps: args.config.buffer_ratio_bps,
            swap_fee_bps: args.config.swap_fee_bps,
            params_hash: args.params_hash,
            version: MARKET_VERSION,
            metadata: MarketEventMetadata::new(ctx.accounts.payer.key(), market_key),
        });

        Ok(())
    }

    fn create_vault_accounts(ctx: &Context<Self>) -> Result<()> {
        let asset0_token_program = token_program_for_mint(
            &ctx.accounts.asset0_mint,
            &ctx.accounts.token_program,
            &ctx.accounts.token_2022_program,
        )?;
        let asset1_token_program = token_program_for_mint(
            &ctx.accounts.asset1_mint,
            &ctx.accounts.token_program,
            &ctx.accounts.token_2022_program,
        )?;
        let claim0_token_program = token_program_for_mint(
            &ctx.accounts.claim0_mint,
            &ctx.accounts.token_program,
            &ctx.accounts.token_2022_program,
        )?;
        let claim1_token_program = token_program_for_mint(
            &ctx.accounts.claim1_mint,
            &ctx.accounts.token_program,
            &ctx.accounts.token_2022_program,
        )?;

        create_vault_token_account(
            &ctx.accounts.market,
            &ctx.accounts.payer,
            &ctx.accounts.reserve0_vault,
            &ctx.accounts.asset0_mint,
            &ctx.accounts.system_program,
            &asset0_token_program,
            MARKET_RESERVE_VAULT_SEED_PREFIX,
            ctx.bumps.reserve0_vault,
        )?;
        create_vault_token_account(
            &ctx.accounts.market,
            &ctx.accounts.payer,
            &ctx.accounts.reserve1_vault,
            &ctx.accounts.asset1_mint,
            &ctx.accounts.system_program,
            &asset1_token_program,
            MARKET_RESERVE_VAULT_SEED_PREFIX,
            ctx.bumps.reserve1_vault,
        )?;
        create_vault_token_account(
            &ctx.accounts.market,
            &ctx.accounts.payer,
            &ctx.accounts.collateral0_vault,
            &ctx.accounts.asset0_mint,
            &ctx.accounts.system_program,
            &asset0_token_program,
            MARKET_COLLATERAL_VAULT_SEED_PREFIX,
            ctx.bumps.collateral0_vault,
        )?;
        create_vault_token_account(
            &ctx.accounts.market,
            &ctx.accounts.payer,
            &ctx.accounts.collateral1_vault,
            &ctx.accounts.asset1_mint,
            &ctx.accounts.system_program,
            &asset1_token_program,
            MARKET_COLLATERAL_VAULT_SEED_PREFIX,
            ctx.bumps.collateral1_vault,
        )?;
        create_vault_token_account(
            &ctx.accounts.market,
            &ctx.accounts.payer,
            &ctx.accounts.insurance0_vault,
            &ctx.accounts.asset0_mint,
            &ctx.accounts.system_program,
            &asset0_token_program,
            INSURANCE_RESERVE_SEED_PREFIX,
            ctx.bumps.insurance0_vault,
        )?;
        create_vault_token_account(
            &ctx.accounts.market,
            &ctx.accounts.payer,
            &ctx.accounts.insurance1_vault,
            &ctx.accounts.asset1_mint,
            &ctx.accounts.system_program,
            &asset1_token_program,
            INSURANCE_RESERVE_SEED_PREFIX,
            ctx.bumps.insurance1_vault,
        )?;
        create_vault_token_account(
            &ctx.accounts.market,
            &ctx.accounts.payer,
            &ctx.accounts.fee0_vault,
            &ctx.accounts.asset0_mint,
            &ctx.accounts.system_program,
            &asset0_token_program,
            MARKET_FEE_VAULT_SEED_PREFIX,
            ctx.bumps.fee0_vault,
        )?;
        create_vault_token_account(
            &ctx.accounts.market,
            &ctx.accounts.payer,
            &ctx.accounts.fee1_vault,
            &ctx.accounts.asset1_mint,
            &ctx.accounts.system_program,
            &asset1_token_program,
            MARKET_FEE_VAULT_SEED_PREFIX,
            ctx.bumps.fee1_vault,
        )?;
        create_vault_token_account(
            &ctx.accounts.market,
            &ctx.accounts.payer,
            &ctx.accounts.hedge0_vault,
            &ctx.accounts.claim0_mint,
            &ctx.accounts.system_program,
            &claim0_token_program,
            HEDGE_VAULT_SEED_PREFIX,
            ctx.bumps.hedge0_vault,
        )?;
        create_vault_token_account(
            &ctx.accounts.market,
            &ctx.accounts.payer,
            &ctx.accounts.hedge1_vault,
            &ctx.accounts.claim1_mint,
            &ctx.accounts.system_program,
            &claim1_token_program,
            HEDGE_VAULT_SEED_PREFIX,
            ctx.bumps.hedge1_vault,
        )?;
        create_vault_token_account(
            &ctx.accounts.market,
            &ctx.accounts.payer,
            &ctx.accounts.claim0_stake_vault,
            &ctx.accounts.claim0_mint,
            &ctx.accounts.system_program,
            &claim0_token_program,
            MARKET_STAKE_VAULT_SEED_PREFIX,
            ctx.bumps.claim0_stake_vault,
        )?;
        create_vault_token_account(
            &ctx.accounts.market,
            &ctx.accounts.payer,
            &ctx.accounts.claim1_stake_vault,
            &ctx.accounts.claim1_mint,
            &ctx.accounts.system_program,
            &claim1_token_program,
            MARKET_STAKE_VAULT_SEED_PREFIX,
            ctx.bumps.claim1_stake_vault,
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
