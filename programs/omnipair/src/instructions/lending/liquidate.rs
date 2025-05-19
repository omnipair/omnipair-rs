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

        // Compute debt
        let user_debt = match is_collateral_token0 {
            true => user_position.calculate_debt1(pair.total_debt1, pair.total_debt1_shares)?,
            false => user_position.calculate_debt0(pair.total_debt0, pair.total_debt0_shares)?,
        }; 

        // Compute borrowing power
        let borrow_limit = user_position.get_borrow_limit(&pair, &debt_token);

        // Compare debt to borrow power
        require_gte!(
            user_debt,
            borrow_limit,
            ErrorCode::NotUndercollateralized
        );

        // TODO: Implement partial liquidation
        // Full liquidation causes two unnecessary problems:
        // 1. Increases damage/loss to borrowers through enforced liquidation
        // 2. Increases the price impact on the liquidated token reserve
        // Need to think about how to incentivize partial liquidation, as the same position may be liquidated multiple times if necessary
        // a fixed percentage of debt can sometimes be insufficient as a liquidation incentive (i.e < gas)
        // a fixed amount of liquidation bond on the other hand will be consumed on the first liquidation, and will not be available for subsequent liquidations
        // with no way of dividing it for arbitrary number of liquidations
        user_position.decrease_debt(pair, &debt_token, user_debt)?;

        // LP seize collateral
        // Liquidation incentive is shared across LPs with no caller incentive
        // No actual transfer of collateral is done here, just increasing reserves
        match is_collateral_token0 {
            true => {
                pair.reserve0 = pair.reserve0.checked_add(user_position.collateral0).unwrap();
                user_position.collateral0 = 0;
            }
            false => {
                pair.reserve1 = pair.reserve1.checked_add(user_position.collateral1).unwrap();
                user_position.collateral1 = 0;
            }
        }

        emit!(UserPositionLiquidatedEvent {
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
