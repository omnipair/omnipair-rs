use anchor_lang::prelude::*;
use anchor_spl::{
    token::Token,
    token_interface::{Mint, Token2022, TokenAccount},
};

use crate::{
    constants::*,
    errors::ErrorCode,
    events::{MarketEventMetadataV2, MarketInsuranceFundedV2, MarketLiquidatedV2},
    generate_market_v2_seeds,
    state::{DebtBookV2, MarginPositionV2, MarketV2},
    utils::{
        math::ceil_div,
        token::{
            transfer_from_user_to_vault, transfer_from_vault_to_user, transfer_from_vault_to_vault,
        },
    },
};

use super::common::{require_supported_asset_mint, token_program_for_mint};

#[derive(AnchorSerialize, AnchorDeserialize, Clone)]
pub struct DepositInsuranceV2Args {
    pub market_side_index: u8,
    pub deposit_amount: u64,
}

#[derive(AnchorSerialize, AnchorDeserialize, Clone)]
pub struct LiquidateV2Args {
    pub debt_asset_is_asset0: bool,
    pub repay_amount: u64,
    pub min_collateral_out: u64,
    pub max_insurance_draw: u64,
    pub max_socialized_loss: u64,
}

#[event_cpi]
#[derive(Accounts)]
#[instruction(args: DepositInsuranceV2Args)]
pub struct DepositInsuranceV2<'info> {
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
    pub market: Box<Account<'info, MarketV2>>,

    #[account(mut)]
    pub sponsor: Signer<'info>,

    pub asset_mint: Box<InterfaceAccount<'info, Mint>>,

    #[account(mut)]
    pub insurance_vault: Box<InterfaceAccount<'info, TokenAccount>>,

    #[account(mut)]
    pub sponsor_asset_account: Box<InterfaceAccount<'info, TokenAccount>>,

    pub token_program: Program<'info, Token>,
    pub token_2022_program: Program<'info, Token2022>,
}

impl<'info> DepositInsuranceV2<'info> {
    pub fn validate(&self, args: &DepositInsuranceV2Args) -> Result<()> {
        self.market.assert_started()?;
        require!(args.deposit_amount > 0, ErrorCode::AmountZero);
        require_gte!(
            self.sponsor_asset_account.amount,
            args.deposit_amount,
            ErrorCode::InsufficientBalance
        );
        validate_insurance_accounts(
            &self.market,
            args.market_side_index,
            self.sponsor.key(),
            &self.asset_mint,
            &self.insurance_vault,
            &self.sponsor_asset_account,
        )?;
        require_supported_asset_mint(&self.asset_mint)?;
        Ok(())
    }

    pub fn handle_deposit(ctx: Context<Self>, args: DepositInsuranceV2Args) -> Result<()> {
        let market_key = ctx.accounts.market.key();
        let sponsor_key = ctx.accounts.sponsor.key();
        let asset_mint_key = ctx.accounts.asset_mint.key();
        let vault_balance_before = ctx.accounts.insurance_vault.amount;
        let asset_token_program = token_program_for_mint(
            &ctx.accounts.asset_mint,
            &ctx.accounts.token_program,
            &ctx.accounts.token_2022_program,
        )?;

        transfer_from_user_to_vault(
            ctx.accounts.sponsor.to_account_info(),
            ctx.accounts.sponsor_asset_account.to_account_info(),
            ctx.accounts.insurance_vault.to_account_info(),
            ctx.accounts.asset_mint.to_account_info(),
            asset_token_program,
            args.deposit_amount,
            ctx.accounts.asset_mint.decimals,
        )?;
        ctx.accounts.insurance_vault.reload()?;

        let insurance_credit = ctx
            .accounts
            .insurance_vault
            .amount
            .checked_sub(vault_balance_before)
            .ok_or(ErrorCode::MarketMathOverflowV2)?;
        require!(insurance_credit > 0, ErrorCode::AmountZero);

        if args.market_side_index == 0 {
            ctx.accounts.market.insurance_reserve.available0 = ctx
                .accounts
                .market
                .insurance_reserve
                .available0
                .checked_add(insurance_credit)
                .ok_or(ErrorCode::MarketMathOverflowV2)?;
        } else {
            ctx.accounts.market.insurance_reserve.available1 = ctx
                .accounts
                .market
                .insurance_reserve
                .available1
                .checked_add(insurance_credit)
                .ok_or(ErrorCode::MarketMathOverflowV2)?;
        }

        emit_cpi!(MarketInsuranceFundedV2 {
            market: market_key,
            sponsor: sponsor_key,
            asset_mint: asset_mint_key,
            insurance_credit,
            available0: ctx.accounts.market.insurance_reserve.available0,
            available1: ctx.accounts.market.insurance_reserve.available1,
            metadata: MarketEventMetadataV2::new(sponsor_key, market_key),
        });

        Ok(())
    }
}

