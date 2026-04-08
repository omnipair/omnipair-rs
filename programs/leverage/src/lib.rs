use anchor_lang::prelude::*;
use anchor_lang::solana_program::{
    instruction::{AccountMeta, Instruction},
    program::invoke,
    hash::hash,
};
use anchor_spl::token::{Token, TokenAccount, Mint, TransferChecked};
use omnipair::{
    FlashLoanCallbackData, FlashloanArgs, SwapArgs, AdjustCollateralArgs, AdjustDebtArgs,
    state::{Pair, UserPosition},
    ceil_div,
};

pub mod state;
pub use state::{UserLeveragePosition, LEVERAGE_POSITION_SEED_PREFIX};

declare_id!("7S6gLNQXrx3GtR91xnF2ZTjdPeJfbMq79u4TovRDQEBn");

// ── Remaining-accounts layout ─────────────────────────────────────────────────
//
// Shared by multiply and close_multiply. Passed verbatim from the outer
// instruction → omnipair flashloan → flash_loan_callback.
//
// For multiply (open), TOKEN_IN = lev_collateral reserve, TOKEN_OUT = position token reserve.
// For close_multiply,  TOKEN_IN = position token reserve, TOKEN_OUT = lev_collateral reserve.
//
// Index  Account                   Writable
// 0      pair                       yes
// 1      rate_model                 yes
// 2      futarchy_authority         no
// 3      user_position              yes
// 4      token_in_reserve_vault     yes
// 5      token_out_reserve_vault    yes
// 6      collateral_vault           yes   (position-token collateral vault)
// 7      token_2022_program         no
// 8      system_program             no
// 9      event_authority            no    (omnipair's __event_authority PDA)
// 10     omnipair_program           no
// 11     user_leverage_position     yes
const IDX_PAIR: usize = 0;
const IDX_RATE_MODEL: usize = 1;
const IDX_FUTARCHY: usize = 2;
const IDX_USER_POSITION: usize = 3;
const IDX_TOKEN_IN_VAULT: usize = 4;
const IDX_TOKEN_OUT_VAULT: usize = 5;
const IDX_COLLATERAL_VAULT: usize = 6;
const IDX_TOKEN_2022_PROGRAM: usize = 7;
const IDX_SYSTEM_PROGRAM: usize = 8;
const IDX_EVENT_AUTHORITY: usize = 9;
const IDX_OMNIPAIR_PROGRAM: usize = 10;
const IDX_USER_LEV_POSITION: usize = 11;

const BPS_DENOMINATOR: u64 = 10_000;
const FLASHLOAN_FEE_BPS: u64 = 5;

/// Params encoded into the flashloan `data` bytes and decoded in the callback.
#[derive(AnchorSerialize, AnchorDeserialize, Clone)]
pub struct InternalCallbackData {
    /// true = close (deleverage), false = open (multiply)
    pub is_close: bool,
    pub is_lev_collateral0: bool,
    /// open: total swap amount in (lev_collateral * multiplier). close: unused (0).
    pub swap_amount_in: u64,
    /// open: min position token out. close: min lev_collateral out after swap-back.
    pub min_amount_out: u64,
    /// open: borrow_amount + flashloan fee (repay vault). close: debt_amount + flashloan fee.
    pub repay_amount: u64,
}

#[program]
pub mod omnipair_leverage {
    use super::*;

