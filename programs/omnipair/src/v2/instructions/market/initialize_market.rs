use anchor_lang::prelude::*;
use anchor_spl::token_interface::Mint;

use crate::{
    constants::*,
    errors::ErrorCode,
    events::{MarketCreated, MarketEventMetadata},
    shared::account::get_size_with_discriminator,
    v2::state::{Market, MarketConfig, MarketSide},
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
            MARKET_SEED_PREFIX,
            asset0_mint.key().as_ref(),
            asset1_mint.key().as_ref(),
            args.params_hash.as_ref(),
        ],
        bump
    )]
    pub market: Box<Account<'info, Market>>,

    /// CHECK: Stored as the protected claim mint for asset0; initialized in a later token-layer instruction.
    pub claim0_mint: UncheckedAccount<'info>,
    /// CHECK: Stored as the protected claim mint for asset1; initialized in a later token-layer instruction.
    pub claim1_mint: UncheckedAccount<'info>,
    /// CHECK: Stored as the hedged wrapper mint for asset0; initialized in a later token-layer instruction.
    pub hedge0_mint: UncheckedAccount<'info>,
    /// CHECK: Stored as the hedged wrapper mint for asset1; initialized in a later token-layer instruction.
    pub hedge1_mint: UncheckedAccount<'info>,
    /// CHECK: Stored as the base claim escrow for h-omLP asset0 wrappers; validation is added in the hedge layer.
    pub hedge0_vault: UncheckedAccount<'info>,
    /// CHECK: Stored as the base claim escrow for h-omLP asset1 wrappers; validation is added in the hedge layer.
    pub hedge1_vault: UncheckedAccount<'info>,
    /// CHECK: Stored as the reserve vault for asset0; PDA/token-account validation is added in the reserve layer.
    pub reserve0_vault: UncheckedAccount<'info>,
    /// CHECK: Stored as the reserve vault for asset1; PDA/token-account validation is added in the reserve layer.
    pub reserve1_vault: UncheckedAccount<'info>,
    /// CHECK: Stored as the collateral vault for asset0; validation is added in the margin layer.
    pub collateral0_vault: UncheckedAccount<'info>,
    /// CHECK: Stored as the collateral vault for asset1; validation is added in the margin layer.
    pub collateral1_vault: UncheckedAccount<'info>,
    /// CHECK: Stored as the junior insurance reserve vault for asset0; validation is added in the insurance layer.
    pub insurance0_vault: UncheckedAccount<'info>,
    /// CHECK: Stored as the junior insurance reserve vault for asset1; validation is added in the insurance layer.
    pub insurance1_vault: UncheckedAccount<'info>,
    /// CHECK: Stored as the non-compounding fee vault for asset0; validation is added in the fee layer.
    pub fee0_vault: UncheckedAccount<'info>,
    /// CHECK: Stored as the non-compounding fee vault for asset1; validation is added in the fee layer.
    pub fee1_vault: UncheckedAccount<'info>,
    /// CHECK: Stored as the staked claim escrow for asset0; validation is added in the staking layer.
    pub claim0_stake_vault: UncheckedAccount<'info>,
    /// CHECK: Stored as the staked claim escrow for asset1; validation is added in the staking layer.
    pub claim1_stake_vault: UncheckedAccount<'info>,

    pub system_program: Program<'info, System>,
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
        args.config.validate()
    }

    pub fn handle_initialize(ctx: Context<Self>, args: InitializeMarketArgs) -> Result<()> {
        let current_slot = Clock::get()?.slot;
        let market_key = ctx.accounts.market.key();

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
}