#[event_cpi]
#[derive(Accounts)]
#[instruction(args: LiquidateV2Args)]
pub struct LiquidateV2<'info> {
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
    pub market: Box<Account<'info, MarketV2>>,

    #[account(mut)]
    pub liquidator: Signer<'info>,

    pub debt_asset_mint: Box<InterfaceAccount<'info, Mint>>,

    pub collateral_asset_mint: Box<InterfaceAccount<'info, Mint>>,

    #[account(mut)]
    pub reserve_vault: Box<InterfaceAccount<'info, TokenAccount>>,

    #[account(mut)]
    pub collateral_vault: Box<InterfaceAccount<'info, TokenAccount>>,

    #[account(mut)]
    pub insurance_vault: Box<InterfaceAccount<'info, TokenAccount>>,

    #[account(mut)]
    pub liquidator_debt_account: Box<InterfaceAccount<'info, TokenAccount>>,

    #[account(mut)]
    pub liquidator_collateral_account: Box<InterfaceAccount<'info, TokenAccount>>,

    #[account(
        mut,
        seeds = [
            MARGIN_POSITION_V2_SEED_PREFIX,
            market.key().as_ref(),
            margin_position.owner.as_ref(),
        ],
        bump = margin_position.bump
    )]
    pub margin_position: Box<Account<'info, MarginPositionV2>>,

    pub token_program: Program<'info, Token>,
    pub token_2022_program: Program<'info, Token2022>,
}

impl<'info> LiquidateV2<'info> {
    pub fn validate(&self, args: &LiquidateV2Args) -> Result<()> {
        self.market.assert_started()?;
        require!(args.repay_amount > 0, ErrorCode::AmountZero);
        require_gte!(
            self.liquidator_debt_account.amount,
            args.repay_amount,
            ErrorCode::InsufficientBalance
        );
        validate_liquidation_accounts(
            &self.market,
            args.debt_asset_is_asset0,
            self.liquidator.key(),
            &self.debt_asset_mint,
            &self.collateral_asset_mint,
            &self.reserve_vault,
            &self.collateral_vault,
            &self.insurance_vault,
            &self.liquidator_debt_account,
            &self.liquidator_collateral_account,
        )?;
        require_supported_asset_mint(&self.debt_asset_mint)?;
        require_supported_asset_mint(&self.collateral_asset_mint)?;
        require_keys_eq!(
            self.margin_position.market,
            self.market.key(),
            ErrorCode::InvalidMarginPositionV2
        );
        let health_bps = position_health_bps(
            &self.market,
            &self.margin_position,
            args.debt_asset_is_asset0,
        )?;
        require!(
            health_bps < self.market.config.market_health_min_bps as u64,
            ErrorCode::PositionNotLiquidatableV2
        );
        Ok(())
    }

