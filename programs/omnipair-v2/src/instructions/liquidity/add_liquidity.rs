use anchor_lang::prelude::*;
use anchor_spl::{
    token::Token,
    token_interface::{Mint, Token2022, TokenAccount},
};

use crate::{
    constants::*,
    errors::ErrorCode,
    events::{LiquidityAdded, MarketEventMetadata},
    generate_market_seeds,
    shared::{
        account::get_size_with_discriminator,
        token::{token_mint_to, transfer_from_user_to_vault},
    },
    state::{Market, MarketAsset, StakePosition},
    transitions::reserve::AddLiquidity as AddLiquidityTransition,
};

use crate::instructions::common::{
    require_fee_free_claim_token_mint, require_supported_asset_mint, token_program_for_mint,
    validate_reserve_accounts,
};

#[derive(AnchorSerialize, AnchorDeserialize, Clone)]
pub struct AddLiquidityArgs {
    pub market_asset: MarketAsset,
    pub deposit_amount: u64,
    pub min_claim_amount: u64,
    pub max_buffer_amount: u64,
}

#[event_cpi]
#[derive(Accounts)]
#[instruction(args: AddLiquidityArgs)]
pub struct AddLiquidity<'info> {
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

    #[account(mut)]
    pub owner: Signer<'info>,

    pub asset_mint: Box<InterfaceAccount<'info, Mint>>,

    #[account(mut)]
    pub claim_token_mint: Box<InterfaceAccount<'info, Mint>>,

    #[account(mut)]
    pub reserve_vault: Box<InterfaceAccount<'info, TokenAccount>>,

    #[account(mut)]
    pub owner_asset_account: Box<InterfaceAccount<'info, TokenAccount>>,

    #[account(mut)]
    pub owner_claim_account: Box<InterfaceAccount<'info, TokenAccount>>,

    #[account(
        init_if_needed,
        payer = owner,
        space = get_size_with_discriminator::<StakePosition>(),
        seeds = [
            STAKE_POSITION_SEED_PREFIX,
            market.key().as_ref(),
            owner.key().as_ref(),
            asset_mint.key().as_ref(),
        ],
        bump
    )]
    pub stake_position: Box<Account<'info, StakePosition>>,

    pub token_program: Program<'info, Token>,
    pub token_2022_program: Program<'info, Token2022>,
    pub system_program: Program<'info, System>,
}

impl<'info> AddLiquidity<'info> {
    pub fn validate(&self, args: &AddLiquidityArgs) -> Result<()> {
        self.market.assert_live()?;
        require!(args.deposit_amount > 0, ErrorCode::AmountZero);
        require_gte!(
            self.owner_asset_account.amount,
            args.deposit_amount,
            ErrorCode::InsufficientBalance
        );
        validate_reserve_accounts(
            &self.market,
            args.market_asset,
            self.owner.key(),
            &self.asset_mint,
            &self.claim_token_mint,
            &self.reserve_vault,
            &self.owner_asset_account,
            &self.owner_claim_account,
        )?;
        require_supported_asset_mint(&self.asset_mint)?;
        require_fee_free_claim_token_mint(&self.claim_token_mint)?;

        if self.stake_position.is_initialized() {
            self.stake_position.assert_position(
                self.owner.key(),
                self.market.key(),
                self.asset_mint.key(),
            )?;
        }

        Ok(())
    }