    /// Opens a leveraged position.
    ///
    /// Flashloans the borrow portion from omnipair, swaps everything into the
    /// position token, deposits as collateral, borrows to repay — all atomic.
    ///
    /// - `is_lev_collateral0`: true = long token0 (token0 is lev_collateral, token1 is position token)
    /// - `multiplier_bps`: leverage in BPS (20_000 = 2×); must be > 10_000
    /// - `max_slippage_bps`: max deviation from spot price (10_000 = no check)
    pub fn multiply<'info>(
        ctx: Context<'_, '_, '_, 'info, Multiply<'info>>,
        is_lev_collateral0: bool,
        lev_collateral_amount: u64,
        multiplier_bps: u64,
        max_slippage_bps: u64,
    ) -> Result<()> {
        require!(lev_collateral_amount > 0, LeverageError::AmountZero);
        require!(multiplier_bps > BPS_DENOMINATOR, LeverageError::MultiplierTooLow);
        require!(max_slippage_bps <= BPS_DENOMINATOR, LeverageError::InvalidSlippage);

        let ra = ctx.remaining_accounts;
        require!(ra.len() >= 12, LeverageError::MissingRemainingAccounts);

        // Tie remaining_accounts to the validated outer accounts
        validate_remaining_accounts(
            ra, &ctx.accounts.pair, &ctx.accounts.rate_model,
            &ctx.accounts.futarchy_authority,
            ctx.accounts.reserve0_vault.key(), ctx.accounts.reserve1_vault.key(),
            is_lev_collateral0, false, // false = open, TOKEN_IN is lev_collateral
            ctx.accounts.user_leverage_position.key(),
        )?;

        // ── compute amounts ────────────────────────────────────────────────
        let swap_amount_in: u64 = (lev_collateral_amount as u128)
            .checked_mul(multiplier_bps as u128).ok_or(LeverageError::Overflow)?
            .checked_div(BPS_DENOMINATOR as u128).ok_or(LeverageError::Overflow)?
            .try_into().map_err(|_| LeverageError::Overflow)?;

        let borrow_amount = swap_amount_in
            .checked_sub(lev_collateral_amount).ok_or(LeverageError::Overflow)?;

        let flashloan_fee = ceil_div(
            (borrow_amount as u128)
                .checked_mul(FLASHLOAN_FEE_BPS as u128).ok_or(LeverageError::Overflow)?,
            BPS_DENOMINATOR as u128,
        ).ok_or(LeverageError::Overflow)? as u64;

        let repay_amount = borrow_amount
            .checked_add(flashloan_fee).ok_or(LeverageError::Overflow)?;

        // ── slippage: spot-price floor ─────────────────────────────────────
        let pair_data = Pair::try_deserialize(&mut &**ra[IDX_PAIR].try_borrow_data()?)?;
        let (reserve_in, reserve_out) = if is_lev_collateral0 {
            (pair_data.reserve0, pair_data.reserve1)
        } else {
            (pair_data.reserve1, pair_data.reserve0)
        };
        require!(reserve_in > 0 && reserve_out > 0, LeverageError::InsufficientLiquidity);

        let spot_out: u64 = (lev_collateral_amount as u128)
            .checked_mul(multiplier_bps as u128).ok_or(LeverageError::Overflow)?
            .checked_mul(reserve_out as u128).ok_or(LeverageError::Overflow)?
            .checked_div(
                (reserve_in as u128)
                    .checked_mul(BPS_DENOMINATOR as u128).ok_or(LeverageError::Overflow)?
            ).ok_or(LeverageError::Overflow)?
            .try_into().map_err(|_| LeverageError::Overflow)?;

        let min_amount_out: u64 = ((spot_out as u128)
            .saturating_mul((BPS_DENOMINATOR as u128).saturating_sub(max_slippage_bps as u128))
            / BPS_DENOMINATOR as u128)
            .try_into().map_err(|_| LeverageError::Overflow)?;

        // ── init user leverage position ────────────────────────────────────
        {
            let lev_pos = &mut ctx.accounts.user_leverage_position;
            lev_pos.initialize(
                ctx.accounts.user.key(),
                ctx.accounts.pair.key(),
                is_lev_collateral0,
                lev_collateral_amount,
                multiplier_bps,
                borrow_amount,
                Clock::get()?.unix_timestamp,
                ctx.bumps.user_leverage_position,
            );
        }

        // ── encode callback params ─────────────────────────────────────────
        let mut callback_bytes = Vec::new();
        InternalCallbackData {
            is_close: false,
            is_lev_collateral0,
            swap_amount_in,
            min_amount_out,
            repay_amount,
        }.serialize(&mut callback_bytes)?;

        let (amount0, amount1) = if is_lev_collateral0 {
            (borrow_amount, 0u64)
        } else {
            (0u64, borrow_amount)
        };

        invoke_flashloan(&ctx.accounts, ra, amount0, amount1, callback_bytes)?;
        Ok(())
    }

    /// Closes a leveraged position and returns margin + PnL to the user.
    ///
    /// Flashloans the outstanding debt amount, repays the borrow, withdraws all
    /// collateral, swaps back to the lev_collateral token, repays the flashloan.
    /// Remainder in the user's receiver account is their margin + PnL.
    /// Closes the UserLeveragePosition PDA and returns rent to the user.
    ///
    /// - `is_lev_collateral0`: must match the direction used when opening
    /// - `min_collateral_out`: minimum lev_collateral token to receive after swapping back
    pub fn close_multiply<'info>(
        ctx: Context<'_, '_, '_, 'info, CloseMultiply<'info>>,
        is_lev_collateral0: bool,
        min_collateral_out: u64,
    ) -> Result<()> {
        let ra = ctx.remaining_accounts;
        require!(ra.len() >= 12, LeverageError::MissingRemainingAccounts);

        // For close: TOKEN_IN = position token reserve, TOKEN_OUT = lev_collateral reserve
        validate_remaining_accounts(
            ra, &ctx.accounts.pair, &ctx.accounts.rate_model,
            &ctx.accounts.futarchy_authority,
            ctx.accounts.reserve0_vault.key(), ctx.accounts.reserve1_vault.key(),
            is_lev_collateral0, true, // true = close, TOKEN_IN is position token
            ctx.accounts.user_leverage_position.key(),
        )?;

        // ── compute current debt from omnipair state ───────────────────────
        let pair_data = Pair::try_deserialize(&mut &**ra[IDX_PAIR].try_borrow_data()?)?;
        let user_pos = UserPosition::try_deserialize(&mut &**ra[IDX_USER_POSITION].try_borrow_data()?)?;

        let debt_amount = if is_lev_collateral0 {
            user_pos.calculate_debt0(pair_data.total_debt0, pair_data.total_debt0_shares)?
        } else {
            user_pos.calculate_debt1(pair_data.total_debt1, pair_data.total_debt1_shares)?
        };
        require!(debt_amount > 0, LeverageError::PositionNotOpen);

        let flashloan_fee = ceil_div(
            (debt_amount as u128)
                .checked_mul(FLASHLOAN_FEE_BPS as u128).ok_or(LeverageError::Overflow)?,
            BPS_DENOMINATOR as u128,
        ).ok_or(LeverageError::Overflow)? as u64;

        let repay_amount = debt_amount
            .checked_add(flashloan_fee).ok_or(LeverageError::Overflow)?;

        // ── encode callback params ─────────────────────────────────────────
        let mut callback_bytes = Vec::new();
        InternalCallbackData {
            is_close: true,
            is_lev_collateral0,
            swap_amount_in: 0, // determined at runtime in callback
            min_amount_out: min_collateral_out,
            repay_amount,
        }.serialize(&mut callback_bytes)?;

        let (amount0, amount1) = if is_lev_collateral0 {
            (debt_amount, 0u64)
        } else {
            (0u64, debt_amount)
        };

        let accounts = CloseMultiplyAsFlashloan {
            pair: ctx.accounts.pair.to_account_info(),
            rate_model: ctx.accounts.rate_model.to_account_info(),
            futarchy_authority: ctx.accounts.futarchy_authority.to_account_info(),
            reserve0_vault: ctx.accounts.reserve0_vault.to_account_info(),
            reserve1_vault: ctx.accounts.reserve1_vault.to_account_info(),
            token0_mint: ctx.accounts.token0_mint.to_account_info(),
            token1_mint: ctx.accounts.token1_mint.to_account_info(),
            repay0_vault: ctx.accounts.repay0_vault.to_account_info(),
            repay1_vault: ctx.accounts.repay1_vault.to_account_info(),
            receiver_token0_account: ctx.accounts.receiver_token0_account.to_account_info(),
            receiver_token1_account: ctx.accounts.receiver_token1_account.to_account_info(),
            receiver_program: ctx.accounts.receiver_program.to_account_info(),
            user: ctx.accounts.user.to_account_info(),
            token_program: ctx.accounts.token_program.to_account_info(),
            token_2022_program: ctx.accounts.token_2022_program.to_account_info(),
            system_program: ctx.accounts.system_program.to_account_info(),
            user_leverage_position: ctx.accounts.user_leverage_position.to_account_info(),
        };
        invoke_flashloan_raw(&accounts, ra, amount0, amount1, callback_bytes)?;
        Ok(())
        // Anchor closes user_leverage_position and returns rent to user after this returns.
    }

    /// Called by omnipair mid-flashloan. Never called directly by users.
    ///
    /// Open path:  swap → add_collateral → borrow → repay flashloan
    /// Close path: repay debt → remove_collateral → swap back → repay flashloan
    pub fn flash_loan_callback<'info>(
        ctx: Context<'_, '_, '_, 'info, FlashLoanCallback<'info>>,
        callback_data: FlashLoanCallbackData,
    ) -> Result<()> {
        let InternalCallbackData {
            is_close,
            is_lev_collateral0,
            swap_amount_in,
            min_amount_out,
            repay_amount,
        } = InternalCallbackData::try_from_slice(&callback_data.data)
            .map_err(|_| LeverageError::InvalidCallbackData)?;

        let ra = ctx.remaining_accounts;
        let omnipair_program = &ra[IDX_OMNIPAIR_PROGRAM];
        let event_authority  = &ra[IDX_EVENT_AUTHORITY];

        let user         = ctx.accounts.initiator.to_account_info();
        let token_program = ctx.accounts.token_program.to_account_info();

        // Direction selector:
        //   open:  lev_collateral → position token
        //   close: position token → lev_collateral (reversed)
        //
        // XOR of is_lev_collateral0 and is_close gives the "is token0 the swap-in" flag.
        //   open,  is_lev_collateral0=true  → token0 in, token1 out
        //   open,  is_lev_collateral0=false → token1 in, token0 out
        //   close, is_lev_collateral0=true  → token1 in, token0 out (reversed)
        //   close, is_lev_collateral0=false → token0 in, token1 out (reversed)
        let token0_in = is_lev_collateral0 ^ is_close;

        let (token_in_mint, token_out_mint, user_token_in_ai, user_token_out_ai) =
            if token0_in {
                (
                    ctx.accounts.token0_mint.to_account_info(),
                    ctx.accounts.token1_mint.to_account_info(),
                    ctx.accounts.receiver_token0_account.to_account_info(),
                    ctx.accounts.receiver_token1_account.to_account_info(),
                )
            } else {
                (
                    ctx.accounts.token1_mint.to_account_info(),
                    ctx.accounts.token0_mint.to_account_info(),
                    ctx.accounts.receiver_token1_account.to_account_info(),
                    ctx.accounts.receiver_token0_account.to_account_info(),
                )
            };

        if is_close {
            // ── Close path ────────────────────────────────────────────────
            // After flashloan:  user_token_out_ai holds debt_amount of lev_collateral.
            //
            // 1. repay full debt → debt = 0
            // 2. remove all collateral → user_token_in_ai receives position token
            // 3. swap position token → lev_collateral → user_token_out_ai receives payout
            // 4. send repay_amount of lev_collateral to repay vault
            // Net: user_token_out_ai retains margin + PnL - flashloan fee

            // 1. Repay
            {
                let accounts = vec![
                    AccountMeta::new(ra[IDX_PAIR].key(), false),
                    AccountMeta::new(ra[IDX_USER_POSITION].key(), false),
                    AccountMeta::new(ra[IDX_RATE_MODEL].key(), false),
                    AccountMeta::new_readonly(ra[IDX_FUTARCHY].key(), false),
                    AccountMeta::new(ra[IDX_TOKEN_OUT_VAULT].key(), false), // lev_collateral reserve
                    AccountMeta::new(user_token_out_ai.key(), false),       // lev_collateral account
                    AccountMeta::new_readonly(token_out_mint.key(), false),
                    AccountMeta::new(user.key(), true),
                    AccountMeta::new_readonly(token_program.key(), false),
                    AccountMeta::new_readonly(ra[IDX_TOKEN_2022_PROGRAM].key(), false),
                    AccountMeta::new_readonly(ra[IDX_SYSTEM_PROGRAM].key(), false),
                    AccountMeta::new_readonly(event_authority.key(), false),
                    AccountMeta::new_readonly(omnipair_program.key(), false),
                ];
                let mut ix_data = hash(b"global:repay").to_bytes()[..8].to_vec();
                AdjustDebtArgs { amount: u64::MAX }.serialize(&mut ix_data)?;
                invoke(
                    &Instruction { program_id: omnipair_program.key(), accounts, data: ix_data },
                    &[
                        ra[IDX_PAIR].to_account_info(),
                        ra[IDX_USER_POSITION].to_account_info(),
                        ra[IDX_RATE_MODEL].to_account_info(),
                        ra[IDX_FUTARCHY].to_account_info(),
                        ra[IDX_TOKEN_OUT_VAULT].to_account_info(),
                        user_token_out_ai.clone(),
                        token_out_mint.clone(),
                        user.clone(),
                        token_program.clone(),
                        ra[IDX_TOKEN_2022_PROGRAM].to_account_info(),
                        ra[IDX_SYSTEM_PROGRAM].to_account_info(),
                        event_authority.to_account_info(),
                        omnipair_program.to_account_info(),
                    ],
                )?;
            }

            // 2. Remove all collateral
            let balance_before_remove = if token0_in {
                ctx.accounts.receiver_token0_account.amount
            } else {
                ctx.accounts.receiver_token1_account.amount
            };
            {
                let accounts = vec![
                    AccountMeta::new(ra[IDX_PAIR].key(), false),
                    AccountMeta::new(ra[IDX_USER_POSITION].key(), false),
                    AccountMeta::new(ra[IDX_RATE_MODEL].key(), false),
                    AccountMeta::new_readonly(ra[IDX_FUTARCHY].key(), false),
                    AccountMeta::new(ra[IDX_COLLATERAL_VAULT].key(), false), // position token collateral vault
                    AccountMeta::new(user_token_in_ai.key(), false),          // position token account
                    AccountMeta::new_readonly(token_in_mint.key(), false),
                    AccountMeta::new(user.key(), true),
                    AccountMeta::new_readonly(token_program.key(), false),
                    AccountMeta::new_readonly(ra[IDX_TOKEN_2022_PROGRAM].key(), false),
                    AccountMeta::new_readonly(ra[IDX_SYSTEM_PROGRAM].key(), false),
                    AccountMeta::new_readonly(event_authority.key(), false),
                    AccountMeta::new_readonly(omnipair_program.key(), false),
                ];
                let mut ix_data = hash(b"global:remove_collateral").to_bytes()[..8].to_vec();
                AdjustCollateralArgs { amount: u64::MAX }.serialize(&mut ix_data)?;
                invoke(
                    &Instruction { program_id: omnipair_program.key(), accounts, data: ix_data },
                    &[
                        ra[IDX_PAIR].to_account_info(),
                        ra[IDX_USER_POSITION].to_account_info(),
                        ra[IDX_RATE_MODEL].to_account_info(),
                        ra[IDX_FUTARCHY].to_account_info(),
                        ra[IDX_COLLATERAL_VAULT].to_account_info(),
                        user_token_in_ai.clone(),
                        token_in_mint.clone(),
                        user.clone(),
                        token_program.clone(),
                        ra[IDX_TOKEN_2022_PROGRAM].to_account_info(),
                        ra[IDX_SYSTEM_PROGRAM].to_account_info(),
                        event_authority.to_account_info(),
                        omnipair_program.to_account_info(),
                    ],
                )?;
            }

            // 3. Compute amount of position token withdrawn
            let amount_in_swap = {
                if token0_in {
                    ctx.accounts.receiver_token0_account.reload()?;
                    ctx.accounts.receiver_token0_account.amount
                        .checked_sub(balance_before_remove)
                        .ok_or(LeverageError::SwapFailed)?
                } else {
                    ctx.accounts.receiver_token1_account.reload()?;
                    ctx.accounts.receiver_token1_account.amount
                        .checked_sub(balance_before_remove)
                        .ok_or(LeverageError::SwapFailed)?
                }
            };
            require!(amount_in_swap > 0, LeverageError::SwapFailed);

            // 4. Swap position token → lev_collateral
            {
                let accounts = vec![
                    AccountMeta::new(ra[IDX_PAIR].key(), false),
                    AccountMeta::new(ra[IDX_RATE_MODEL].key(), false),
                    AccountMeta::new_readonly(ra[IDX_FUTARCHY].key(), false),
                    AccountMeta::new(ra[IDX_TOKEN_IN_VAULT].key(), false),
                    AccountMeta::new(ra[IDX_TOKEN_OUT_VAULT].key(), false),
                    AccountMeta::new(user_token_in_ai.key(), false),
                    AccountMeta::new(user_token_out_ai.key(), false),
                    AccountMeta::new_readonly(token_in_mint.key(), false),
                    AccountMeta::new_readonly(token_out_mint.key(), false),
                    AccountMeta::new_readonly(user.key(), true),
                    AccountMeta::new_readonly(token_program.key(), false),
                    AccountMeta::new_readonly(ra[IDX_TOKEN_2022_PROGRAM].key(), false),
                    AccountMeta::new_readonly(event_authority.key(), false),
                    AccountMeta::new_readonly(omnipair_program.key(), false),
                ];
                let mut ix_data = hash(b"global:swap").to_bytes()[..8].to_vec();
                SwapArgs { amount_in: amount_in_swap, min_amount_out }.serialize(&mut ix_data)?;
                invoke(
                    &Instruction { program_id: omnipair_program.key(), accounts, data: ix_data },
                    &[
                        ra[IDX_PAIR].to_account_info(),
                        ra[IDX_RATE_MODEL].to_account_info(),
                        ra[IDX_FUTARCHY].to_account_info(),
                        ra[IDX_TOKEN_IN_VAULT].to_account_info(),
                        ra[IDX_TOKEN_OUT_VAULT].to_account_info(),
                        user_token_in_ai.clone(),
                        user_token_out_ai.clone(),
                        token_in_mint.clone(),
                        token_out_mint.clone(),
                        user.clone(),
                        token_program.clone(),
                        ra[IDX_TOKEN_2022_PROGRAM].to_account_info(),
                        event_authority.to_account_info(),
                        omnipair_program.to_account_info(),
                    ],
                )?;
            }

            // 5. Send repay_amount of lev_collateral to flashloan repay vault
            let (repay_vault, repay_mint, repay_decimals) = if is_lev_collateral0 {
                (ctx.accounts.repay0_vault.to_account_info(), ctx.accounts.token0_mint.to_account_info(), ctx.accounts.token0_mint.decimals)
            } else {
                (ctx.accounts.repay1_vault.to_account_info(), ctx.accounts.token1_mint.to_account_info(), ctx.accounts.token1_mint.decimals)
            };
            anchor_spl::token::transfer_checked(
                CpiContext::new(
                    token_program,
                    TransferChecked {
                        from:      user_token_out_ai, // lev_collateral account
                        mint:      repay_mint,
                        to:        repay_vault,
                        authority: user,
                    },
                ),
                repay_amount,
                repay_decimals,
            )?;

        } else {
            // ── Open path ─────────────────────────────────────────────────
            // After flashloan: user_token_in_ai holds borrow_amount of lev_collateral.
            // user deposits lev_collateral_amount separately before this tx.
            //
            // 1. swap   lev_collateral + borrow_amount → position token
            // 2. add_collateral   deposit position token into omnipair
            // 3. borrow           borrow repay_amount of lev_collateral token
            // 4. repay            return principal + fee to repay vault

            // Record token_out balance before swap to compute exact delta
            let balance_before_swap = if token0_in {
                ctx.accounts.receiver_token1_account.amount
            } else {
                ctx.accounts.receiver_token0_account.amount
            };

            // 1. Swap
            {
                let accounts = vec![
                    AccountMeta::new(ra[IDX_PAIR].key(), false),
                    AccountMeta::new(ra[IDX_RATE_MODEL].key(), false),
                    AccountMeta::new_readonly(ra[IDX_FUTARCHY].key(), false),
                    AccountMeta::new(ra[IDX_TOKEN_IN_VAULT].key(), false),
                    AccountMeta::new(ra[IDX_TOKEN_OUT_VAULT].key(), false),
                    AccountMeta::new(user_token_in_ai.key(), false),
                    AccountMeta::new(user_token_out_ai.key(), false),
                    AccountMeta::new_readonly(token_in_mint.key(), false),
                    AccountMeta::new_readonly(token_out_mint.key(), false),
                    AccountMeta::new_readonly(user.key(), true),
                    AccountMeta::new_readonly(token_program.key(), false),
                    AccountMeta::new_readonly(ra[IDX_TOKEN_2022_PROGRAM].key(), false),
                    AccountMeta::new_readonly(event_authority.key(), false),
                    AccountMeta::new_readonly(omnipair_program.key(), false),
                ];
                let mut ix_data = hash(b"global:swap").to_bytes()[..8].to_vec();
                SwapArgs { amount_in: swap_amount_in, min_amount_out }.serialize(&mut ix_data)?;
                invoke(
                    &Instruction { program_id: omnipair_program.key(), accounts, data: ix_data },
                    &[
                        ra[IDX_PAIR].to_account_info(),
                        ra[IDX_RATE_MODEL].to_account_info(),
                        ra[IDX_FUTARCHY].to_account_info(),
                        ra[IDX_TOKEN_IN_VAULT].to_account_info(),
                        ra[IDX_TOKEN_OUT_VAULT].to_account_info(),
                        user_token_in_ai.clone(),
                        user_token_out_ai.clone(),
                        token_in_mint.clone(),
                        token_out_mint.clone(),
                        user.clone(),
                        token_program.clone(),
                        ra[IDX_TOKEN_2022_PROGRAM].to_account_info(),
                        event_authority.to_account_info(),
                        omnipair_program.to_account_info(),
                    ],
                )?;
            }

            // Reload token_out account to get post-swap balance (delta = amount received)
            let amount_out = {
                if token0_in {
                    ctx.accounts.receiver_token1_account.reload()?;
                    ctx.accounts.receiver_token1_account.amount
                        .checked_sub(balance_before_swap)
                        .ok_or(LeverageError::SwapFailed)?
                } else {
                    ctx.accounts.receiver_token0_account.reload()?;
                    ctx.accounts.receiver_token0_account.amount
                        .checked_sub(balance_before_swap)
                        .ok_or(LeverageError::SwapFailed)?
                }
            };
            require!(amount_out > 0, LeverageError::SwapFailed);

            // Write position_size into the leverage position account via raw byte offset.
            // discriminator(8) + owner(32) + pair(32) + is_lev_collateral0(1) +
            // lev_collateral_amount(8) + multiplier_bps(8) = offset 89
            {
                const POSITION_SIZE_OFFSET: usize = 8 + 32 + 32 + 1 + 8 + 8;
                let lev_pos_account = &ra[IDX_USER_LEV_POSITION];
                let mut data = lev_pos_account.try_borrow_mut_data()?;
                data[POSITION_SIZE_OFFSET..POSITION_SIZE_OFFSET + 8]
                    .copy_from_slice(&amount_out.to_le_bytes());
            }

            // 2. Add collateral
            {
                let accounts = vec![
                    AccountMeta::new(ra[IDX_PAIR].key(), false),
                    AccountMeta::new(ra[IDX_USER_POSITION].key(), false),
                    AccountMeta::new(ra[IDX_RATE_MODEL].key(), false),
                    AccountMeta::new_readonly(ra[IDX_FUTARCHY].key(), false),
                    AccountMeta::new(ra[IDX_COLLATERAL_VAULT].key(), false),
                    AccountMeta::new(user_token_out_ai.key(), false),
                    AccountMeta::new_readonly(token_out_mint.key(), false),
                    AccountMeta::new(user.key(), true),
                    AccountMeta::new_readonly(token_program.key(), false),
                    AccountMeta::new_readonly(ra[IDX_TOKEN_2022_PROGRAM].key(), false),
                    AccountMeta::new_readonly(ra[IDX_SYSTEM_PROGRAM].key(), false),
                    AccountMeta::new_readonly(event_authority.key(), false),
                    AccountMeta::new_readonly(omnipair_program.key(), false),
                ];
                let mut ix_data = hash(b"global:add_collateral").to_bytes()[..8].to_vec();
                AdjustCollateralArgs { amount: amount_out }.serialize(&mut ix_data)?;
                invoke(
                    &Instruction { program_id: omnipair_program.key(), accounts, data: ix_data },
                    &[
                        ra[IDX_PAIR].to_account_info(),
                        ra[IDX_USER_POSITION].to_account_info(),
                        ra[IDX_RATE_MODEL].to_account_info(),
                        ra[IDX_FUTARCHY].to_account_info(),
                        ra[IDX_COLLATERAL_VAULT].to_account_info(),
                        user_token_out_ai.clone(),
                        token_out_mint.clone(),
                        user.clone(),
                        token_program.clone(),
                        ra[IDX_TOKEN_2022_PROGRAM].to_account_info(),
                        ra[IDX_SYSTEM_PROGRAM].to_account_info(),
                        event_authority.to_account_info(),
                        omnipair_program.to_account_info(),
                    ],
                )?;
            }

            // 3. Borrow repay_amount of lev_collateral token
            {
                let accounts = vec![
                    AccountMeta::new(ra[IDX_PAIR].key(), false),
                    AccountMeta::new(ra[IDX_USER_POSITION].key(), false),
                    AccountMeta::new(ra[IDX_RATE_MODEL].key(), false),
                    AccountMeta::new_readonly(ra[IDX_FUTARCHY].key(), false),
                    AccountMeta::new(ra[IDX_TOKEN_IN_VAULT].key(), false),
                    AccountMeta::new(user_token_in_ai.key(), false),
                    AccountMeta::new_readonly(token_in_mint.key(), false),
                    AccountMeta::new_readonly(user.key(), true),
                    AccountMeta::new_readonly(token_program.key(), false),
                    AccountMeta::new_readonly(ra[IDX_TOKEN_2022_PROGRAM].key(), false),
                    AccountMeta::new_readonly(ra[IDX_SYSTEM_PROGRAM].key(), false),
                    AccountMeta::new_readonly(event_authority.key(), false),
                    AccountMeta::new_readonly(omnipair_program.key(), false),
                ];
                let mut ix_data = hash(b"global:borrow").to_bytes()[..8].to_vec();
                AdjustDebtArgs { amount: repay_amount }.serialize(&mut ix_data)?;
                invoke(
                    &Instruction { program_id: omnipair_program.key(), accounts, data: ix_data },
                    &[
                        ra[IDX_PAIR].to_account_info(),
                        ra[IDX_USER_POSITION].to_account_info(),
                        ra[IDX_RATE_MODEL].to_account_info(),
                        ra[IDX_FUTARCHY].to_account_info(),
                        ra[IDX_TOKEN_IN_VAULT].to_account_info(),
                        user_token_in_ai.clone(),
                        token_in_mint.clone(),
                        user.clone(),
                        token_program.clone(),
                        ra[IDX_TOKEN_2022_PROGRAM].to_account_info(),
                        ra[IDX_SYSTEM_PROGRAM].to_account_info(),
                        event_authority.to_account_info(),
                        omnipair_program.to_account_info(),
                    ],
                )?;
            }

            // 4. Repay flashloan
            let (repay_vault, repay_mint, repay_decimals) = if is_lev_collateral0 {
                (ctx.accounts.repay0_vault.to_account_info(), ctx.accounts.token0_mint.to_account_info(), ctx.accounts.token0_mint.decimals)
            } else {
                (ctx.accounts.repay1_vault.to_account_info(), ctx.accounts.token1_mint.to_account_info(), ctx.accounts.token1_mint.decimals)
            };
            anchor_spl::token::transfer_checked(
                CpiContext::new(
                    token_program,
                    TransferChecked {
                        from:      user_token_in_ai,
                        mint:      repay_mint,
                        to:        repay_vault,
                        authority: user,
                    },
                ),
                repay_amount,
                repay_decimals,
            )?;
        }

        Ok(())
    }
}