    pub fn handle_liquidate(ctx: Context<Self>, args: LiquidateV2Args) -> Result<()> {
        let market_key = ctx.accounts.market.key();
        let borrower_key = ctx.accounts.margin_position.owner;
        let liquidator_key = ctx.accounts.liquidator.key();
        let debt_asset_mint_key = ctx.accounts.debt_asset_mint.key();
        let collateral_asset_mint_key = ctx.accounts.collateral_asset_mint.key();

        let reserve_balance_before_repay = ctx.accounts.reserve_vault.amount;
        let debt_token_program = token_program_for_mint(
            &ctx.accounts.debt_asset_mint,
            &ctx.accounts.token_program,
            &ctx.accounts.token_2022_program,
        )?;
        transfer_from_user_to_vault(
            ctx.accounts.liquidator.to_account_info(),
            ctx.accounts.liquidator_debt_account.to_account_info(),
            ctx.accounts.reserve_vault.to_account_info(),
            ctx.accounts.debt_asset_mint.to_account_info(),
            debt_token_program.clone(),
            args.repay_amount,
            ctx.accounts.debt_asset_mint.decimals,
        )?;
        ctx.accounts.reserve_vault.reload()?;
        let repay_credit = ctx
            .accounts
            .reserve_vault
            .amount
            .checked_sub(reserve_balance_before_repay)
            .ok_or(ErrorCode::MarketMathOverflowV2)?;
        require!(repay_credit > 0, ErrorCode::AmountZero);

        let insurance_request = insurance_request_for_liquidation(
            &ctx.accounts.market,
            &ctx.accounts.margin_position,
            args.debt_asset_is_asset0,
            repay_credit,
            args.max_insurance_draw,
        )?;

        let (insurance_spent, insurance_credit) = if insurance_request > 0 {
            let reserve_balance_before_insurance = ctx.accounts.reserve_vault.amount;
            let insurance_balance_before = ctx.accounts.insurance_vault.amount;
            transfer_from_vault_to_vault(
                ctx.accounts.market.to_account_info(),
                ctx.accounts.insurance_vault.to_account_info(),
                ctx.accounts.reserve_vault.to_account_info(),
                ctx.accounts.debt_asset_mint.to_account_info(),
                debt_token_program,
                insurance_request,
                ctx.accounts.debt_asset_mint.decimals,
                &[&generate_market_v2_seeds!(ctx.accounts.market)[..]],
            )?;
            ctx.accounts.reserve_vault.reload()?;
            ctx.accounts.insurance_vault.reload()?;
            (
                insurance_balance_before
                    .checked_sub(ctx.accounts.insurance_vault.amount)
                    .ok_or(ErrorCode::MarketMathOverflowV2)?,
                ctx.accounts
                    .reserve_vault
                    .amount
                    .checked_sub(reserve_balance_before_insurance)
                    .ok_or(ErrorCode::MarketMathOverflowV2)?,
            )
        } else {
            (0, 0)
        };

        let outcome = apply_liquidation_state(
            &mut ctx.accounts.market,
            &mut ctx.accounts.margin_position,
            args.debt_asset_is_asset0,
            repay_credit,
            insurance_spent,
            insurance_credit,
            args.max_socialized_loss,
        )?;
        require_gte!(
            outcome.collateral_seized,
            args.min_collateral_out,
            ErrorCode::SlippageExceeded
        );

        if outcome.collateral_seized > 0 {
            let collateral_token_program = token_program_for_mint(
                &ctx.accounts.collateral_asset_mint,
                &ctx.accounts.token_program,
                &ctx.accounts.token_2022_program,
            )?;
            transfer_from_vault_to_user(
                ctx.accounts.market.to_account_info(),
                ctx.accounts.collateral_vault.to_account_info(),
                ctx.accounts.liquidator_collateral_account.to_account_info(),
                ctx.accounts.collateral_asset_mint.to_account_info(),
                collateral_token_program,
                outcome.collateral_seized,
                ctx.accounts.collateral_asset_mint.decimals,
                &[&generate_market_v2_seeds!(ctx.accounts.market)[..]],
            )?;
        }

        emit_cpi!(MarketLiquidatedV2 {
            market: market_key,
            borrower: borrower_key,
            liquidator: liquidator_key,
            debt_asset_mint: debt_asset_mint_key,
            collateral_asset_mint: collateral_asset_mint_key,
            repaid_amount: outcome.repaid_amount,
            collateral_seized: outcome.collateral_seized,
            insurance_drawn: outcome.insurance_drawn,
            socialized_loss: outcome.socialized_loss,
            remaining_debt: outcome.remaining_debt,
            metadata: MarketEventMetadataV2::new(liquidator_key, market_key),
        });

        Ok(())
    }
}

#[derive(Clone, Copy, Debug)]
struct LiquidationOutcome {
    repaid_amount: u64,
    collateral_seized: u64,
    insurance_drawn: u64,
    socialized_loss: u64,
    remaining_debt: u128,
}

