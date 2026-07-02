use anchor_lang::prelude::*;
use anchor_lang::solana_program::sysvar;
use anchor_spl::{
    token::{Mint, Token, TokenAccount},
    token_interface::Token2022,
};

use crate::{
    constants::*,
    errors::ErrorCode,
    events::{AdjustDebtEvent, EventMetadata, UserPositionUpdatedEvent},
    generate_gamm_pair_seeds,
    instructions::lending::common::AdjustDebtArgs,
    state::{
        futarchy_authority::FutarchyAuthority, pair::Pair, rate_model::RateModel,
        user_position::UserPosition,
    },
    utils::{
        liquidity_delta_circuit_breaker::require_no_same_tx_liquidity_delta,
        token::transfer_from_vault_to_user,
    },
};

#[event_cpi]
#[derive(Accounts)]
pub struct Borrow<'info> {
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
        constraint = user_position.owner == user.key(),
        constraint = user_position.pair == pair.key(),
        seeds = [
            POSITION_SEED_PREFIX,
            pair.key().as_ref(),
            user.key().as_ref()
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
            RESERVE_VAULT_SEED_PREFIX,
            pair.key().as_ref(),
            reserve_token_mint.key().as_ref(),
        ],
        bump = pair.get_reserve_vault_bump(&reserve_token_mint.key())
    )]
    pub reserve_vault: Box<Account<'info, TokenAccount>>,

    #[account(
        mut,
        constraint = user_reserve_token_account.mint == reserve_token_mint.key() @ ErrorCode::InvalidMint,
        token::authority = user,
    )]
    pub user_reserve_token_account: Box<Account<'info, TokenAccount>>,

    #[account(
        constraint = reserve_token_mint.key() == pair.token0 || reserve_token_mint.key() == pair.token1 @ ErrorCode::InvalidMint
    )]
    pub reserve_token_mint: Box<Account<'info, Mint>>,

    pub user: Signer<'info>,
    pub token_program: Program<'info, Token>,
    pub token_2022_program: Program<'info, Token2022>,
    pub system_program: Program<'info, System>,

    /// CHECK: Instructions sysvar used by the liquidity delta circuit breaker.
    #[account(address = sysvar::instructions::ID @ ErrorCode::InvalidInstructionsSysvar)]
    pub instructions_sysvar: UncheckedAccount<'info>,
}

fn resolve_borrow_amount(requested_amount: u64, borrow_limit: u64, user_debt: u64) -> Result<u64> {
    let borrow_amount = if requested_amount == u64::MAX {
        borrow_limit
            .checked_sub(user_debt)
            .ok_or(ErrorCode::DebtMathOverflow)?
    } else {
        requested_amount
    };
    require!(borrow_amount > 0, ErrorCode::AmountZero);
    Ok(borrow_amount)
}

impl<'info> Borrow<'info> {
    pub fn validate_borrow(&self, args: &AdjustDebtArgs) -> Result<()> {
        let AdjustDebtArgs {
            amount: borrow_amount,
        } = args;

        require_no_same_tx_liquidity_delta(
            &self.pair.key(),
            &self.instructions_sysvar.to_account_info(),
        )?;

        // Check reduce-only mode (global or per-pair)
        require!(
            !self
                .futarchy_authority
                .is_reduce_only(self.pair.reduce_only),
            ErrorCode::ReduceOnlyMode
        );

        require!(*borrow_amount > 0, ErrorCode::AmountZero);

        Ok(())
    }

    pub fn update(&mut self) -> Result<()> {
        let pair_key = self.pair.to_account_info().key();
        self.pair.update(
            &self.rate_model,
            &self.futarchy_authority,
            pair_key,
            Some(self.event_authority.to_account_info()),
        )?;
        Ok(())
    }

    pub fn update_and_validate_borrow(&mut self, args: &AdjustDebtArgs) -> Result<()> {
        self.update()?;
        self.validate_borrow(args)?;
        Ok(())
    }