// ── Helpers ───────────────────────────────────────────────────────────────────

/// Validates that remaining_accounts keys match the corresponding validated
/// outer accounts. Prevents a malicious client from substituting different
/// accounts in remaining_accounts while using correct accounts on the outer CPI.
fn validate_remaining_accounts<'info>(
    ra: &[AccountInfo<'info>],
    pair: &AccountInfo<'info>,
    rate_model: &AccountInfo<'info>,
    futarchy: &AccountInfo<'info>,
    reserve0_vault: Pubkey,
    reserve1_vault: Pubkey,
    is_lev_collateral0: bool,
    is_close: bool,
    user_lev_position: Pubkey,
) -> Result<()> {
    require_keys_eq!(ra[IDX_PAIR].key(), pair.key(), LeverageError::RemainingAccountMismatch);
    require_keys_eq!(ra[IDX_RATE_MODEL].key(), rate_model.key(), LeverageError::RemainingAccountMismatch);
    require_keys_eq!(ra[IDX_FUTARCHY].key(), futarchy.key(), LeverageError::RemainingAccountMismatch);
    require_keys_eq!(ra[IDX_OMNIPAIR_PROGRAM].key(), omnipair::ID, LeverageError::RemainingAccountMismatch);
    require_keys_eq!(ra[IDX_USER_LEV_POSITION].key(), user_lev_position, LeverageError::RemainingAccountMismatch);

    // TOKEN_IN/OUT vault direction depends on is_close:
    //   open:  TOKEN_IN = lev_collateral reserve, TOKEN_OUT = position token reserve
    //   close: TOKEN_IN = position token reserve, TOKEN_OUT = lev_collateral reserve
    let (lev_collateral_vault, position_token_vault) = if is_lev_collateral0 {
        (reserve0_vault, reserve1_vault)
    } else {
        (reserve1_vault, reserve0_vault)
    };
    let (expected_in_vault, expected_out_vault) = if is_close {
        (position_token_vault, lev_collateral_vault)
    } else {
        (lev_collateral_vault, position_token_vault)
    };
    require_keys_eq!(ra[IDX_TOKEN_IN_VAULT].key(), expected_in_vault, LeverageError::RemainingAccountMismatch);
    require_keys_eq!(ra[IDX_TOKEN_OUT_VAULT].key(), expected_out_vault, LeverageError::RemainingAccountMismatch);

    Ok(())
}

