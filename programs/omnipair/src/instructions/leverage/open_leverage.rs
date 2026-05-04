use anchor_lang::prelude::*;
use anchor_spl::{
    token::{Mint, Token, TokenAccount},
    token_interface::Token2022,
};

use crate::{
    constants::*,
    errors::ErrorCode,
    events::{EventMetadata, LeveragePositionOpenedEvent, SwapEvent},
    generate_gamm_pair_seeds,
    state::{FutarchyAuthority, Pair, RateModel, UserLeveragePosition},
    utils::{
        account::get_size_with_discriminator,
        token::{transfer_from_user_to_vault, transfer_from_vault_to_vault},
    },
};

use super::common::{quote_swap, spot_value_from_reserves, token_program_for_mint, unwind_impact_bps};

#[derive(AnchorSerialize, AnchorDeserialize, Clone)]
pub struct OpenLeverageArgs {
    pub is_debt_token0: bool,
    pub margin_amount: u64,
    pub multiplier_bps: u64,
    pub min_collateral_out: u64,
}

#[event_cpi]
#[derive(Accounts)]
#[instruction(args: OpenLeverageArgs)]
pub struct OpenLeverage<'info> {
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

    #[account(
        init,
        payer = user,
        space = get_size_with_discriminator::<UserLeveragePosition>(),
        seeds = [
            LEVERAGE_POSITION_SEED_PREFIX,
            pair.key().as_ref(),
            user.key().as_ref(),
            &[args.is_debt_token0 as u8]
        ],
        bump
    )]
    pub user_leverage_position: Account<'info, UserLeveragePosition>,

    #[account(
        mut,
        seeds = [
            RESERVE_VAULT_SEED_PREFIX,
            pair.key().as_ref(),
            token_in_mint.key().as_ref(),
        ],
        bump = pair.get_reserve_vault_bump(&token_in_mint.key())
    )]
    pub token_in_vault: Box<Account<'info, TokenAccount>>,

    #[account(
        mut,
        seeds = [
            RESERVE_VAULT_SEED_PREFIX,
            pair.key().as_ref(),
            token_out_mint.key().as_ref(),
        ],
        bump = pair.get_reserve_vault_bump(&token_out_mint.key())
    )]
    pub token_out_vault: Box<Account<'info, TokenAccount>>,

    #[account(
        init_if_needed,
        payer = user,
        token::mint = token_out_mint,
        token::authority = pair,
        seeds = [
            LEVERAGE_COLLATERAL_VAULT_SEED_PREFIX,
            pair.key().as_ref(),
            token_out_mint.key().as_ref(),
        ],
        bump
    )]
    pub leverage_collateral_vault: Box<Account<'info, TokenAccount>>,

    #[account(
        mut,
        constraint = user_token_in_account.mint == token_in_mint.key() @ ErrorCode::InvalidTokenAccount,
        token::authority = user,
    )]
    pub user_token_in_account: Box<Account<'info, TokenAccount>>,

    #[account(
        constraint = token_in_mint.key() == pair.token0 || token_in_mint.key() == pair.token1 @ ErrorCode::InvalidMint
    )]
    pub token_in_mint: Box<Account<'info, Mint>>,

    #[account(
        constraint = token_out_mint.key() == pair.token0 || token_out_mint.key() == pair.token1 @ ErrorCode::InvalidMint
    )]
    pub token_out_mint: Box<Account<'info, Mint>>,

    #[account(mut)]
    pub user: Signer<'info>,
    pub token_program: Program<'info, Token>,
    pub token_2022_program: Program<'info, Token2022>,
    pub system_program: Program<'info, System>,
}