    pub fn handle_add_liquidity(ctx: Context<Self>, args: AddLiquidityArgs) -> Result<()> {
        let market_key = ctx.accounts.market.key();
        let owner_key = ctx.accounts.owner.key();
        let asset_mint_key = ctx.accounts.asset_mint.key();

        ctx.accounts.market.refresh_risk_book()?;

        if !ctx.accounts.stake_position.is_initialized() {
            ctx.accounts.stake_position.initialize(
                owner_key,
                market_key,
                asset_mint_key,
                ctx.bumps.stake_position,
            );
        }
        ctx.accounts
            .stake_position
            .assert_position(owner_key, market_key, asset_mint_key)?;

        let reserve_balance_before = ctx.accounts.reserve_vault.amount;
        let asset_token_program = token_program_for_mint(
            &ctx.accounts.asset_mint,
            &ctx.accounts.token_program,
            &ctx.accounts.token_2022_program,
        )?;

        transfer_from_user_to_vault(
            ctx.accounts.owner.to_account_info(),
            ctx.accounts.owner_asset_account.to_account_info(),
            ctx.accounts.reserve_vault.to_account_info(),
            ctx.accounts.asset_mint.to_account_info(),
            asset_token_program,
            args.deposit_amount,
            ctx.accounts.asset_mint.decimals,
        )?;
        ctx.accounts.reserve_vault.reload()?;

        let reserve_credit = ctx
            .accounts
            .reserve_vault
            .amount
            .checked_sub(reserve_balance_before)
            .ok_or(ErrorCode::MarketMathOverflow)?;
        let receipt = {
            let market_side = ctx.accounts.market.side_mut(args.market_asset)?;
            AddLiquidityTransition::new(reserve_credit).apply(market_side)?
        };
        if ctx.accounts.market.risk_book.base_price_ema_nad > 0
            && ctx.accounts.market.risk_book.quote_price_ema_nad > 0
        {
            ctx.accounts.market.assert_risk_circuit_breakers()?;
        }
        require_gte!(
            receipt.claim_amount,
            args.min_claim_amount,
            ErrorCode::SlippageExceeded
        );
        require_gte!(
            args.max_buffer_amount,
            receipt.buffer_amount,
            ErrorCode::SlippageExceeded
        );

        ctx.accounts
            .stake_position
            .credit_buffer_share_amount(receipt.buffer_amount)?;

        let claim_token_program = token_program_for_mint(
            &ctx.accounts.claim_token_mint,
            &ctx.accounts.token_program,
            &ctx.accounts.token_2022_program,
        )?;
        token_mint_to(
            ctx.accounts.market.to_account_info(),
            claim_token_program,
            ctx.accounts.claim_token_mint.to_account_info(),
            ctx.accounts.owner_claim_account.to_account_info(),
            receipt.claim_amount,
            &[&generate_market_seeds!(ctx.accounts.market)[..]],
        )?;

        emit_cpi!(LiquidityAdded {
            market: market_key,
            owner: owner_key,
            asset_mint: asset_mint_key,
            reserve_credit: receipt.reserve_credit,
            claim_amount: receipt.claim_amount,
            buffer_amount: receipt.buffer_amount,
            protected_claim_token_supply: receipt.protected_claim_token_supply,
            required_buffer: receipt.required_buffer,
            metadata: MarketEventMetadata::new(owner_key, market_key)?,
        });

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        constants::{BPS_DENOMINATOR, NAD},
        state::{BufferLedger, MarketConfig, MarketSide, ReserveLedger},
    };

    const TEST_RESERVE: u64 = 1_000_000;

    fn market_side(asset_mint: Pubkey) -> MarketSide {
        MarketSide {
            asset_mint,
            asset_decimals: 6,
            claim_token_mint: Pubkey::new_unique(),
            hedge_token_mint: Pubkey::new_unique(),
            hedge_vault: Pubkey::new_unique(),
            reserve_vault: Pubkey::new_unique(),
            collateral_vault: Pubkey::new_unique(),
            fee_vault: Pubkey::new_unique(),
            stake_vault: Pubkey::new_unique(),
            reserve_ledger: ReserveLedger {
                live_reserve: TEST_RESERVE,
                cash_reserve: TEST_RESERVE,
                reserved_liability: 0,
            },
            buffer_ledger: BufferLedger {
                buffer_ratio_bps: 2_000,
                ..BufferLedger::default()
            },
            ..MarketSide::default()
        }
    }

    fn market_config() -> MarketConfig {
        MarketConfig {
            swap_fee_bps: 30,
            operator_fee_bps: 1_000,
            protocol_fee_bps: 0,
            buffer_ratio_bps: 2_000,
            fee_routing_k_nad: NAD,
            ema_half_life_ms: 60_000,
            directional_ema_half_life_ms: 60_000,
            k_ema_half_life_ms: 60_000,
            max_daily_borrow_bps: BPS_DENOMINATOR,
            max_daily_withdraw_bps: BPS_DENOMINATOR,
            spot_ema_divergence_bps: 1_000,
            k_ema_drawdown_bps: BPS_DENOMINATOR,
            recognized_collateral_cap_bps: 15_000,
            market_health_min_bps: 11_000,
            effective_debt_weight_min_bps: BPS_DENOMINATOR,
            effective_debt_gamma_nad: NAD,
            soft_borrow_enabled: false,
            hedged_lp_enabled: true,
            start_time: 0,
        }
    }

    fn test_market() -> Market {
        let base_mint = Pubkey::new_unique();
        let quote_mint = Pubkey::new_unique();
        Market::initialize(
            base_mint,
            quote_mint,
            Pubkey::new_unique(),
            Pubkey::new_unique(),
            market_side(base_mint),
            market_side(quote_mint),
            market_config(),
            [29_u8; 32],
            42,
            250,
        )
        .unwrap()
    }

    #[test]
    fn pre_add_liquidity_risk_snapshot_blocks_bootstrap_from_post_add_spot() {
        let mut market = test_market();
        market.refresh_risk_book().unwrap();
        assert_eq!(market.risk_book.base_price_ema_nad, NAD);
        assert_eq!(market.risk_book.quote_price_ema_nad, NAD);

        {
            let market_side = market.side_mut(MarketAsset::Base).unwrap();
            AddLiquidityTransition::new(900_000)
                .apply(market_side)
                .unwrap();
        }
        let err = market.assert_risk_circuit_breakers().unwrap_err();
        assert_eq!(
            err,
            anchor_lang::prelude::error!(ErrorCode::MarketRiskCircuitBreaker)
        );
        market.refresh_risk_book().unwrap();

        let err = market.assert_risk_circuit_breakers().unwrap_err();
        assert_eq!(
            err,
            anchor_lang::prelude::error!(ErrorCode::MarketRiskCircuitBreaker)
        );
    }
}