fn validate_insurance_accounts<'info>(
    market: &Account<'info, MarketV2>,
    market_side_index: u8,
    owner: Pubkey,
    asset_mint: &InterfaceAccount<'info, Mint>,
    insurance_vault: &InterfaceAccount<'info, TokenAccount>,
    owner_asset_account: &InterfaceAccount<'info, TokenAccount>,
) -> Result<()> {
    let market_side = market.side(market_side_index)?;
    let expected_vault = if market_side_index == 0 {
        market.insurance_reserve.vault0
    } else {
        market.insurance_reserve.vault1
    };
    require_keys_eq!(
        market_side.asset_mint,
        asset_mint.key(),
        ErrorCode::InvalidMint
    );
    require_keys_eq!(
        expected_vault,
        insurance_vault.key(),
        ErrorCode::InvalidVault
    );
    require_keys_eq!(
        insurance_vault.mint,
        asset_mint.key(),
        ErrorCode::InvalidVault
    );
    require_keys_eq!(insurance_vault.owner, market.key(), ErrorCode::InvalidVault);
    require_keys_eq!(
        owner_asset_account.mint,
        asset_mint.key(),
        ErrorCode::InvalidTokenAccount
    );
    require_keys_eq!(
        owner_asset_account.owner,
        owner,
        ErrorCode::InvalidTokenAccount
    );
    Ok(())
}

fn validate_liquidation_accounts<'info>(
    market: &Account<'info, MarketV2>,
    debt_asset_is_asset0: bool,
    liquidator: Pubkey,
    debt_asset_mint: &InterfaceAccount<'info, Mint>,
    collateral_asset_mint: &InterfaceAccount<'info, Mint>,
    reserve_vault: &InterfaceAccount<'info, TokenAccount>,
    collateral_vault: &InterfaceAccount<'info, TokenAccount>,
    insurance_vault: &InterfaceAccount<'info, TokenAccount>,
    liquidator_debt_account: &InterfaceAccount<'info, TokenAccount>,
    liquidator_collateral_account: &InterfaceAccount<'info, TokenAccount>,
) -> Result<()> {
    let (debt_side, collateral_side, insurance_vault_key) = if debt_asset_is_asset0 {
        (
            &market.side0,
            &market.side1,
            market.insurance_reserve.vault0,
        )
    } else {
        (
            &market.side1,
            &market.side0,
            market.insurance_reserve.vault1,
        )
    };
    require_keys_eq!(
        debt_side.asset_mint,
        debt_asset_mint.key(),
        ErrorCode::InvalidMint
    );
    require_keys_eq!(
        collateral_side.asset_mint,
        collateral_asset_mint.key(),
        ErrorCode::InvalidMint
    );
    require_keys_eq!(
        debt_side.reserve_vault,
        reserve_vault.key(),
        ErrorCode::InvalidVault
    );
    require_keys_eq!(
        collateral_side.collateral_vault,
        collateral_vault.key(),
        ErrorCode::InvalidVault
    );
    require_keys_eq!(
        insurance_vault_key,
        insurance_vault.key(),
        ErrorCode::InvalidVault
    );
    require_keys_eq!(
        reserve_vault.mint,
        debt_asset_mint.key(),
        ErrorCode::InvalidVault
    );
    require_keys_eq!(
        insurance_vault.mint,
        debt_asset_mint.key(),
        ErrorCode::InvalidVault
    );
    require_keys_eq!(
        collateral_vault.mint,
        collateral_asset_mint.key(),
        ErrorCode::InvalidVault
    );
    require_keys_eq!(reserve_vault.owner, market.key(), ErrorCode::InvalidVault);
    require_keys_eq!(insurance_vault.owner, market.key(), ErrorCode::InvalidVault);
    require_keys_eq!(
        collateral_vault.owner,
        market.key(),
        ErrorCode::InvalidVault
    );
    require_keys_eq!(
        liquidator_debt_account.mint,
        debt_asset_mint.key(),
        ErrorCode::InvalidTokenAccount
    );
    require_keys_eq!(
        liquidator_debt_account.owner,
        liquidator,
        ErrorCode::InvalidTokenAccount
    );
    require_keys_eq!(
        liquidator_collateral_account.mint,
        collateral_asset_mint.key(),
        ErrorCode::InvalidTokenAccount
    );
    require_keys_eq!(
        liquidator_collateral_account.owner,
        liquidator,
        ErrorCode::InvalidTokenAccount
    );
    Ok(())
}

