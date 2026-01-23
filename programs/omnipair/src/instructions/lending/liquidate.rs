use anchor_lang::prelude::*;
use std::cmp::min;
use anchor_spl::{
    token::{Token, TokenAccount, Mint},
    token_interface::{Token2022},
};
use crate::{
    state::pair::Pair,
    state::rate_model::RateModel,
    state::futarchy_authority::FutarchyAuthority,
    constants::*,
    errors::ErrorCode,
    events::{UserPositionLiquidatedEvent, EventMetadata},
    state::user_position::{UserPosition, DebtDecreaseReason},
    utils::{
        token::{transfer_from_vault_to_user, transfer_from_vault_to_vault}, 
        math::ceil_div,
        gamm_math::{CPCurve, construct_virtual_reserves_at_pessimistic_price},
    },
    generate_gamm_pair_seeds,
};

#[event_cpi]
#[derive(Accounts)]
pub struct Liquidate<'info> {
    #[account(
        mut,
        seeds = [
            PAIR_SEED_PREFIX,
            pair.token0.as_ref(),
            pair.token1.as_ref(),
            pair.params_hash.as_ref()
        ],
        bump = pair.bump
    )]
    pub pair: Account<'info, Pair>,

    #[account(
        mut,
        seeds = [
            POSITION_SEED_PREFIX,
            pair.key().as_ref(),
            position_owner.key().as_ref()
        ],
        bump = user_position.bump
    )]
    pub user_position: Account<'info, UserPosition>,

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

    #[account(
        mut,
        seeds = [
            COLLATERAL_VAULT_SEED_PREFIX,
            pair.key().as_ref(),
            collateral_token_mint.key().as_ref(),
        ],
        bump = pair.get_collateral_vault_bump(&collateral_token_mint.key())
    )]
    pub collateral_vault: Box<Account<'info, TokenAccount>>,

    #[account(
        mut,
        constraint = caller_token_account.mint == collateral_vault.mint,
    )]
    pub caller_token_account: Box<Account<'info, TokenAccount>>,

    #[account(
        constraint = collateral_token_mint.key() == pair.token0 || collateral_token_mint.key() == pair.token1 @ ErrorCode::InvalidVault
    )]
    pub collateral_token_mint: Box<Account<'info, Mint>>,
    
    #[account(
        mut,
        seeds = [
            RESERVE_VAULT_SEED_PREFIX,
            pair.key().as_ref(),
            collateral_token_mint.key().as_ref(),
        ],
        bump = pair.get_reserve_vault_bump(&collateral_token_mint.key())
    )]
    pub reserve_vault: Box<Account<'info, TokenAccount>>,

    /// CHECK: This is the owner of the position being liquidated.
    #[account(address = user_position.owner)]
    pub position_owner: AccountInfo<'info>,
    #[account(mut)]
    pub payer: Signer<'info>,
    pub token_program: Program<'info, Token>,
    pub token_2022_program: Program<'info, Token2022>,
    pub system_program: Program<'info, System>,
}

impl<'info> Liquidate<'info> {
    pub fn validate(&self) -> Result<()> {
        let user_position = &self.user_position;

        require!(user_position.is_initialized(), ErrorCode::UserPositionNotInitialized);
        
        // Check if user has enough debt
        match self.collateral_token_mint.key() == self.pair.token0 {
            true => require_gt!(
                user_position.debt1_shares,
                0,
                ErrorCode::ZeroDebtAmount
            ),
            false => require_gt!(
                user_position.debt0_shares,
                0,
                ErrorCode::ZeroDebtAmount
            ),
        }
        
        Ok(())
    }

    pub fn update(&mut self) -> Result<()> {
        let pair_key = self.pair.to_account_info().key();
        self.pair.update(&self.rate_model, &self.futarchy_authority, pair_key)?;
        Ok(())
    }

    pub fn update_and_validate_liquidate(&mut self) -> Result<()> {
        self.update()?;
        self.validate()?;
        Ok(())
    }

