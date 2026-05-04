use anchor_lang::prelude::*;
use anchor_spl::{
    token::{Mint, Token, TokenAccount},
    token_interface::Token2022,
};
use std::cmp::min;

use crate::{
    constants::*,
    errors::ErrorCode,
    events::{EventMetadata, LeveragePositionLiquidatedEvent},
    generate_gamm_pair_seeds,
    state::{FutarchyAuthority, Pair, RateModel, UserLeveragePosition},
    utils::{
        math::ceil_div,
        gamm_math::CPCurve,
        token::{transfer_from_vault_to_user, transfer_from_vault_to_vault},
    },
};

use super::common::{equity_bps, quote_swap, token_program_for_mint};

#[derive(AnchorSerialize, AnchorDeserialize, Clone)]
pub struct LiquidateLeverageArgs {
    pub is_debt_token0: bool,
}

fn require_no_debtless_collateral_stranding(
    is_insolvent: bool,
    shares_to_writeoff: u128,
    position_debt_shares: u128,
    collateral_final: u64,
    position_collateral_amount: u64,
) -> Result<()> {
    require!(
        is_insolvent
            || shares_to_writeoff < position_debt_shares
            || collateral_final == position_collateral_amount,
        ErrorCode::LeverageLiquidationDust
    );
    Ok(())
}

fn liquidation_incentive(collateral_base: u64, collateral_final: u64) -> Result<u64> {
    let raw_incentive = (collateral_base as u128)
        .checked_mul(LIQUIDATION_INCENTIVE_BPS as u128)
        .ok_or(ErrorCode::DebtMathOverflow)?
        .checked_div(BPS_DENOMINATOR as u128)
        .ok_or(ErrorCode::DebtMathOverflow)? as u64;
    let max_incentive = collateral_final.saturating_sub(collateral_base);
    Ok(min(raw_incentive, max_incentive))
}

#[event_cpi]
#[derive(Accounts)]
#[instruction(args: LiquidateLeverageArgs)]
pub struct LiquidateLeverage<'info> {
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
    pub pair: Box<Account<'info, Pair>>,

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

    /// CHECK: Position owner receives remaining funds and closed account rent.
    #[account(mut, address = user_leverage_position.owner)]
    pub position_owner: AccountInfo<'info>,

    #[account(
        mut,
        seeds = [
            LEVERAGE_POSITION_SEED_PREFIX,
            pair.key().as_ref(),
            position_owner.key().as_ref(),
            &[args.is_debt_token0 as u8]
        ],
        bump = user_leverage_position.bump,
        constraint = user_leverage_position.pair == pair.key(),
        constraint = user_leverage_position.is_debt_token0 == args.is_debt_token0,
    )]
    pub user_leverage_position: Account<'info, UserLeveragePosition>,

    #[account(
        mut,
        seeds = [
            RESERVE_VAULT_SEED_PREFIX,
            pair.key().as_ref(),
            collateral_token_mint.key().as_ref(),
        ],
        bump = pair.get_reserve_vault_bump(&collateral_token_mint.key())
    )]
    pub collateral_token_vault: Box<Account<'info, TokenAccount>>,

    #[account(
        mut,
        seeds = [
            LEVERAGE_COLLATERAL_VAULT_SEED_PREFIX,
            pair.key().as_ref(),
            collateral_token_mint.key().as_ref(),
        ],
        bump,
        constraint = leverage_collateral_vault.mint == collateral_token_mint.key() @ ErrorCode::InvalidVault,
        constraint = leverage_collateral_vault.owner == pair.key() @ ErrorCode::InvalidVault
    )]
    pub leverage_collateral_vault: Box<Account<'info, TokenAccount>>,

    #[account(
        mut,
        constraint = liquidator_collateral_token_account.mint == collateral_token_mint.key() @ ErrorCode::InvalidTokenAccount,
        token::authority = liquidator,
    )]
    pub liquidator_collateral_token_account: Box<Account<'info, TokenAccount>>,

    #[account(
        constraint = collateral_token_mint.key() == pair.token0 || collateral_token_mint.key() == pair.token1 @ ErrorCode::InvalidMint
    )]
    pub collateral_token_mint: Box<Account<'info, Mint>>,

    #[account(
        constraint = debt_token_mint.key() == pair.token0 || debt_token_mint.key() == pair.token1 @ ErrorCode::InvalidMint
    )]
    pub debt_token_mint: Box<Account<'info, Mint>>,

    #[account(mut)]
    pub liquidator: Signer<'info>,
    pub token_program: Program<'info, Token>,
    pub token_2022_program: Program<'info, Token2022>,
    pub system_program: Program<'info, System>,
}