fn position_health_bps(
    market: &MarketV2,
    margin_position: &MarginPositionV2,
    debt_asset_is_asset0: bool,
) -> Result<u64> {
    let (recognized_collateral, debt_amount) = if debt_asset_is_asset0 {
        (
            margin_position.recognized_collateral1_for_debt0,
            margin_position.fixed_debt0(&market.debt_book)?,
        )
    } else {
        (
            margin_position.recognized_collateral0_for_debt1,
            margin_position.fixed_debt1(&market.debt_book)?,
        )
    };
    if debt_amount == 0 {
        return Ok(u64::MAX);
    }
    let health = (recognized_collateral as u128)
        .checked_mul(BPS_DENOMINATOR as u128)
        .and_then(|value| value.checked_div(debt_amount))
        .ok_or(ErrorCode::MarketMathOverflowV2)?;
    u64::try_from(health).map_err(|_| ErrorCode::MarketMathOverflowV2.into())
}

fn insurance_request_for_liquidation(
    market: &MarketV2,
    margin_position: &MarginPositionV2,
    debt_asset_is_asset0: bool,
    repay_credit: u64,
    max_insurance_draw: u64,
) -> Result<u64> {
    let debt_before = position_debt(market, margin_position, debt_asset_is_asset0)?;
    require_gte!(
        debt_before,
        repay_credit as u128,
        ErrorCode::InsufficientDebt
    );
    let collateral_before = position_collateral(margin_position, debt_asset_is_asset0);
    let collateral_seized = collateral_to_seize(repay_credit, collateral_before)?;
    let remaining_debt = debt_before
        .checked_sub(repay_credit as u128)
        .ok_or(ErrorCode::MarketMathOverflowV2)?;
    if collateral_seized < collateral_before || remaining_debt == 0 {
        return Ok(0);
    }
    let available = if debt_asset_is_asset0 {
        market.insurance_reserve.available0
    } else {
        market.insurance_reserve.available1
    };
    Ok((remaining_debt as u64)
        .min(available)
        .min(max_insurance_draw))
}

fn apply_liquidation_state(
    market: &mut MarketV2,
    margin_position: &mut MarginPositionV2,
    debt_asset_is_asset0: bool,
    repay_credit: u64,
    insurance_spent: u64,
    insurance_credit: u64,
    max_socialized_loss: u64,
) -> Result<LiquidationOutcome> {
    let debt_before = position_debt(market, margin_position, debt_asset_is_asset0)?;
    require_gte!(
        debt_before,
        repay_credit as u128,
        ErrorCode::InsufficientDebt
    );
    let collateral_before = position_collateral(margin_position, debt_asset_is_asset0);
    let collateral_seized = collateral_to_seize(repay_credit, collateral_before)?;
    let collateral_exhausted = collateral_seized == collateral_before;
    let repay_plus_insurance = (repay_credit as u128)
        .checked_add(insurance_credit as u128)
        .ok_or(ErrorCode::MarketMathOverflowV2)?;
    require_gte!(
        debt_before,
        repay_plus_insurance,
        ErrorCode::InsufficientDebt
    );

    let bad_debt = debt_before
        .checked_sub(repay_plus_insurance)
        .ok_or(ErrorCode::MarketMathOverflowV2)?;
    let socialized_loss = if collateral_exhausted {
        u64::try_from(bad_debt).map_err(|_| ErrorCode::MarketMathOverflowV2)?
    } else {
        0
    };
    require_gte!(
        max_socialized_loss,
        socialized_loss,
        ErrorCode::LiquidationSocializationExceededV2
    );
    if bad_debt > 0 && !collateral_exhausted {
        require!(
            socialized_loss == 0,
            ErrorCode::InsufficientInsuranceReserveV2
        );
    }

    let debt_reduction = repay_plus_insurance
        .checked_add(socialized_loss as u128)
        .ok_or(ErrorCode::MarketMathOverflowV2)?;
    apply_liquidation_debt_reduction(
        market,
        margin_position,
        debt_asset_is_asset0,
        debt_reduction,
        collateral_seized,
    )?;

    let debt_side = if debt_asset_is_asset0 {
        &mut market.side0
    } else {
        &mut market.side1
    };
    debt_side.reserve_ledger.live_reserve = debt_side
        .reserve_ledger
        .live_reserve
        .checked_add(repay_credit)
        .and_then(|value| value.checked_add(insurance_credit))
        .ok_or(ErrorCode::ReserveOverflow)?;
    debt_side.reserve_ledger.cash_reserve = debt_side
        .reserve_ledger
        .cash_reserve
        .checked_add(repay_credit)
        .and_then(|value| value.checked_add(insurance_credit))
        .ok_or(ErrorCode::ReserveOverflow)?;
    if debt_asset_is_asset0 {
        market.insurance_reserve.available0 = market
            .insurance_reserve
            .available0
            .checked_sub(insurance_spent)
            .ok_or(ErrorCode::InsufficientInsuranceReserveV2)?;
    } else {
        market.insurance_reserve.available1 = market
            .insurance_reserve
            .available1
            .checked_sub(insurance_spent)
            .ok_or(ErrorCode::InsufficientInsuranceReserveV2)?;
    }

    market.refresh_market_health()?;
    Ok(LiquidationOutcome {
        repaid_amount: repay_credit,
        collateral_seized,
        insurance_drawn: insurance_credit,
        socialized_loss,
        remaining_debt: position_debt(market, margin_position, debt_asset_is_asset0)?,
    })
}