/// Shared flashloan CPI builder used by `multiply`.
fn invoke_flashloan<'info>(
    accounts: &Multiply<'info>,
    ra: &[AccountInfo<'info>],
    amount0: u64,
    amount1: u64,
    callback_bytes: Vec<u8>,
) -> Result<()> {
    let raw = CloseMultiplyAsFlashloan {
        pair: accounts.pair.to_account_info(),
        rate_model: accounts.rate_model.to_account_info(),
        futarchy_authority: accounts.futarchy_authority.to_account_info(),
        reserve0_vault: accounts.reserve0_vault.to_account_info(),
        reserve1_vault: accounts.reserve1_vault.to_account_info(),
        token0_mint: accounts.token0_mint.to_account_info(),
        token1_mint: accounts.token1_mint.to_account_info(),
        repay0_vault: accounts.repay0_vault.to_account_info(),
        repay1_vault: accounts.repay1_vault.to_account_info(),
        receiver_token0_account: accounts.receiver_token0_account.to_account_info(),
        receiver_token1_account: accounts.receiver_token1_account.to_account_info(),
        receiver_program: accounts.receiver_program.to_account_info(),
        user: accounts.user.to_account_info(),
        token_program: accounts.token_program.to_account_info(),
        token_2022_program: accounts.token_2022_program.to_account_info(),
        system_program: accounts.system_program.to_account_info(),
        user_leverage_position: accounts.user_leverage_position.to_account_info(),
    };
    invoke_flashloan_raw(&raw, ra, amount0, amount1, callback_bytes)
}

