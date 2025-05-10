use anchor_lang::prelude::*;
use anchor_spl::{
    token::Token,
    token_interface::{Mint, TokenAccount, Token2022},
};
use crate::{
    state::pair::Pair,
    state::rate_model::RateModel,
    constants::*,
    errors::ErrorCode,
    events::UserPositionLiquidatedEvent,
    state::user_position::UserPosition,
};

#[derive(Accounts)]
pub struct Liquidate<'info> {
    #[account(
        mut,
        seeds = [
            PAIR_SEED_PREFIX,
            pair.token0.as_ref(),
            pair.token1.as_ref()
        ],
        bump
    )]
    pub pair: Account<'info, Pair>,

    #[account(
        mut,
        seeds = [
            POSITION_SEED_PREFIX,
            pair.key().as_ref(),
            user.key().as_ref()
        ],
        bump
    )]
    pub user_position: Account<'info, UserPosition>,

    #[account(
        mut,
        address = pair.rate_model,
    )]
    pub rate_model: Account<'info, RateModel>,

    #[account(
        mut,
        constraint = token_vault.mint == pair.token0 || token_vault.mint == pair.token1,
    )]
    pub token_vault: Box<InterfaceAccount<'info, TokenAccount>>,

    #[account(
        mut,
        constraint = user_token_account.mint == pair.token0 || user_token_account.mint == pair.token1,
        token::authority = user,
    )]
    pub user_token_account: Box<InterfaceAccount<'info, TokenAccount>>,

    #[account(address = token_vault.mint)]
    pub vault_token_mint: Box<InterfaceAccount<'info, Mint>>,

    #[account(mut)]
    pub user: Signer<'info>,
    pub token_program: Program<'info, Token>,
    pub token_2022_program: Program<'info, Token2022>,
    pub system_program: Program<'info, System>,
}

impl<'info> Liquidate<'info> {
    pub fn validate(&self) -> Result<()> {
        let user_position = &self.user_position;

        require!(user_position.is_initialized(), ErrorCode::UserPositionNotInitialized);
        
        // Check if user has enough debt
        match self.token_vault.mint == self.pair.token0 {
            true => require_gt!(
                user_position.debt0_shares,
                0,
                ErrorCode::ZeroDebtAmount
            ),
            false => require_gt!(
                user_position.debt1_shares,
                0,
                ErrorCode::ZeroDebtAmount
            ),
        }
        
        Ok(())
    }

    pub fn update(&mut self) -> Result<()> {
        self.pair.update(&self.rate_model)?;
        Ok(())
    }

    pub fn update_and_validate(&mut self) -> Result<()> {
        self.update()?;
        self.validate()?;
        Ok(())
    }

    pub fn handle_liquidate(ctx: Context<Self>) -> Result<()> {
        let Liquidate {
            user_token_account,
            token_vault,
            vault_token_mint,
            user,
            user_position,
            token_program,
            token_2022_program,
            ..
        } = ctx.accounts;
        let pair = &mut ctx.accounts.pair;

        // Compute debt
        let user_debt = if user_token_account.mint == pair.token0 {
            user_position.calculate_debt0(pair.total_debt0, pair.total_debt0_shares)
        } else {
            user_position.calculate_debt1(pair.total_debt1, pair.total_debt1_shares)
        };
        // Compute borrowing power
        let borrow_power = user_position.get_borrowing_power(&pair, &user_token_account.mint);

        // Compare debt to borrow power
        require_gte!(
            user_debt,
            borrow_power,
            ErrorCode::NotUndercollateralized
        );

        let (debt_to_writeoff, collateral_to_seize, incentive_applied) = Self::calculate_partial_liquidation_amount(
            user_debt,
            borrow_power,
            LIQUIDATION_LP_INCENTIVE_BPS,
        );

        // Skip if nothing needs to be repaid
        require!(debt_to_writeoff > 0 && collateral_to_seize > 0, ErrorCode::NotUndercollateralized);

        // Decrease debt
        user_position.decrease_debt(pair, &user_token_account.mint, debt_to_writeoff);

        // LP seize collateral
        // Liquidation incentive is shared across LPs with no caller incentive
        // No actual transfer of collateral is done here, just increasing reserves
        match user_token_account.mint == pair.token0 {
            true => {
                pair.reserve0 = pair.reserve0.checked_add(collateral_to_seize).unwrap();
            }
            false => {
                pair.reserve1 = pair.reserve1.checked_add(collateral_to_seize).unwrap();
            }
        }

        emit!(UserPositionLiquidatedEvent {
            user: user.key(),
            pair: pair.key(),
            position: user_position.key(),
            liquidator: user.key(),
            collateral0_liquidated: if user_token_account.mint == pair.token0 { 0 } else { collateral_to_seize },
            collateral1_liquidated: if user_token_account.mint == pair.token0 { collateral_to_seize } else { 0 },
            debt0_liquidated: if user_token_account.mint == pair.token0 { debt_to_writeoff } else { 0 },
            debt1_liquidated: if user_token_account.mint == pair.token0 { 0 } else { debt_to_writeoff },
            collateral_price: if user_token_account.mint == pair.token0 { pair.ema_price0_nad() } else { pair.ema_price1_nad() },
            liquidation_bonus_applied: incentive_applied,
            timestamp: Clock::get()?.unix_timestamp,
        });

        Ok(())
    }

    /// Calculates how much debt should be repaid (and how much collateral seized)
    /// to bring a position back to health (HF ≥ 1).
    /// Uses: debt, borrow_power, liquidation_lp_incentive_bps
    pub fn calculate_partial_liquidation_amount(
        debt: u64,
        borrow_power: u64,
        liquidation_lp_incentive_bps: u64,
    ) -> (u64, u64, u64) {
        if borrow_power >= debt {
            return (0, 0, 0); // no need to liquidate
        }
        let overexposed = debt - borrow_power;

        // amount of debt to write off = overexposed
        // amount of collateral to seize = writeoff * (1 + bonus)
        let collateral_seize_with_incentive = overexposed
            .saturating_mul(BPS_DENOMINATOR + liquidation_lp_incentive_bps)
            .checked_div(BPS_DENOMINATOR)
            .unwrap_or(0);
        let incentive_amount = collateral_seize_with_incentive.saturating_mul(liquidation_lp_incentive_bps).checked_div(BPS_DENOMINATOR).unwrap_or(0);

        (overexposed, collateral_seize_with_incentive, incentive_amount)
    }
}