fn apply_liquidation_debt_reduction(
    market: &mut MarketV2,
    margin_position: &mut MarginPositionV2,
    debt_asset_is_asset0: bool,
    debt_reduction: u128,
    collateral_seized: u64,
) -> Result<()> {
    if debt_asset_is_asset0 {
        let shares_before = margin_position.fixed_debt0_shares;
        let debt_before = margin_position.fixed_debt0(&market.debt_book)?;
        let shares_to_burn = shares_to_burn_for_reduction(
            debt_reduction,
            debt_before,
            shares_before,
            market.debt_book.borrow_index0_nad,
        )?;
        margin_position.collateral1 = margin_position
            .collateral1
            .checked_sub(collateral_seized)
            .ok_or(ErrorCode::InsufficientRecognizedCollateralV2)?;
        let recognized_decrease = recognized_decrease_after_seizure(
            margin_position.recognized_collateral1_for_debt0,
            margin_position.collateral1,
            shares_to_burn,
            shares_before,
        )?;
        margin_position.recognized_collateral1_for_debt0 = margin_position
            .recognized_collateral1_for_debt0
            .checked_sub(recognized_decrease)
            .ok_or(ErrorCode::MarketMathOverflowV2)?;
        margin_position.fixed_debt0_shares = margin_position
            .fixed_debt0_shares
            .checked_sub(shares_to_burn)
            .ok_or(ErrorCode::MarketMathOverflowV2)?;
        market.debt_book.fixed_debt0_shares = market
            .debt_book
            .fixed_debt0_shares
            .checked_sub(shares_to_burn)
            .ok_or(ErrorCode::MarketMathOverflowV2)?;
        market.recognition_ledger.debt_bearing_collateral1_for_debt0 = market
            .recognition_ledger
            .debt_bearing_collateral1_for_debt0
            .checked_sub(recognized_decrease)
            .ok_or(ErrorCode::MarketMathOverflowV2)?;
    } else {
        let shares_before = margin_position.fixed_debt1_shares;
        let debt_before = margin_position.fixed_debt1(&market.debt_book)?;
        let shares_to_burn = shares_to_burn_for_reduction(
            debt_reduction,
            debt_before,
            shares_before,
            market.debt_book.borrow_index1_nad,
        )?;
        margin_position.collateral0 = margin_position
            .collateral0
            .checked_sub(collateral_seized)
            .ok_or(ErrorCode::InsufficientRecognizedCollateralV2)?;
        let recognized_decrease = recognized_decrease_after_seizure(
            margin_position.recognized_collateral0_for_debt1,
            margin_position.collateral0,
            shares_to_burn,
            shares_before,
        )?;
        margin_position.recognized_collateral0_for_debt1 = margin_position
            .recognized_collateral0_for_debt1
            .checked_sub(recognized_decrease)
            .ok_or(ErrorCode::MarketMathOverflowV2)?;
        margin_position.fixed_debt1_shares = margin_position
            .fixed_debt1_shares
            .checked_sub(shares_to_burn)
            .ok_or(ErrorCode::MarketMathOverflowV2)?;
        market.debt_book.fixed_debt1_shares = market
            .debt_book
            .fixed_debt1_shares
            .checked_sub(shares_to_burn)
            .ok_or(ErrorCode::MarketMathOverflowV2)?;
        market.recognition_ledger.debt_bearing_collateral0_for_debt1 = market
            .recognition_ledger
            .debt_bearing_collateral0_for_debt1
            .checked_sub(recognized_decrease)
            .ok_or(ErrorCode::MarketMathOverflowV2)?;
    }
    Ok(())
}