/// Account infos for the flashloan CPI, shared between multiply and close_multiply.
struct CloseMultiplyAsFlashloan<'info> {
    pair: AccountInfo<'info>,
    rate_model: AccountInfo<'info>,
    futarchy_authority: AccountInfo<'info>,
    reserve0_vault: AccountInfo<'info>,
    reserve1_vault: AccountInfo<'info>,
    token0_mint: AccountInfo<'info>,
    token1_mint: AccountInfo<'info>,
    repay0_vault: AccountInfo<'info>,
    repay1_vault: AccountInfo<'info>,
    receiver_token0_account: AccountInfo<'info>,
    receiver_token1_account: AccountInfo<'info>,
    receiver_program: AccountInfo<'info>,
    user: AccountInfo<'info>,
    token_program: AccountInfo<'info>,
    token_2022_program: AccountInfo<'info>,
    system_program: AccountInfo<'info>,
    user_leverage_position: AccountInfo<'info>,
}

fn invoke_flashloan_raw<'info>(
    a: &CloseMultiplyAsFlashloan<'info>,
    ra: &[AccountInfo<'info>],
    amount0: u64,
    amount1: u64,
    callback_bytes: Vec<u8>,
) -> Result<()> {
    let omnipair_program = &ra[IDX_OMNIPAIR_PROGRAM];
    let event_authority  = &ra[IDX_EVENT_AUTHORITY];

    let mut flashloan_metas = vec![
        AccountMeta::new(a.pair.key(), false),
        AccountMeta::new(a.rate_model.key(), false),
        AccountMeta::new_readonly(a.futarchy_authority.key(), false),
        AccountMeta::new(a.reserve0_vault.key(), false),
        AccountMeta::new(a.reserve1_vault.key(), false),
        AccountMeta::new_readonly(a.token0_mint.key(), false),
        AccountMeta::new_readonly(a.token1_mint.key(), false),
        AccountMeta::new(a.repay0_vault.key(), false),
        AccountMeta::new(a.repay1_vault.key(), false),
        AccountMeta::new(a.receiver_token0_account.key(), false),
        AccountMeta::new(a.receiver_token1_account.key(), false),
        AccountMeta::new_readonly(a.receiver_program.key(), false),
        AccountMeta::new(a.user.key(), true),
        AccountMeta::new_readonly(a.token_program.key(), false),
        AccountMeta::new_readonly(a.token_2022_program.key(), false),
        AccountMeta::new_readonly(a.system_program.key(), false),
        AccountMeta::new_readonly(event_authority.key(), false),
        AccountMeta::new_readonly(omnipair_program.key(), false),
    ];
    // remaining_accounts[0..11] forwarded verbatim to the callback
    for acc in ra.iter() {
        flashloan_metas.push(AccountMeta {
            pubkey: acc.key(),
            is_signer: acc.is_signer,
            is_writable: acc.is_writable,
        });
    }
    // ra[11] = user_leverage_position is already included above via ra.iter(),
    // but for the flashloan metas we push it explicitly after to keep ordering clear.
    // Actually ra.iter() includes all 12 accounts already (indices 0-11).

    let mut ix_data = hash(b"global:flashloan").to_bytes()[..8].to_vec();
    FlashloanArgs { amount0, amount1, data: callback_bytes }.serialize(&mut ix_data)?;

    let mut account_infos = vec![
        a.pair.clone(),
        a.rate_model.clone(),
        a.futarchy_authority.clone(),
        a.reserve0_vault.clone(),
        a.reserve1_vault.clone(),
        a.token0_mint.clone(),
        a.token1_mint.clone(),
        a.repay0_vault.clone(),
        a.repay1_vault.clone(),
        a.receiver_token0_account.clone(),
        a.receiver_token1_account.clone(),
        a.receiver_program.clone(),
        a.user.clone(),
        a.token_program.clone(),
        a.token_2022_program.clone(),
        a.system_program.clone(),
        event_authority.to_account_info(),
        omnipair_program.to_account_info(),
    ];
    for acc in ra.iter() {
        account_infos.push(acc.to_account_info());
    }

    invoke(
        &Instruction {
            program_id: omnipair_program.key(),
            accounts: flashloan_metas,
            data: ix_data,
        },
        &account_infos,
    )?;
    Ok(())
}

