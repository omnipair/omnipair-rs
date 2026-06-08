use anchor_lang::prelude::*;
use anchor_spl::token_interface::Mint;

use crate::{
    constants::*,
    errors::ErrorCode,
    events::{MarketCreatedV2, MarketEventMetadataV2, MarketUpdatedV2},
    state::{MarketConfigV2, MarketSideV2, MarketV2},
    utils::account::get_size_with_discriminator,
};

#[derive(AnchorSerialize, AnchorDeserialize, Clone)]
pub struct InitializeMarketV2Args {
    pub operator: Pubkey,
    pub manager: Pubkey,
    pub config: MarketConfigV2,
    pub params_hash: [u8; 32],
}

#[derive(AnchorSerialize, AnchorDeserialize, Clone)]
pub struct UpdateMarketConfigV2Args {
    pub config: MarketConfigV2,
}

#[derive(AnchorSerialize, AnchorDeserialize, Clone)]
pub struct SetMarketReduceOnlyV2Args {
    pub reduce_only: bool,
}

#[event_cpi]
#[derive(Accounts)]
#[instruction(args: InitializeMarketV2Args)]
pub struct InitializeMarketV2<'info> {
    #[account(mut)]
    pub payer: Signer<'info>,

    pub asset0_mint: InterfaceAccount<'info, Mint>,
    pub asset1_mint: InterfaceAccount<'info, Mint>,

    #[account(
        init,
        payer = payer,
        space = get_size_with_discriminator::<MarketV2>(),
        seeds = [
            MARKET_V2_SEED_PREFIX,
            asset0_mint.key().as_ref(),
            asset1_mint.key().as_ref(),
            args.params_hash.as_ref(),
        ],
        bump
    )]
    pub market: Account<'info, MarketV2>,

    /// CHECK: Stored as the protected claim mint for asset0; initialized in a later token-layer instruction.
    pub claim0_mint: UncheckedAccount<'info>,
    /// CHECK: Stored as the protected claim mint for asset1; initialized in a later token-layer instruction.
    pub claim1_mint: UncheckedAccount<'info>,
    /// CHECK: Stored as the hedged wrapper mint for asset0; initialized in a later token-layer instruction.
    pub hedge0_mint: UncheckedAccount<'info>,
    /// CHECK: Stored as the hedged wrapper mint for asset1; initialized in a later token-layer instruction.
    pub hedge1_mint: UncheckedAccount<'info>,
    /// CHECK: Stored as the reserve vault for asset0; PDA/token-account validation is added in the reserve layer.
    pub reserve0_vault: UncheckedAccount<'info>,
    /// CHECK: Stored as the reserve vault for asset1; PDA/token-account validation is added in the reserve layer.
    pub reserve1_vault: UncheckedAccount<'info>,
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

impl<'info> InitializeMarketV2<'info> {
    pub fn validate(&self, args: &InitializeMarketV2Args) -> Result<()> {
        require_gt!(
            self.asset1_mint.key(),
            self.asset0_mint.key(),
            ErrorCode::InvalidTokenOrder
        );
        require_keys_neq!(
            args.operator,
            Pubkey::default(),
            ErrorCode::InvalidMarketConfigV2
        );
        args.config.validate()
    }

    pub fn handle_initialize(ctx: Context<Self>, args: InitializeMarketV2Args) -> Result<()> {
        let current_slot = Clock::get()?.slot;
        let market_key = ctx.accounts.market.key();

        let side0 = MarketSideV2 {
            asset_mint: ctx.accounts.asset0_mint.key(),
            claim_mint: ctx.accounts.claim0_mint.key(),
            hedge_mint: ctx.accounts.hedge0_mint.key(),
            reserve_vault: ctx.accounts.reserve0_vault.key(),
            fee_vault: ctx.accounts.fee0_vault.key(),
            stake_vault: ctx.accounts.claim0_stake_vault.key(),
            buffer_book: crate::state::BufferBookV2 {
                buffer_ratio_bps: args.config.buffer_ratio_bps,
                ..crate::state::BufferBookV2::default()
            },
            ..MarketSideV2::default()
        };
        let side1 = MarketSideV2 {
            asset_mint: ctx.accounts.asset1_mint.key(),
            claim_mint: ctx.accounts.claim1_mint.key(),
            hedge_mint: ctx.accounts.hedge1_mint.key(),
            reserve_vault: ctx.accounts.reserve1_vault.key(),
            fee_vault: ctx.accounts.fee1_vault.key(),
            stake_vault: ctx.accounts.claim1_stake_vault.key(),
            buffer_book: crate::state::BufferBookV2 {
                buffer_ratio_bps: args.config.buffer_ratio_bps,
                ..crate::state::BufferBookV2::default()
            },
            ..MarketSideV2::default()
        };

        ctx.accounts.market.set_inner(MarketV2::initialize(
            ctx.accounts.asset0_mint.key(),
            ctx.accounts.asset1_mint.key(),
            args.operator,
            args.manager,
            side0,
            side1,
            args.config,
            args.params_hash,
            current_slot,
            ctx.bumps.market,
        )?);

        emit_cpi!(MarketCreatedV2 {
            market: market_key,
            asset0_mint: ctx.accounts.asset0_mint.key(),
            asset1_mint: ctx.accounts.asset1_mint.key(),
            claim0_mint: ctx.accounts.claim0_mint.key(),
            claim1_mint: ctx.accounts.claim1_mint.key(),
            claim0_stake_vault: ctx.accounts.claim0_stake_vault.key(),
            claim1_stake_vault: ctx.accounts.claim1_stake_vault.key(),
            hedge0_mint: ctx.accounts.hedge0_mint.key(),
            hedge1_mint: ctx.accounts.hedge1_mint.key(),
            operator: args.operator,
            manager: args.manager,
            buffer_ratio_bps: args.config.buffer_ratio_bps,
            swap_fee_bps: args.config.swap_fee_bps,
            params_hash: args.params_hash,
            version: MARKET_V2_VERSION,
            metadata: MarketEventMetadataV2::new(ctx.accounts.payer.key(), market_key),
        });

        Ok(())
    }
}