impl<'info> OpenLeverage<'info> {
    pub fn update_and_validate_open(&mut self, args: &OpenLeverageArgs) -> Result<()> {
        let pair_key = self.pair.key();
        self.pair.update(
            &self.rate_model,
            &self.futarchy_authority,
            pair_key,
            Some(self.event_authority.to_account_info()),
        )?;

        require!(
            !self.futarchy_authority.is_reduce_only(self.pair.reduce_only),
            ErrorCode::ReduceOnlyMode
        );
        require!(args.margin_amount > 0, ErrorCode::AmountZero);
        require!(
            args.multiplier_bps > BPS_DENOMINATOR as u64,
            ErrorCode::InvalidArgument
        );
        require!(
            args.multiplier_bps <= LEVERAGE_MAX_MULTIPLIER_BPS,
            ErrorCode::LeverageMultiplierTooHigh
        );
        require_gte!(
            self.user_token_in_account.amount,
            args.margin_amount,
            ErrorCode::InsufficientBalance
        );

        let debt_token = if args.is_debt_token0 { self.pair.token0 } else { self.pair.token1 };
        let collateral_token = self.pair.get_token_y(&debt_token);
        require_keys_eq!(self.token_in_mint.key(), debt_token, ErrorCode::InvalidMint);
        require_keys_eq!(self.token_out_mint.key(), collateral_token, ErrorCode::InvalidMint);
        require_keys_neq!(
            self.token_in_vault.key(),
            self.token_out_vault.key(),
            ErrorCode::InvalidVaultSameAccount
        );
        Ok(())
    }