// ── Account structs ───────────────────────────────────────────────────────────

#[derive(Accounts)]
#[instruction(is_lev_collateral0: bool)]
pub struct Multiply<'info> {
    /// CHECK: validated by omnipair
    #[account(mut)]
    pub pair: UncheckedAccount<'info>,
    /// CHECK: validated by omnipair
    #[account(mut)]
    pub rate_model: UncheckedAccount<'info>,
    /// CHECK: validated by omnipair
    pub futarchy_authority: UncheckedAccount<'info>,

    /// CHECK: validated by omnipair
    #[account(mut)]
    pub reserve0_vault: UncheckedAccount<'info>,
    /// CHECK: validated by omnipair
    #[account(mut)]
    pub reserve1_vault: UncheckedAccount<'info>,

    /// CHECK: validated by omnipair
    pub token0_mint: UncheckedAccount<'info>,
    /// CHECK: validated by omnipair
    pub token1_mint: UncheckedAccount<'info>,

    /// CHECK: PDA created and closed by omnipair flashloan
    #[account(mut)]
    pub repay0_vault: UncheckedAccount<'info>,
    /// CHECK: PDA created and closed by omnipair flashloan
    #[account(mut)]
    pub repay1_vault: UncheckedAccount<'info>,

    #[account(mut, token::authority = user)]
    pub receiver_token0_account: Account<'info, TokenAccount>,
    #[account(mut, token::authority = user)]
    pub receiver_token1_account: Account<'info, TokenAccount>,

    /// CHECK: must be this program's own ID
    #[account(address = ID)]
    pub receiver_program: UncheckedAccount<'info>,

    /// Leverage position PDA for this (pair, user, side).
    /// Fails if already exists — close the position first.
    #[account(
        init,
        payer = user,
        space = 8 + UserLeveragePosition::INIT_SPACE,
        seeds = [LEVERAGE_POSITION_SEED_PREFIX, pair.key().as_ref(), user.key().as_ref(), &[is_lev_collateral0 as u8]],
        bump,
    )]
    pub user_leverage_position: Account<'info, UserLeveragePosition>,

    #[account(mut)]
    pub user: Signer<'info>,
    pub token_program: Program<'info, Token>,
    /// CHECK: SPL Token-2022 program
    pub token_2022_program: UncheckedAccount<'info>,
    pub system_program: Program<'info, System>,
    // remaining_accounts[0..11]: see layout constants at top of file
}