    pub fn handle_liquidate(ctx: Context<Self>) -> Result<()> {
        let Liquidate {
            collateral_vault,
            caller_token_account,
            collateral_token_mint,
            reserve_vault,
            position_owner,
            payer,
            user_position,
            token_program,
            token_2022_program,
            ..
        } = ctx.accounts;
        let pair = &mut ctx.accounts.pair;
        
        // Validate collateral vault and pool vault - already validated by Anchor seeds
        require_keys_eq!(
            collateral_vault.mint,
            collateral_token_mint.key(),
            ErrorCode::InvalidVault
        );
        require_keys_eq!(
            reserve_vault.mint,
            collateral_token_mint.key(),
            ErrorCode::InvalidVault
        );
        require_keys_eq!(
            reserve_vault.owner,
            pair.key(),
            ErrorCode::InvalidVault
        );

        let collateral_token = collateral_token_mint.key();
        let debt_token = if collateral_token == pair.token0 { pair.token1 } else { pair.token0 };
        let is_collateral_token0 = collateral_token == pair.token0;
        let liquidation_cf_bps = user_position.get_liquidation_cf_bps(pair, &debt_token)?;
        let k0 = pair.k(); // k before liquidation

        // Compute debt
        let (
            user_debt, 
            debt_reserve, 
            user_collateral, 
            collateral_reserve, 
            collateral_ema_nad
        )  = match is_collateral_token0 {
            true => (
                // collateral is token0, debt is token1
                user_position.calculate_debt1(pair.total_debt1, pair.total_debt1_shares)?,
                pair.reserve1, 
                user_position.collateral0, 
                pair.reserve0, 
                pair.ema_price0_nad()
            ),
            false => (
                // collateral is token1, debt is token0
                user_position.calculate_debt0(pair.total_debt0, pair.total_debt0_shares)?,
                pair.reserve0,
                user_position.collateral1, 
                pair.reserve1, 
                pair.ema_price1_nad()
            ),
        };

        // Construct virtual reserves at pessimistic price
        let (collateral_ema_reserve, debt_ema_reserve) = construct_virtual_reserves_at_pessimistic_price(
            collateral_reserve, debt_reserve, collateral_ema_nad, collateral_ema_nad
        )?;

        // Collateral value with impact: debt coverable by selling all collateral
        let collateral_value_with_impact = CPCurve::calculate_amount_out(collateral_ema_reserve, debt_ema_reserve, user_collateral)?;
        
        // Borrow limit = collateral_value * liquidation_cf
        let borrow_limit = (collateral_value_with_impact as u128)
            .checked_mul(liquidation_cf_bps as u128).ok_or(ErrorCode::DebtMathOverflow)?
            .checked_div(BPS_DENOMINATOR as u128).ok_or(ErrorCode::DebtMathOverflow)?;

        // Position is liquidatable if debt >= borrow_limit
        require_gte!(user_debt as u128, borrow_limit, ErrorCode::NotUndercollateralized);
 
        // Health Factor (HF) < 1: undercollateralized (liquidatable)
        // collateral_value > user_debt > borrow_limit: position can be liquidated partially
        // user_debt > collateral_value: insolvent (bad debt, collateral can't cover principal)
        // If insolvent, writeoff all debt; else, writeoff a portion using close factor (partial liquidation)
        let is_insolvent = user_debt > collateral_value_with_impact;

        // Get user's debt shares for the debt token
        let (user_debt_shares, total_debt, total_debt_shares) = match is_collateral_token0 {
            true => (user_position.debt1_shares, pair.total_debt1, pair.total_debt1_shares),
            false => (user_position.debt0_shares, pair.total_debt0, pair.total_debt0_shares),
        };

        // Calculate shares to writeoff first
        // For partial liquidation: ceil(user_debt_shares * CLOSE_FACTOR_BPS / BPS_DENOMINATOR)
        // For insolvent positions: all user debt shares
        let shares_to_writeoff: u128 = match is_insolvent {
            true => user_debt_shares,
            false => {
                // ceiled division to avoid edge case where small shares never get fully written off
                let partial_shares = ceil_div(
                    user_debt_shares
                        .checked_mul(CLOSE_FACTOR_BPS as u128).ok_or(ErrorCode::DebtMathOverflow)?,
                    BPS_DENOMINATOR as u128
                ).ok_or(ErrorCode::DebtMathOverflow)?;
                min(user_debt_shares, partial_shares) // clamped to user's shares
            }
        };

        let debt_to_writeoff: u64 = match total_debt_shares == 0 {
            true => 0,
            false => {
                let debt = ceil_div(
                    shares_to_writeoff
                        .checked_mul(total_debt as u128).ok_or(ErrorCode::DebtMathOverflow)?,
                    total_debt_shares
                ).ok_or(ErrorCode::DebtMathOverflow)?;
                min(user_debt, debt as u64) // clamped to user's debt
            }
        };
        
        // Calculate base collateral to seize with price impact: Δx = Δy * x / (y - Δy)
        let collateral_base = CPCurve::calculate_amount_in(collateral_ema_reserve, debt_ema_reserve, debt_to_writeoff)?;

        // Add liquidation penalty on top (paid by borrower, benefits LPs)
        // total_seized = base * (1 + LIQUIDATION_PENALTY_BPS / BPS)
        let collateral_with_penalty = ceil_div(
            (collateral_base as u128)
                .checked_mul((BPS_DENOMINATOR + LIQUIDATION_PENALTY_BPS) as u128)
                .ok_or(ErrorCode::DebtMathOverflow)?,
            BPS_DENOMINATOR as u128
        ).ok_or(ErrorCode::DebtMathOverflow)?;

        // Clamp to what user actually has
        let collateral_final: u64 = min(collateral_with_penalty, user_collateral as u128)
            .try_into().map_err(|_| ErrorCode::DebtMathOverflow)?;

        let collateral_token = pair.get_collateral_token(&debt_token);
        let collateral_amount_pre_liquidation = match collateral_token == pair.token0 {
            true => user_position.collateral0,
            false => user_position.collateral1,
        };

        let collateral_amount_post_liquidation = collateral_amount_pre_liquidation
            .checked_sub(collateral_final)
            .ok_or(ErrorCode::DebtMathOverflow)?;
        let (_, _, liquidation_cf_bps) = pair.get_max_debt_and_cf_bps_for_collateral(&pair, &collateral_token, collateral_amount_post_liquidation)?;

        // Liquidator incentive from base amount (not from penalty)
        let caller_incentive: u64 = min(
            (collateral_base as u128)
                .checked_mul(LIQUIDATION_INCENTIVE_BPS as u128).ok_or(ErrorCode::DebtMathOverflow)?
                .checked_div(BPS_DENOMINATOR as u128).ok_or(ErrorCode::DebtMathOverflow)?
                .try_into().map_err(|_| ErrorCode::DebtMathOverflow)?,
            collateral_final
        );
        
        // Remaining collateral goes to reserves (LPs get base + penalty - incentive)
        let collateral_to_reserves = collateral_final
            .checked_sub(caller_incentive)
            .ok_or(ErrorCode::DebtMathOverflow)?;


        // Pass exact shares to writeoff to avoid edge cases where floor division leaves residual shares
        user_position.decrease_debt(pair, &debt_token, debt_to_writeoff, DebtDecreaseReason::WriteOff(shares_to_writeoff))?;
        user_position.set_applied_min_cf_for_debt_token(&debt_token, &pair, liquidation_cf_bps);

        // Transfer liquidation incentive to caller from collateral vault
        if caller_incentive > 0 {
            transfer_from_vault_to_user(
                pair.to_account_info(),
                collateral_vault.to_account_info(),
                caller_token_account.to_account_info(),
                collateral_token_mint.to_account_info(),
                match collateral_token_mint.to_account_info().owner == token_program.key {
                    true => token_program.to_account_info(),
                    false => token_2022_program.to_account_info(),
                },
                caller_incentive,
                collateral_token_mint.decimals,
                &[&generate_gamm_pair_seeds!(pair)[..]],
            )?;
        }

        // Transfer remaining collateral from collateral vault to reserve vault
        transfer_from_vault_to_vault(
            pair.to_account_info(),
            collateral_vault.to_account_info(),
            reserve_vault.to_account_info(),
            collateral_token_mint.to_account_info(),
            match collateral_token_mint.to_account_info().owner == token_program.key {
                true => token_program.to_account_info(),
                false => token_2022_program.to_account_info(),
            },
            collateral_to_reserves,
            collateral_token_mint.decimals,
            &[&generate_gamm_pair_seeds!(pair)[..]],
        )?;

        // Update user position collateral and pair reserves
        // Subtract the full seized amount from user position
        match is_collateral_token0 {
            true => {
                user_position.collateral0 = user_position.collateral0.checked_sub(collateral_final).unwrap();
                pair.total_collateral0 = pair.total_collateral0.checked_sub(collateral_final).unwrap();
                // Add remaining collateral (after incentive) to reserves
                pair.reserve0 = pair.reserve0.checked_add(collateral_to_reserves).unwrap();
                pair.cash_reserve0 = pair.cash_reserve0.saturating_add(collateral_to_reserves);
            }
            false => {
                user_position.collateral1 = user_position.collateral1.checked_sub(collateral_final).unwrap();
                pair.total_collateral1 = pair.total_collateral1.checked_sub(collateral_final).unwrap();
                // Add remaining collateral (after incentive) to reserves
                pair.reserve1 = pair.reserve1.checked_add(collateral_to_reserves).unwrap();
                pair.cash_reserve1 = pair.cash_reserve1.saturating_add(collateral_to_reserves);
            }
        }

        emit_cpi!(UserPositionLiquidatedEvent {
            metadata: EventMetadata::new(position_owner.key(), pair.key()),
            position: user_position.key(),
            liquidator: payer.key(),
            collateral0_liquidated: if is_collateral_token0 { 0 } else { collateral_final },
            collateral1_liquidated: if is_collateral_token0 { collateral_final } else { 0 },
            debt0_liquidated: if is_collateral_token0 { 0 } else { debt_to_writeoff },
            debt1_liquidated: if is_collateral_token0 { debt_to_writeoff } else { 0 },
            collateral_price: if is_collateral_token0 { pair.ema_price0_nad() } else { pair.ema_price1_nad() },
            shortfall: if is_insolvent { (user_debt as u128).saturating_sub(collateral_value_with_impact as u128) } else { 0 },
            liquidation_bonus_applied: caller_incentive,
            k0: k0,
            k1: pair.k(),
        });

        Ok(())
    }
}