fn position_debt(
    market: &MarketV2,
    margin_position: &MarginPositionV2,
    debt_asset_is_asset0: bool,
) -> Result<u128> {
    if debt_asset_is_asset0 {
        margin_position.fixed_debt0(&market.debt_book)
    } else {
        margin_position.fixed_debt1(&market.debt_book)
    }
}

fn position_collateral(margin_position: &MarginPositionV2, debt_asset_is_asset0: bool) -> u64 {
    if debt_asset_is_asset0 {
        margin_position.collateral1
    } else {
        margin_position.collateral0
    }
}

fn collateral_to_seize(repay_credit: u64, collateral_before: u64) -> Result<u64> {
    let seizure = ceil_div(
        (repay_credit as u128)
            .checked_mul((BPS_DENOMINATOR + LIQUIDATION_INCENTIVE_BPS) as u128)
            .ok_or(ErrorCode::MarketMathOverflowV2)?,
        BPS_DENOMINATOR as u128,
    )
    .ok_or(ErrorCode::MarketMathOverflowV2)?;
    let seizure = u64::try_from(seizure).map_err(|_| ErrorCode::MarketMathOverflowV2)?;
    Ok(seizure.min(collateral_before))
}

fn shares_to_burn_for_reduction(
    debt_reduction: u128,
    debt_before: u128,
    shares_before: u128,
    borrow_index_nad: u128,
) -> Result<u128> {
    require!(
        shares_before > 0 && debt_before > 0,
        ErrorCode::InsufficientDebt
    );
    if debt_reduction >= debt_before {
        return Ok(shares_before);
    }
    let debt_reduction =
        u64::try_from(debt_reduction).map_err(|_| ErrorCode::MarketMathOverflowV2)?;
    DebtBookV2::debt_to_shares(debt_reduction, borrow_index_nad)
        .map(|shares| shares.min(shares_before))
}