#[derive(Accounts)]
#[instruction(is_lev_collateral0: bool)]
pub struct CloseMultiply<'info> {
    /// CHECK: validated by omnipair
    #[account(mut)]
    pub pair: UncheckedAccount<'info>,
    /// CHECK: validated by omnipair
    #[account(mut)]
    pub rate_model: UncheckedAccount<'info>,
    /// CHECK: validated by omnipair
    pub futarchy_authority: UncheckedAccount<'info>,

    /// CHECK: validated by omnipair
    #[account(mut)]
    pub reserve0_vault: UncheckedAccount<'info>,
    /// CHECK: validated by omnipair
    #[account(mut)]
    pub reserve1_vault: UncheckedAccount<'info>,

    /// CHECK: validated by omnipair
    pub token0_mint: UncheckedAccount<'info>,
    /// CHECK: validated by omnipair
    pub token1_mint: UncheckedAccount<'info>,

    /// CHECK: PDA created and closed by omnipair flashloan
    #[account(mut)]
    pub repay0_vault: UncheckedAccount<'info>,
    /// CHECK: PDA created and closed by omnipair flashloan
    #[account(mut)]
    pub repay1_vault: UncheckedAccount<'info>,

    #[account(mut, token::authority = user)]
    pub receiver_token0_account: Account<'info, TokenAccount>,
    #[account(mut, token::authority = user)]
    pub receiver_token1_account: Account<'info, TokenAccount>,

    /// CHECK: must be this program's own ID
    #[account(address = ID)]
    pub receiver_program: UncheckedAccount<'info>,

    /// Leverage position to close. Rent is returned to user.
    #[account(
        mut,
        close = user,
        seeds = [LEVERAGE_POSITION_SEED_PREFIX, pair.key().as_ref(), user.key().as_ref(), &[is_lev_collateral0 as u8]],
        bump = user_leverage_position.bump,
    )]
    pub user_leverage_position: Account<'info, UserLeveragePosition>,

    #[account(mut)]
    pub user: Signer<'info>,
    pub token_program: Program<'info, Token>,
    /// CHECK: SPL Token-2022 program
    pub token_2022_program: UncheckedAccount<'info>,
    pub system_program: Program<'info, System>,
    // remaining_accounts[0..11]: see layout constants at top of file
}

