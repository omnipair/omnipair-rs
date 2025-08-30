use anchor_lang::prelude::*;
use anchor_spl::token_interface::TokenAccount;
use crate::{
    state::pair::Pair,
    state::rate_model::RateModel,
    constants::*,
    errors::ErrorCode,
    events::UserPositionLiquidatedEvent,
    state::user_position::UserPosition,
};

#[event_cpi]
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
            position_owner.key().as_ref()
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
        constraint = collateral_vault.mint == pair.token0 || collateral_vault.mint == pair.token1,
    )]
    pub collateral_vault: Box<InterfaceAccount<'info, TokenAccount>>,

    /// CHECK: This is the owner of the position being liquidated.
    #[account(mut)]
    pub position_owner: AccountInfo<'info>,
    #[account(mut)]
    pub payer: Signer<'info>,
    pub system_program: Program<'info, System>,
}

impl<'info> Liquidate<'info> {
    pub fn validate(&self) -> Result<()> {
        let user_position = &self.user_position;

        require!(user_position.is_initialized(), ErrorCode::UserPositionNotInitialized);
        
        // Check if user has enough debt
        match self.collateral_vault.mint == self.pair.token0 {
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
        self.pair.update(&self.rate_model)?;
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
            position_owner,
            payer,
            user_position,
            ..
        } = ctx.accounts;
        let pair = &mut ctx.accounts.pair;
        let collateral_token = collateral_vault.mint;
        let debt_token = if collateral_token == pair.token0 { pair.token1 } else { pair.token0 };
        let is_collateral_token0 = collateral_token == pair.token0;
        let fixed_cf_bps = user_position.get_liquidation_cf_bps(pair, &debt_token);
        let k0 = pair.k(); // k before liquidation

        // Compute debt

        let (user_debt, collateral_amount, collateral_price_nad) = match is_collateral_token0 {
            true => (
                // collateral is token0, debt is token1
                user_position.calculate_debt1(pair.total_debt1, pair.total_debt1_shares)?,
                user_position.collateral0 as u128, 
                pair.ema_price0_nad() as u128
            ),
            false => (
                // collateral is token1, debt is token0
                user_position.calculate_debt0(pair.total_debt0, pair.total_debt0_shares)?, 
                user_position.collateral1 as u128, 
                pair.ema_price1_nad() as u128
            ),
        };

        let collateral_value = collateral_amount
        .checked_mul(collateral_price_nad).ok_or(ErrorCode::DebtMathOverflow)?
        .checked_div(NAD as u128).ok_or(ErrorCode::DebtMathOverflow)?;

        // Compute borrow limit using fixed liquidation CF
        let borrow_limit = collateral_value
        .checked_mul(fixed_cf_bps as u128).ok_or(ErrorCode::DebtMathOverflow)?
        .checked_div(BPS_DENOMINATOR as u128).ok_or(ErrorCode::DebtMathOverflow)?;

        // Check if position is undercollateralized
        require_gte!(user_debt as u128, borrow_limit, ErrorCode::NotUndercollateralized);


        // apply close factor 
        let debt_to_repay = (user_debt as u128)
        .checked_mul(CLOSE_FACTOR_BPS as u128).ok_or(ErrorCode::DebtMathOverflow)?
        .checked_div(BPS_DENOMINATOR as u128).ok_or(ErrorCode::DebtMathOverflow)?;
        let debt_to_repay: u64 = core::cmp::min(user_debt, debt_to_repay as u64);

        // collateral_amount_to_seize = debt_to_repay * NAD / collateral_price
        let collateral_amount_to_seize = (debt_to_repay as u128)
        .checked_mul(NAD as u128).ok_or(ErrorCode::DebtMathOverflow)?
        .checked_div(collateral_price_nad).ok_or(ErrorCode::DebtMathOverflow)?;

        let collateral_amount_to_seize_u64: u64 = collateral_amount_to_seize
            .try_into()
            .map_err(|_| ErrorCode::DebtMathOverflow)?;

        let applied_min_cf_bps = user_position.get_user_pessimistic_collateral_factor_bps(&pair, &debt_token);

        // Clamp to what user actually has
        let collateral_final = core::cmp::min(collateral_amount_to_seize_u64,
            if is_collateral_token0 { user_position.collateral0 } else { user_position.collateral1 }
        );

        user_position.decrease_debt(pair, &debt_token, debt_to_repay)?;
        user_position.set_applied_min_cf_for_debt_token(&debt_token, &pair, applied_min_cf_bps);

        // LP seize collateral
        // Liquidation incentive is shared across LPs with no caller incentive
        // No actual transfer of collateral is done here, just increasing reserves
        match is_collateral_token0 {
            true => {
                user_position.collateral0 = user_position.collateral0.checked_sub(collateral_final).unwrap();
                pair.reserve0 = pair.reserve0.checked_add(collateral_final).unwrap();
                pair.reserve1 = pair.reserve1.checked_sub(debt_to_repay).unwrap();
            }
            false => {
                user_position.collateral1 = user_position.collateral1.checked_sub(collateral_final).unwrap();
                pair.reserve1 = pair.reserve1.checked_add(collateral_final).unwrap();
                pair.reserve0 = pair.reserve0.checked_sub(debt_to_repay).unwrap();
            }
        }

        emit_cpi!(UserPositionLiquidatedEvent {
            user: position_owner.key(),
            pair: pair.key(),
            position: user_position.key(),
            liquidator: payer.key(),
            collateral0_liquidated: if is_collateral_token0 { 0 } else { user_position.collateral1 },
            collateral1_liquidated: if is_collateral_token0 { user_position.collateral0 } else { 0 },
            debt0_liquidated: if is_collateral_token0 { user_debt } else { 0 },
            debt1_liquidated: if is_collateral_token0 { 0 } else { user_debt },
            collateral_price: if is_collateral_token0 { pair.ema_price0_nad() } else { pair.ema_price1_nad() },
            liquidation_bonus_applied: 0,
            k0: k0,
            k1: pair.k(),
            timestamp: Clock::get()?.unix_timestamp,
        });

        Ok(())
    }

    // Calculates how much debt should be repaid (and how much collateral seized)
    // to bring a position back to health (HF ≥ 1).
    // Uses: debt, borrow_power, liquidation_lp_incentive_bps
    // pub fn calculate_partial_liquidation_amount(
    //     debt: u64,
    //     borrow_power: u64,
    //     liquidation_lp_incentive_bps: u64,
    // ) -> (u64, u64, u64) {

    //     (overexposed, collateral_seize_with_incentive, incentive_amount)
    // }
}