#[event_cpi]
#[derive(Accounts)]
pub struct UpdateMarketConfigV2<'info> {
    #[account(
        mut,
        seeds = [
            MARKET_V2_SEED_PREFIX,
            market.asset0_mint.as_ref(),
            market.asset1_mint.as_ref(),
            market.params_hash.as_ref(),
        ],
        bump = market.bump
    )]
    pub market: Account<'info, MarketV2>,

    #[account(address = market.operator @ ErrorCode::InvalidMarketV2)]
    pub operator: Signer<'info>,
}

impl<'info> UpdateMarketConfigV2<'info> {
    pub fn handle_update(ctx: Context<Self>, args: UpdateMarketConfigV2Args) -> Result<()> {
        args.config.validate()?;
        let market = &mut ctx.accounts.market;
        market.config = args.config;
        market.side0.buffer_book.buffer_ratio_bps = args.config.buffer_ratio_bps;
        market.side1.buffer_book.buffer_ratio_bps = args.config.buffer_ratio_bps;

        emit_cpi!(MarketUpdatedV2 {
            market: market.key(),
            reduce_only: market.reduce_only,
            buffer_ratio_bps: market.config.buffer_ratio_bps,
            swap_fee_bps: market.config.swap_fee_bps,
            operator_fee_bps: market.config.operator_fee_bps,
            metadata: MarketEventMetadataV2::new(ctx.accounts.operator.key(), market.key()),
        });

        Ok(())
    }
}

#[event_cpi]
#[derive(Accounts)]
pub struct SetMarketReduceOnlyV2<'info> {
    #[account(
        mut,
        seeds = [
            MARKET_V2_SEED_PREFIX,
            market.asset0_mint.as_ref(),
            market.asset1_mint.as_ref(),
            market.params_hash.as_ref(),
        ],
        bump = market.bump
    )]
    pub market: Account<'info, MarketV2>,

    #[account(address = market.operator @ ErrorCode::InvalidMarketV2)]
    pub operator: Signer<'info>,
}

impl<'info> SetMarketReduceOnlyV2<'info> {
    pub fn handle_set(ctx: Context<Self>, args: SetMarketReduceOnlyV2Args) -> Result<()> {
        let market = &mut ctx.accounts.market;
        market.reduce_only = args.reduce_only;

        emit_cpi!(MarketUpdatedV2 {
            market: market.key(),
            reduce_only: market.reduce_only,
            buffer_ratio_bps: market.config.buffer_ratio_bps,
            swap_fee_bps: market.config.swap_fee_bps,
            operator_fee_bps: market.config.operator_fee_bps,
            metadata: MarketEventMetadataV2::new(ctx.accounts.operator.key(), market.key()),
        });

        Ok(())
    }
}

#[derive(Accounts)]
pub struct ViewMarketStateV2<'info> {
    #[account(
        seeds = [
            MARKET_V2_SEED_PREFIX,
            market.asset0_mint.as_ref(),
            market.asset1_mint.as_ref(),
            market.params_hash.as_ref(),
        ],
        bump = market.bump
    )]
    pub market: Account<'info, MarketV2>,
}

impl ViewMarketStateV2<'_> {
    pub fn handle_view(ctx: Context<Self>) -> Result<()> {
        let market = &ctx.accounts.market;
        msg!(
            "MarketV2: market={}, asset0={}, asset1={}, reduce_only={}, buffer_ratio_bps={}",
            market.key(),
            market.asset0_mint,
            market.asset1_mint,
            market.reduce_only,
            market.config.buffer_ratio_bps
        );
        msg!(
            "MarketV2 side0: reserve={}, protected_claims={}, required_buffer={}, fee_liability={}",
            market.side0.reserve_ledger.live_reserve,
            market.side0.claim_ledger.protected_claim_supply,
            market.side0.buffer_book.required_buffer,
            market.side0.fee_ledger.fee_liability
        );
        msg!(
            "MarketV2 side1: reserve={}, protected_claims={}, required_buffer={}, fee_liability={}",
            market.side1.reserve_ledger.live_reserve,
            market.side1.claim_ledger.protected_claim_supply,
            market.side1.buffer_book.required_buffer,
            market.side1.fee_ledger.fee_liability
        );
        Ok(())
    }
}