impl<'info> LiquidateLeverage<'info> {
    pub fn update_and_validate_liquidate_leverage(
        &mut self,
        args: &LiquidateLeverageArgs,
    ) -> Result<()> {
        let pair_key = self.pair.key();
        self.pair.update(
            &self.rate_model,
            &self.futarchy_authority,
            pair_key,
            Some(self.event_authority.to_account_info()),
        )?;

        let debt_token = if args.is_debt_token0 { self.pair.token0 } else { self.pair.token1 };
        let collateral_token = self.pair.get_token_y(&debt_token);
        require_keys_eq!(self.collateral_token_mint.key(), collateral_token, ErrorCode::InvalidMint);
        require_keys_eq!(self.debt_token_mint.key(), debt_token, ErrorCode::InvalidMint);
        require!(self.user_leverage_position.debt_shares > 0, ErrorCode::ZeroDebtAmount);
        require!(
            self.user_leverage_position.collateral_amount > 0,
            ErrorCode::InsufficientAmount
        );
        Ok(())
    }

    pub fn handle_liquidate_leverage(
        mut ctx: Context<'_, '_, '_, 'info, Self>,
        args: LiquidateLeverageArgs,
    ) -> Result<()> {
        let accounts = &mut ctx.accounts;
        let pair = &mut accounts.pair;
        let position = &mut accounts.user_leverage_position;
        let debt_amount = position.calculate_debt(pair)?;
        require_gt!(debt_amount, 0, ErrorCode::ZeroDebtAmount);

        let is_collateral_token0 = !args.is_debt_token0;
        let collateral_reserve = if is_collateral_token0 { pair.reserve0 } else { pair.reserve1 };
        let debt_reserve = if is_collateral_token0 { pair.reserve1 } else { pair.reserve0 };
        let quote = quote_swap(
            position.collateral_amount,
            collateral_reserve,
            debt_reserve,
            pair.swap_fee_bps,
            accounts.futarchy_authority.revenue_share.swap_bps,
        )?;
        let margin_bps = equity_bps(quote.amount_out, debt_amount)?;
        require!(
            quote.amount_out <= debt_amount
                || margin_bps <= LEVERAGE_MAINTENANCE_BUFFER_BPS as u128,
            ErrorCode::LeveragePositionNotLiquidatable
        );

        // Match normal lending: solvent liquidations use close factor, insolvent ones write off all shares.
        let is_insolvent = debt_amount > quote.amount_out;
        let shares_to_writeoff = match is_insolvent {
            true => position.debt_shares,
            false => min(
                position.debt_shares,
                ceil_div(
                    position
                        .debt_shares
                        .checked_mul(CLOSE_FACTOR_BPS as u128)
                        .ok_or(ErrorCode::DebtMathOverflow)?,
                    BPS_DENOMINATOR as u128,
                )
                .ok_or(ErrorCode::DebtMathOverflow)?,
            ),
        };
        require!(shares_to_writeoff > 0, ErrorCode::DebtShareDivisionOverflow);

        let (total_debt, total_debt_shares) = match args.is_debt_token0 {
            true => (pair.total_debt0, pair.total_debt0_shares),
            false => (pair.total_debt1, pair.total_debt1_shares),
        };
        let debt_to_writeoff = match total_debt_shares == 0 {
            true => 0,
            false => min(
                debt_amount,
                shares_to_writeoff
                    .checked_mul(total_debt as u128)
                    .ok_or(ErrorCode::DebtMathOverflow)?
                    .checked_div(total_debt_shares)
                    .ok_or(ErrorCode::DebtMathOverflow)?
                    .try_into()
                    .map_err(|_| ErrorCode::DebtMathOverflow)?,
            ),
        };
        require!(debt_to_writeoff > 0, ErrorCode::DebtMathOverflow);

        // Borrower pays the penalty from escrowed collateral; LPs receive the seized collateral net of caller incentive.
        let collateral_base =
            CPCurve::calculate_amount_in(collateral_reserve, debt_reserve, debt_to_writeoff)?;
        let collateral_with_penalty = ceil_div(
            (collateral_base as u128)
                .checked_mul((BPS_DENOMINATOR + LIQUIDATION_PENALTY_BPS) as u128)
                .ok_or(ErrorCode::DebtMathOverflow)?,
            BPS_DENOMINATOR as u128,
        )
        .ok_or(ErrorCode::DebtMathOverflow)?;
        let collateral_final = match is_insolvent {
            true => position.collateral_amount,
            false => min(collateral_with_penalty, position.collateral_amount as u128)
                .try_into()
                .map_err(|_| ErrorCode::DebtMathOverflow)?,
        };
        require!(collateral_final > 0, ErrorCode::InsufficientAmount);
        require_no_debtless_collateral_stranding(
            is_insolvent,
            shares_to_writeoff,
            position.debt_shares,
            collateral_final,
            position.collateral_amount,
        )?;

        let incentive = liquidation_incentive(collateral_base, collateral_final)?;
        let collateral_to_reserves = collateral_final
            .checked_sub(incentive)
            .ok_or(ErrorCode::DebtMathOverflow)?;
        let shortfall = match is_insolvent {
            true => debt_amount.saturating_sub(quote.amount_out),
            false => 0,
        };

        position.writeoff_debt_shares(pair, shares_to_writeoff, debt_to_writeoff)?;
        position.collateral_amount = position
            .collateral_amount
            .checked_sub(collateral_final)
            .ok_or(ErrorCode::InsufficientAmount)?;
        match is_collateral_token0 {
            true => {
                pair.reserve0 = pair
                    .reserve0
                    .checked_add(collateral_to_reserves)
                    .ok_or(ErrorCode::ReserveOverflow)?;
                pair.cash_reserve0 = pair
                    .cash_reserve0
                    .checked_add(collateral_to_reserves)
                    .ok_or(ErrorCode::Overflow)?;
            }
            false => {
                pair.reserve1 = pair
                    .reserve1
                    .checked_add(collateral_to_reserves)
                    .ok_or(ErrorCode::ReserveOverflow)?;
                pair.cash_reserve1 = pair
                    .cash_reserve1
                    .checked_add(collateral_to_reserves)
                    .ok_or(ErrorCode::Overflow)?;
            }
        }

        if incentive > 0 {
            transfer_from_vault_to_user(
                pair.to_account_info(),
                accounts.leverage_collateral_vault.to_account_info(),
                accounts.liquidator_collateral_token_account.to_account_info(),
                accounts.collateral_token_mint.to_account_info(),
                token_program_for_mint(
                    &accounts.collateral_token_mint.to_account_info(),
                    &accounts.token_program.to_account_info(),
                    &accounts.token_2022_program.to_account_info(),
                ),
                incentive,
                accounts.collateral_token_mint.decimals,
                &[&generate_gamm_pair_seeds!(pair)[..]],
            )?;
        }
        if collateral_to_reserves > 0 {
            transfer_from_vault_to_vault(
                pair.to_account_info(),
                accounts.leverage_collateral_vault.to_account_info(),
                accounts.collateral_token_vault.to_account_info(),
                accounts.collateral_token_mint.to_account_info(),
                token_program_for_mint(
                    &accounts.collateral_token_mint.to_account_info(),
                    &accounts.token_program.to_account_info(),
                    &accounts.token_2022_program.to_account_info(),
                ),
                collateral_to_reserves,
                accounts.collateral_token_mint.decimals,
                &[&generate_gamm_pair_seeds!(pair)[..]],
            )?;
        }

        emit!(LeveragePositionLiquidatedEvent {
            metadata: EventMetadata::new(accounts.liquidator.key(), pair.key()),
            position: position.key(),
            owner: accounts.position_owner.key(),
            liquidator: accounts.liquidator.key(),
            is_debt_token0: args.is_debt_token0,
            debt_repaid: debt_to_writeoff,
            debt_shares_repaid: shares_to_writeoff,
            collateral_seized: collateral_final,
            collateral_to_reserves,
            remaining_collateral: position.collateral_amount,
            closeout_value: quote.amount_out,
            incentive,
            shortfall,
        });

        if position.debt_shares == 0 && position.collateral_amount == 0 {
            position.close(accounts.position_owner.to_account_info())?;
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dust_close_factor_liquidation_rejects_debtless_collateral_stranding() {
        let collateral_amount: u64 = 106;
        let debt_amount: u64 = 100;
        let debt_shares: u128 = 1;
        let shares_to_writeoff = min(
            debt_shares,
            ceil_div(
                debt_shares
                    .checked_mul(CLOSE_FACTOR_BPS as u128)
                    .unwrap(),
                BPS_DENOMINATOR as u128,
            )
            .unwrap(),
        );
        assert_eq!(shares_to_writeoff, debt_shares);

        let closeout_value =
            CPCurve::calculate_amount_out(1_000_000, 1_000_000, collateral_amount).unwrap();
        assert_eq!(closeout_value, 105);
        assert!(debt_amount < closeout_value);
        assert!(equity_bps(closeout_value, debt_amount).unwrap()
            <= LEVERAGE_MAINTENANCE_BUFFER_BPS as u128);

        let collateral_base =
            CPCurve::calculate_amount_in(1_000_000, 1_000_000, debt_amount).unwrap();
        assert_eq!(collateral_base, 101);
        let collateral_with_penalty = ceil_div(
            (collateral_base as u128)
                .checked_mul((BPS_DENOMINATOR + LIQUIDATION_PENALTY_BPS) as u128)
                .unwrap(),
            BPS_DENOMINATOR as u128,
        )
        .unwrap() as u64;
        assert_eq!(collateral_with_penalty, 105);

        assert!(require_no_debtless_collateral_stranding(
            false,
            shares_to_writeoff,
            debt_shares,
            collateral_with_penalty,
            collateral_amount,
        )
        .is_err());
    }

    #[test]
    fn liquidation_dust_guard_allows_partial_and_full_close_cases() {
        assert!(require_no_debtless_collateral_stranding(false, 50, 100, 10, 20).is_ok());
        assert!(require_no_debtless_collateral_stranding(false, 100, 100, 20, 20).is_ok());
        assert!(require_no_debtless_collateral_stranding(true, 100, 100, 10, 20).is_ok());
    }

    #[test]
    fn liquidation_incentive_is_capped_to_surplus_above_base() {
        assert_eq!(liquidation_incentive(10_000, 10_000).unwrap(), 0);
        assert_eq!(liquidation_incentive(10_000, 10_010).unwrap(), 10);
        assert_eq!(liquidation_incentive(10_000, 10_100).unwrap(), 50);
    }

    #[test]
    fn liquidation_incentive_is_zero_when_collateral_is_below_base() {
        assert_eq!(liquidation_incentive(10_000, 9_999).unwrap(), 0);
    }
}