    pub fn handle_open_leverage(
        mut ctx: Context<'_, '_, '_, 'info, Self>,
        args: OpenLeverageArgs,
    ) -> Result<()> {
        let accounts = &mut ctx.accounts;
        let pair = &mut accounts.pair;
        let clock = Clock::get()?;

        let debt_amount: u64 = (args.margin_amount as u128)
            .checked_mul(args.multiplier_bps as u128)
            .ok_or(ErrorCode::Overflow)?
            .checked_div(BPS_DENOMINATOR as u128)
            .ok_or(ErrorCode::Overflow)?
            .checked_sub(args.margin_amount as u128)
            .ok_or(ErrorCode::Overflow)?
            .try_into()
            .map_err(|_| ErrorCode::Overflow)?;
        require!(debt_amount > 0, ErrorCode::AmountZero);

        let notional = args
            .margin_amount
            .checked_add(debt_amount)
            .ok_or(ErrorCode::Overflow)?;
        let is_token0_in = args.is_debt_token0;
        let reserve_in = if is_token0_in { pair.reserve0 } else { pair.reserve1 };
        let reserve_out = if is_token0_in { pair.reserve1 } else { pair.reserve0 };
        let cash_out = if is_token0_in { pair.cash_reserve1 } else { pair.cash_reserve0 };

        match args.is_debt_token0 {
            true => require_gte!(pair.cash_reserve0, debt_amount, ErrorCode::InsufficientCashReserve0),
            false => require_gte!(pair.cash_reserve1, debt_amount, ErrorCode::InsufficientCashReserve1),
        }

        let quote = quote_swap(
            notional,
            reserve_in,
            reserve_out,
            pair.swap_fee_bps,
            accounts.futarchy_authority.revenue_share.swap_bps,
        )?;
        require_gte!(
            quote.amount_out,
            args.min_collateral_out,
            ErrorCode::SlippageExceeded
        );
        require_gte!(cash_out, quote.amount_out, ErrorCode::InsufficientLiquidity);

        let post_reserve_in = reserve_in
            .checked_add(quote.amount_in_with_lp_fee)
            .ok_or(ErrorCode::Overflow)?;
        let post_reserve_out = reserve_out
            .checked_sub(quote.amount_out)
            .ok_or(ErrorCode::Overflow)?;
        let closeout_value = quote_swap(
            quote.amount_out,
            post_reserve_out,
            post_reserve_in,
            pair.swap_fee_bps,
            accounts.futarchy_authority.revenue_share.swap_bps,
        )?
        .amount_out;
        require_gt!(closeout_value, debt_amount, ErrorCode::LeverageInitialMarginTooLow);
        let equity = closeout_value
            .checked_sub(debt_amount)
            .ok_or(ErrorCode::LeverageInitialMarginTooLow)?;
        let initial_margin_bps = (equity as u128)
            .checked_mul(BPS_DENOMINATOR as u128)
            .ok_or(ErrorCode::Overflow)?
            .checked_div(closeout_value as u128)
            .ok_or(ErrorCode::Overflow)?;
        require_gte!(
            initial_margin_bps,
            LEVERAGE_INITIAL_MARGIN_BPS as u128,
            ErrorCode::LeverageInitialMarginTooLow
        );

        let spot_value = spot_value_from_reserves(quote.amount_out, post_reserve_out, post_reserve_in)?;
        let unwind_impact_bps = unwind_impact_bps(spot_value, closeout_value)?;
        require_gte!(
            LEVERAGE_MAX_UNWIND_IMPACT_BPS as u128,
            unwind_impact_bps,
            ErrorCode::LeverageUnwindImpactTooHigh
        );

        require_gte!(
            accounts.user_token_in_account.amount,
            args.margin_amount,
            ErrorCode::InsufficientBalance
        );
        match args.is_debt_token0 {
            true => require_gte!(pair.cash_reserve0, debt_amount, ErrorCode::InsufficientCashReserve0),
            false => require_gte!(pair.cash_reserve1, debt_amount, ErrorCode::InsufficientCashReserve1),
        }

        let last_k = pair.k();
        accounts
            .user_leverage_position
            .initialize(
                accounts.user.key(),
                pair.key(),
                args.is_debt_token0,
                quote.amount_out,
                args.margin_amount,
                notional,
                debt_amount,
                0,
                args.multiplier_bps,
                clock.unix_timestamp,
                clock.slot,
                ctx.bumps.user_leverage_position,
            );
        accounts.user_leverage_position.increase_debt(pair, debt_amount)?;
        let debt_shares = accounts.user_leverage_position.debt_shares;

        match is_token0_in {
            true => {
                pair.reserve0 = post_reserve_in;
                pair.reserve1 = post_reserve_out;
                pair.cash_reserve0 = pair
                    .cash_reserve0
                    .checked_add(quote.amount_in_with_lp_fee)
                    .ok_or(ErrorCode::Overflow)?;
                pair.cash_reserve1 = pair
                    .cash_reserve1
                    .checked_sub(quote.amount_out)
                    .ok_or(ErrorCode::CashReserveUnderflow)?;
            }
            false => {
                pair.reserve1 = post_reserve_in;
                pair.reserve0 = post_reserve_out;
                pair.cash_reserve1 = pair
                    .cash_reserve1
                    .checked_add(quote.amount_in_with_lp_fee)
                    .ok_or(ErrorCode::Overflow)?;
                pair.cash_reserve0 = pair
                    .cash_reserve0
                    .checked_sub(quote.amount_out)
                    .ok_or(ErrorCode::CashReserveUnderflow)?;
            }
        }
        require_gte!(pair.k(), last_k, ErrorCode::BrokenInvariant);

        transfer_from_user_to_vault(
            accounts.user.to_account_info(),
            accounts.user_token_in_account.to_account_info(),
            accounts.token_in_vault.to_account_info(),
            accounts.token_in_mint.to_account_info(),
            token_program_for_mint(
                &accounts.token_in_mint.to_account_info(),
                &accounts.token_program.to_account_info(),
                &accounts.token_2022_program.to_account_info(),
            ),
            args.margin_amount,
            accounts.token_in_mint.decimals,
        )?;

        transfer_from_vault_to_vault(
            pair.to_account_info(),
            accounts.token_out_vault.to_account_info(),
            accounts.leverage_collateral_vault.to_account_info(),
            accounts.token_out_mint.to_account_info(),
            token_program_for_mint(
                &accounts.token_out_mint.to_account_info(),
                &accounts.token_program.to_account_info(),
                &accounts.token_2022_program.to_account_info(),
            ),
            quote.amount_out,
            accounts.token_out_mint.decimals,
            &[&generate_gamm_pair_seeds!(pair)[..]],
        )?;

        emit!(SwapEvent {
            metadata: EventMetadata::new(accounts.user.key(), pair.key()),
            reserve0: pair.reserve0,
            reserve1: pair.reserve1,
            is_token0_in,
            amount_in: notional,
            amount_out: quote.amount_out,
            amount_in_after_fee: quote.amount_in_after_swap_fee,
            lp_fee: quote.lp_fee,
            protocol_fee: quote.protocol_fee,
        });
        emit!(LeveragePositionOpenedEvent {
            metadata: EventMetadata::new(accounts.user.key(), pair.key()),
            position: accounts.user_leverage_position.key(),
            owner: accounts.user.key(),
            is_debt_token0: args.is_debt_token0,
            margin_amount: args.margin_amount,
            debt_amount,
            debt_shares,
            collateral_amount: quote.amount_out,
            closeout_value,
            equity,
            multiplier_bps: args.multiplier_bps,
        });
        Ok(())
    }
}
