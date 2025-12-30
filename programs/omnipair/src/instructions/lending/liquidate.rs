use anchor_lang::prelude::*;
use anchor_spl::{
    token::Token,
    token_interface::{TokenAccount, Mint, Token2022},
};
use crate::{
    state::pair::Pair,
    state::rate_model::RateModel,
    state::futarchy_authority::FutarchyAuthority,
    constants::*,
    errors::ErrorCode,
    events::{UserPositionLiquidatedEvent, EventMetadata},
    state::user_position::UserPosition,
    utils::token::transfer_from_pool_vault_to_user,
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
        seeds = [FUTARCHY_AUTHORITY_SEED_PREFIX],
        bump
    )]
    pub futarchy_authority: Account<'info, FutarchyAuthority>,

    #[account(
        mut,
        constraint = collateral_vault.mint == pair.token0 || collateral_vault.mint == pair.token1,
        constraint = collateral_vault.owner == pair.key() @ ErrorCode::InvalidVaultIn,
    )]
    pub collateral_vault: Box<InterfaceAccount<'info, TokenAccount>>,

    #[account(
        mut,
        constraint = caller_token_account.mint == collateral_vault.mint,
    )]
    pub caller_token_account: Box<InterfaceAccount<'info, TokenAccount>>,

    #[account(address = collateral_vault.mint)]
    pub collateral_token_mint: Box<InterfaceAccount<'info, Mint>>,

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
            position_owner,
            payer,
            user_position,
            token_program,
            token_2022_program,
            ..
        } = ctx.accounts;
        let pair = &mut ctx.accounts.pair;
        let collateral_token = collateral_vault.mint;
        let debt_token = if collateral_token == pair.token0 { pair.token1 } else { pair.token0 };
        let is_collateral_token0 = collateral_token == pair.token0;
        let liquidation_cf_bps = user_position.get_liquidation_cf_bps(pair, &debt_token)?;
        let k0 = pair.k(); // k before liquidation

        // Compute debt

        let (user_debt, collateral_amount, collateral_price_nad, reserve_amount) = match is_collateral_token0 {
            true => (
                // collateral is token0, debt is token1
                user_position.calculate_debt1(pair.total_debt1, pair.total_debt1_shares)?,
                user_position.collateral0 as u128, 
                pair.ema_price0_nad() as u128,
                // if collateral is token0, then we need reserve1 (debt token reserve)
                pair.reserve1 as u64,
            ),
            false => (
                // collateral is token1, debt is token0
                user_position.calculate_debt0(pair.total_debt0, pair.total_debt0_shares)?, 
                user_position.collateral1 as u128, 
                pair.ema_price1_nad() as u128,
                // if collateral is token1, then we need reserve0 (debt token reserve)
                pair.reserve0 as u64,
            ),
        };

        let collateral_value = collateral_amount
        .checked_mul(collateral_price_nad).ok_or(ErrorCode::DebtMathOverflow)?
        .checked_div(NAD as u128).ok_or(ErrorCode::DebtMathOverflow)?;

        // Compute borrow limit using fixed liquidation CF
        let borrow_limit = collateral_value
        .checked_mul(liquidation_cf_bps as u128).ok_or(ErrorCode::DebtMathOverflow)?
        .checked_div(BPS_DENOMINATOR as u128).ok_or(ErrorCode::DebtMathOverflow)?;

        // Check if position is undercollateralized
        require_gte!(user_debt as u128, borrow_limit, ErrorCode::NotUndercollateralized);

        // Health Factor (HF) < 1: undercollateralized (liquidatable)
        // collateral_value > user_debt > borrow_limit: position can be liquidated partially
        // user_debt > collateral_value: insolvent (bad debt, collateral can't cover principal)
        // If insolvent, repay all debt; else, repay a portion using close factor (partial liquidation)
        let is_insolvent = user_debt as u128 > collateral_value;
        // made it mutable to allow for custom debt repayment logic in case of debt token reserve is not enough
        let mut debt_to_repay: u64 = if is_insolvent {
            user_debt
        } else {
            let partial = (user_debt as u128)
                .checked_mul(CLOSE_FACTOR_BPS as u128).ok_or(ErrorCode::DebtMathOverflow)?
                .checked_div(BPS_DENOMINATOR as u128).ok_or(ErrorCode::DebtMathOverflow)?;
            core::cmp::min(user_debt, partial as u64)
        };

        let is_debt_token_reserve_not_enough = debt_to_repay > reserve_amount;
        // if debt token reserve is not enough, then we use half of the reserve to repay the debt, to prevent zero reserve
        debt_to_repay = match is_debt_token_reserve_not_enough {
            true => (reserve_amount) / 2,
            false => debt_to_repay
        };

        // collateral_amount_to_seize = debt_to_repay * NAD / collateral_price
        let collateral_amount_to_seize = (debt_to_repay as u128)
        .checked_mul(NAD as u128).ok_or(ErrorCode::DebtMathOverflow)?
        .checked_div(collateral_price_nad).ok_or(ErrorCode::DebtMathOverflow)?;

        let collateral_amount_to_seize_u64: u64 = collateral_amount_to_seize
            .try_into()
            .map_err(|_| ErrorCode::DebtMathOverflow)?;

        // Clamp to what user actually has
        let collateral_final = core::cmp::min(collateral_amount_to_seize_u64,
            if is_collateral_token0 { user_position.collateral0 } else { user_position.collateral1 }
        );

        let collateral_token = pair.get_collateral_token(&debt_token);
        let collateral_amount_pre_liquidation = match collateral_token == pair.token0 {
            true => user_position.collateral0,
            false => user_position.collateral1,
        };

        let collateral_amount_post_liquidation = collateral_amount_pre_liquidation
            .checked_sub(collateral_final)
            .ok_or(ErrorCode::DebtMathOverflow)?;
        let applied_min_cf_bps = pair.get_max_debt_and_cf_bps_for_collateral(&pair, &collateral_token, collateral_amount_post_liquidation)?.1;

        let caller_incentive = (collateral_final as u128)
            .checked_mul(LIQUIDATION_INCENTIVE_BPS as u128).ok_or(ErrorCode::DebtMathOverflow)?
            .checked_div(BPS_DENOMINATOR as u128).ok_or(ErrorCode::DebtMathOverflow)?.try_into().map_err(|_| ErrorCode::DebtMathOverflow)?;
        
        // Remaining collateral goes to reserves (after incentive)
        let collateral_to_reserves = collateral_final
            .checked_sub(caller_incentive)
            .ok_or(ErrorCode::DebtMathOverflow)?;

        if is_insolvent && !is_debt_token_reserve_not_enough {
            user_position.writeoff_debt(pair, &debt_token)?;
        } else {
            user_position.decrease_debt(pair, &debt_token, debt_to_repay)?;
        }
        user_position.set_applied_min_cf_for_debt_token(&debt_token, &pair, applied_min_cf_bps);

        // Transfer liquidation incentive to caller from collateral vault
        if caller_incentive > 0 {
            transfer_from_pool_vault_to_user(
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

        // Update user position collateral and pair reserves
        // Subtract the full seized amount from user position
        match is_collateral_token0 {
            true => {
                user_position.collateral0 = user_position.collateral0.checked_sub(collateral_final).unwrap();
                pair.total_collateral0 = pair.total_collateral0.checked_sub(collateral_final).unwrap();
                // Add remaining collateral (after incentive) to reserves
                pair.reserve0 = pair.reserve0.checked_add(collateral_to_reserves).unwrap();
                pair.reserve1 = pair.reserve1.saturating_sub(debt_to_repay);
            }
            false => {
                user_position.collateral1 = user_position.collateral1.checked_sub(collateral_final).unwrap();
                pair.total_collateral1 = pair.total_collateral1.checked_sub(collateral_final).unwrap();
                // Add remaining collateral (after incentive) to reserves
                pair.reserve1 = pair.reserve1.checked_add(collateral_to_reserves).unwrap();
                pair.reserve0 = pair.reserve0.saturating_sub(debt_to_repay);
            }
        }

        emit_cpi!(UserPositionLiquidatedEvent {
            metadata: EventMetadata::new(position_owner.key(), pair.key()),
            position: user_position.key(),
            liquidator: payer.key(),
            collateral0_liquidated: if is_collateral_token0 { 0 } else { collateral_final },
            collateral1_liquidated: if is_collateral_token0 { collateral_final } else { 0 },
            debt0_liquidated: if is_collateral_token0 { 0 } else { debt_to_repay },
            debt1_liquidated: if is_collateral_token0 { debt_to_repay } else { 0 },
            collateral_price: if is_collateral_token0 { pair.ema_price0_nad() } else { pair.ema_price1_nad() },
            // needs review after adding not enough debt token reserve case
            shortfall: if is_insolvent { (user_debt as u128).checked_sub(collateral_value).unwrap() } else { 0 },
            liquidation_bonus_applied: caller_incentive,
            k0: k0,
            k1: pair.k(),
        });

        Ok(())
    }
}