/// Accounts for the flashloan callback.
/// Field order must match exactly what omnipair's flashloan passes to the receiver program:
///   initiator, receiver_token0_account, receiver_token1_account,
///   token0_mint, token1_mint, repay0_vault, repay1_vault,
///   [remaining_accounts forwarded from multiply/close_multiply],
///   token_program  ← last, appended by omnipair
#[derive(Accounts)]
pub struct FlashLoanCallback<'info> {
    pub initiator: Signer<'info>,

    #[account(mut)]
    pub receiver_token0_account: Account<'info, TokenAccount>,
    #[account(mut)]
    pub receiver_token1_account: Account<'info, TokenAccount>,

    pub token0_mint: Account<'info, Mint>,
    pub token1_mint: Account<'info, Mint>,

    #[account(mut)]
    pub repay0_vault: Account<'info, TokenAccount>,
    #[account(mut)]
    pub repay1_vault: Account<'info, TokenAccount>,

    // remaining_accounts[0..11] accessed via ctx.remaining_accounts
    // token_program is the LAST account (appended by omnipair after remaining_accounts)
    pub token_program: Program<'info, Token>,
}

// ── Errors ────────────────────────────────────────────────────────────────────

#[error_code]
pub enum LeverageError {
    #[msg("Amount must be greater than zero")]
    AmountZero,
    #[msg("Multiplier must be > 1x (multiplier_bps > 10_000)")]
    MultiplierTooLow,
    #[msg("max_slippage_bps must be <= 10_000")]
    InvalidSlippage,
    #[msg("Arithmetic overflow")]
    Overflow,
    #[msg("Pair has no liquidity")]
    InsufficientLiquidity,
    #[msg("Failed to decode internal callback data")]
    InvalidCallbackData,
    #[msg("Swap returned zero tokens")]
    SwapFailed,
    #[msg("No open debt found — position may already be closed or never opened")]
    PositionNotOpen,
    #[msg("Expected 12 remaining_accounts (pair..user_leverage_position)")]
    MissingRemainingAccounts,
    #[msg("remaining_accounts key does not match the corresponding validated account")]
    RemainingAccountMismatch,
}