    /// Handles borrowing a specific token from the AMM vault.
    ///
    /// - `vault_token_mint`: Mint address of the token the user wants to borrow.
    /// - `token_vault`: AMM liquidity vault holding the borrowable tokens (pair.token0 or pair.token1 vault).
    /// - `user_token_account`: User's associated token account that will receive the borrowed tokens.
    ///
    /// Notes:
    /// Only the specified borrow amount of the `vault_token_mint` is transferred.
    /// Tokens are sourced directly from the AMM's liquidity vault (`token_vault`).
    /// Assumes that collateral checks have already passed via [`Borrow::validate_borrow`].
    pub fn handle_borrow(ctx: Context<Self>, args: AdjustDebtArgs) -> Result<()> {
        let Borrow {
            user_reserve_token_account,
            reserve_token_mint,
            token_program,
            token_2022_program,
            user,
            user_position,
            ..
        } = ctx.accounts;
        let pair = &mut ctx.accounts.pair;
        let debt_token_vault = &ctx.accounts.reserve_vault;
        let debt_token = reserve_token_mint.key();
        let is_token0 = debt_token == pair.token0;

        let user_debt = match is_token0 {
            true => user_position.calculate_debt0(pair.total_debt0, pair.total_debt0_shares)?,
            false => user_position.calculate_debt1(pair.total_debt1, pair.total_debt1_shares)?,
        };

        // If EMA lags behind a falling spot price, there will be a window where the collateral value may be artificially inflated.
        // To prevent bad debt, we compute a pessimistic collateral factor:
        // CF_pessimistic = min(CF_base, P_spot / P_EMA * CF_base)
        // This ensures the solvency invariant: P_spot >= P_EMA * CF
        let collateral_token = pair.get_collateral_token(&debt_token);
        let collateral_amount = match collateral_token == pair.token0 {
            true => user_position.collateral0,
            false => user_position.collateral1,
        };
        let (borrow_limit, _, liquidation_cf_bps) = pair.get_max_debt_and_cf_bps_for_collateral(
            &pair,
            &collateral_token,
            collateral_amount,
        )?;
        let borrow_amount = resolve_borrow_amount(args.amount, borrow_limit, user_debt)?;

        let new_debt = user_debt
            .checked_add(borrow_amount)
            .ok_or(ErrorCode::DebtMathOverflow)?;

        require_gte!(borrow_limit, new_debt, ErrorCode::BorrowingPowerExceeded);

        // r_cash >= r_debt_out
        match is_token0 {
            true => require_gte!(
                pair.cash_reserve0,
                borrow_amount,
                ErrorCode::InsufficientCashReserve0
            ),
            false => require_gte!(
                pair.cash_reserve1,
                borrow_amount,
                ErrorCode::InsufficientCashReserve1
            ),
        };

        // Transfer tokens from vault to user
        transfer_from_vault_to_user(
            pair.to_account_info(),
            debt_token_vault.to_account_info(),
            user_reserve_token_account.to_account_info(),
            reserve_token_mint.to_account_info(),
            match reserve_token_mint.to_account_info().owner == token_program.key {
                true => token_program.to_account_info(),
                false => token_2022_program.to_account_info(),
            },
            borrow_amount,
            reserve_token_mint.decimals,
            &[&generate_gamm_pair_seeds!(pair)[..]],
        )?;

        user_position.increase_debt(pair, &debt_token, borrow_amount)?;
        user_position.set_liquidation_cf_for_debt_token(&debt_token, &pair, liquidation_cf_bps);

        // Emit debt adjustment event
        let (amount0, amount1) = if is_token0 {
            (borrow_amount as i64, 0)
        } else {
            (0, borrow_amount as i64)
        };

        emit_cpi!(AdjustDebtEvent {
            metadata: EventMetadata::new(user.key(), pair.key()),
            amount0,
            amount1,
        });

        // Emit position updated event
        emit_cpi!(UserPositionUpdatedEvent {
            metadata: EventMetadata::new(user.key(), pair.key()),
            position: user_position.key(),
            collateral0: user_position.collateral0,
            collateral1: user_position.collateral1,
            debt0_shares: user_position.debt0_shares,
            debt1_shares: user_position.debt1_shares,
            collateral0_max_cf_bps: user_position.get_max_cf_bps_for_debt_token(pair, &pair.token1),
            collateral1_max_cf_bps: user_position.get_max_cf_bps_for_debt_token(pair, &pair.token0),
            collateral0_liquidation_cf_bps: user_position.collateral0_liquidation_cf_bps,
            collateral1_liquidation_cf_bps: user_position.collateral1_liquidation_cf_bps,
        });

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn max_borrow_rejects_zero_resolved_amount() {
        let err = resolve_borrow_amount(u64::MAX, 100, 100).unwrap_err();
        assert_eq!(err, error!(ErrorCode::AmountZero));
    }

    #[test]
    fn max_borrow_rejects_debt_above_limit() {
        let err = resolve_borrow_amount(u64::MAX, 100, 101).unwrap_err();
        assert_eq!(err, error!(ErrorCode::DebtMathOverflow));
    }

    #[test]
    fn explicit_borrow_rejects_zero_amount() {
        let err = resolve_borrow_amount(0, 100, 0).unwrap_err();
        assert_eq!(err, error!(ErrorCode::AmountZero));
    }

    #[test]
    fn max_borrow_resolves_positive_remaining_limit() {
        assert_eq!(resolve_borrow_amount(u64::MAX, 100, 40).unwrap(), 60);
    }
}