fn recognized_decrease_after_seizure(
    recognized_before: u64,
    collateral_after: u64,
    shares_to_burn: u128,
    shares_before: u128,
) -> Result<u64> {
    if shares_to_burn == shares_before {
        return Ok(recognized_before);
    }
    let proportional = (recognized_before as u128)
        .checked_mul(shares_to_burn)
        .and_then(|value| value.checked_div(shares_before))
        .ok_or(ErrorCode::MarketMathOverflowV2)?;
    let proportional = u64::try_from(proportional).map_err(|_| ErrorCode::MarketMathOverflowV2)?;
    let recognized_after_proportional = recognized_before
        .checked_sub(proportional)
        .ok_or(ErrorCode::MarketMathOverflowV2)?;
    if recognized_after_proportional <= collateral_after {
        Ok(proportional)
    } else {
        let extra = recognized_after_proportional
            .checked_sub(collateral_after)
            .ok_or(ErrorCode::MarketMathOverflowV2)?;
        proportional
            .checked_add(extra)
            .ok_or(ErrorCode::MarketMathOverflowV2.into())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::{BufferBookV2, MarketConfigV2, MarketSideV2, ReserveLedgerV2};

    fn market_side(asset_mint: Pubkey) -> MarketSideV2 {
        MarketSideV2 {
            asset_mint,
            claim_mint: Pubkey::new_unique(),
            hedge_mint: Pubkey::new_unique(),
            hedge_vault: Pubkey::new_unique(),
            reserve_vault: Pubkey::new_unique(),
            collateral_vault: Pubkey::new_unique(),
            fee_vault: Pubkey::new_unique(),
            stake_vault: Pubkey::new_unique(),
            reserve_ledger: ReserveLedgerV2 {
                live_reserve: 1_000,
                cash_reserve: 1_000,
                reserved_liability: 0,
            },
            buffer_book: BufferBookV2 {
                buffer_ratio_bps: 2_000,
                ..BufferBookV2::default()
            },
            ..MarketSideV2::default()
        }
    }

    fn test_market() -> MarketV2 {
        let asset0_mint = Pubkey::new_unique();
        let asset1_mint = Pubkey::new_unique();
        let mut market = MarketV2::initialize(
            asset0_mint,
            asset1_mint,
            Pubkey::new_unique(),
            Pubkey::new_unique(),
            market_side(asset0_mint),
            market_side(asset1_mint),
            MarketConfigV2 {
                swap_fee_bps: 30,
                operator_fee_bps: 1_000,
                buffer_ratio_bps: 2_000,
                fee_routing_k_nad: NAD,
                ema_half_life_ms: 60_000,
                directional_ema_half_life_ms: 60_000,
                k_ema_half_life_ms: 60_000,
                max_daily_borrow_bps: 2_000,
                max_daily_withdraw_bps: 2_000,
                spot_ema_divergence_bps: 1_000,
                recognized_collateral_cap_bps: 10_000,
                market_health_min_bps: 11_000,
                effective_debt_weight_min_bps: 10_000,
                effective_debt_gamma_nad: NAD,
                soft_borrow_enabled: false,
                hedged_lp_enabled: true,
                start_time: 0,
            },
            [9_u8; 32],
            42,
            253,
        )
        .unwrap();
        market.insurance_reserve.available0 = 40;
        market.insurance_reserve.available1 = 40;
        market
    }

    fn insolvent_position(market: &mut MarketV2) -> MarginPositionV2 {
        let debt_shares = DebtBookV2::debt_to_shares(100, market.debt_book.borrow_index0_nad)
            .unwrap();
        market.debt_book.fixed_debt0_shares = debt_shares;
        market.recognition_ledger.debt_bearing_collateral1_for_debt0 = 50;

        MarginPositionV2 {
            owner: Pubkey::new_unique(),
            market: Pubkey::new_unique(),
            collateral0: 0,
            collateral1: 50,
            recognized_collateral0_for_debt1: 0,
            recognized_collateral1_for_debt0: 50,
            fixed_debt0_shares: debt_shares,
            fixed_debt1_shares: 0,
            bump: 1,
        }
    }

    #[test]
    fn insurance_request_starts_after_collateral_is_exhausted() {
        let mut market = test_market();
        let position = insolvent_position(&mut market);

        let partial_request =
            insurance_request_for_liquidation(&market, &position, true, 25, 30).unwrap();
        assert_eq!(partial_request, 0);

        let exhausted_request =
            insurance_request_for_liquidation(&market, &position, true, 50, 30).unwrap();
        assert_eq!(exhausted_request, 30);
    }

    #[test]
    fn liquidation_uses_repay_insurance_then_socialization() {
        let mut market = test_market();
        let mut position = insolvent_position(&mut market);

        let outcome =
            apply_liquidation_state(&mut market, &mut position, true, 50, 30, 30, 20).unwrap();

        assert_eq!(outcome.repaid_amount, 50);
        assert_eq!(outcome.collateral_seized, 50);
        assert_eq!(outcome.insurance_drawn, 30);
        assert_eq!(outcome.socialized_loss, 20);
        assert_eq!(outcome.remaining_debt, 0);
        assert_eq!(position.collateral1, 0);
        assert_eq!(position.recognized_collateral1_for_debt0, 0);
        assert_eq!(position.fixed_debt0_shares, 0);
        assert_eq!(market.debt_book.fixed_debt0_shares, 0);
        assert_eq!(market.insurance_reserve.available0, 10);
        assert_eq!(market.side0.reserve_ledger.live_reserve, 1_080);
        assert_eq!(market.side0.reserve_ledger.cash_reserve, 1_080);
    }

    #[test]
    fn liquidation_rejects_socialization_above_caller_cap() {
        let mut market = test_market();
        let mut position = insolvent_position(&mut market);

        let err = apply_liquidation_state(&mut market, &mut position, true, 50, 30, 30, 19)
            .unwrap_err();

        assert_eq!(
            err,
            error!(ErrorCode::LiquidationSocializationExceededV2)
        );
    }

    #[test]
    fn recognized_decrease_never_exceeds_remaining_collateral() {
        let decrease = recognized_decrease_after_seizure(80, 25, 250, 1_000).unwrap();

        assert_eq!(decrease, 55);
        assert_eq!(80 - decrease, 25);
    }
}
