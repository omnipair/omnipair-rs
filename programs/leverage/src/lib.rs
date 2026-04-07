use anchor_lang::prelude::*;
use anchor_lang::solana_program::{
    instruction::{AccountMeta, Instruction},
    program::invoke,
    hash::hash,
};
use anchor_spl::token::{Token, TokenAccount, Mint, TransferChecked};
use omnipair::{
    FlashLoanCallbackData, FlashloanArgs, SwapArgs, AdjustCollateralArgs, AdjustDebtArgs,
    state::Pair,
    ceil_div,
};

declare_id!("Fg6PaFpoGXkYsidMpWTK6W2BeZ7FEfcYkg476zPFsLnS");

// ── Remaining-accounts layout (passed from multiply → flashloan → flash_loan_callback) ──
//
// When `multiply` calls omnipair's `flashloan`, it appends these accounts to
// the flashloan's remaining_accounts. omnipair forwards them verbatim to the
// `flash_loan_callback`, where they appear as ctx.remaining_accounts[0..10].
//
// Index  Account                   Writable
// 0      pair                       yes
// 1      rate_model                 yes
// 2      futarchy_authority         no
// 3      user_position              yes   (init_if_needed inside add_collateral)
// 4      token_in_reserve_vault     yes   (reserve vault of the lev-collateral token)
// 5      token_out_reserve_vault    yes   (reserve vault of the position token)
// 6      collateral_out_vault       yes   (collateral vault of the position token)
// 7      token_2022_program         no
// 8      system_program             no
// 9      event_authority            no    (omnipair's __event_authority PDA)
// 10     omnipair_program           no
const IDX_PAIR: usize = 0;
const IDX_RATE_MODEL: usize = 1;
const IDX_FUTARCHY: usize = 2;
const IDX_USER_POSITION: usize = 3;
const IDX_TOKEN_IN_VAULT: usize = 4;
const IDX_TOKEN_OUT_VAULT: usize = 5;
const IDX_COLLATERAL_OUT_VAULT: usize = 6;
const IDX_TOKEN_2022_PROGRAM: usize = 7;
const IDX_SYSTEM_PROGRAM: usize = 8;
const IDX_EVENT_AUTHORITY: usize = 9;
const IDX_OMNIPAIR_PROGRAM: usize = 10;

const BPS_DENOMINATOR: u64 = 10_000;
const FLASHLOAN_FEE_BPS: u64 = 5;

/// Params encoded into the flashloan `data` bytes and decoded in the callback.
#[derive(AnchorSerialize, AnchorDeserialize, Clone)]
pub struct InternalCallbackData {
    pub is_lev_collateral0: bool,
    /// swap_amount_in = lev_collateral_amount * multiplier_bps / BPS
    pub swap_amount_in: u64,
    /// Minimum position token output enforced inside the swap CPI
    pub min_amount_out: u64,
    /// Principal + flashloan fee that must be returned to the repay vault
    pub repay_amount: u64,
}

#[program]
pub mod omnipair_leverage {
    use super::*;

    /// Entry point called by the user.
    ///
    /// Leverages `lev_collateral_amount` of lev-collateral token by `multiplier_bps / 10_000`.
    /// Internally flashloans the borrow portion from omnipair, swaps everything into the
    /// position token, deposits it as collateral, borrows to repay the flashloan — all in
    /// one atomic transaction via the `flash_loan_callback` below.
    ///
    /// - `is_lev_collateral0`: true ↔ token0 is being leveraged (long token0/short token1)
    /// - `multiplier_bps`: leverage in BPS (20_000 = 2×, 15_000 = 1.5×); must be > 10_000
    /// - `max_slippage_bps`: max deviation from zero-impact spot price (10_000 = no check)
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
        require!(ra.len() >= 11, LeverageError::MissingRemainingAccounts);

        // ── tie remaining_accounts to the validated outer accounts ─────────
        // Prevents a malicious/buggy client from passing a different pair in
        // remaining_accounts[0] (used for reserve reads and callback CPIs)
        // while using the correct pair on the outer flashloan.
        require_keys_eq!(
            ra[IDX_PAIR].key(), ctx.accounts.pair.key(),
            LeverageError::RemainingAccountMismatch
        );
        require_keys_eq!(
            ra[IDX_RATE_MODEL].key(), ctx.accounts.rate_model.key(),
            LeverageError::RemainingAccountMismatch
        );
        require_keys_eq!(
            ra[IDX_FUTARCHY].key(), ctx.accounts.futarchy_authority.key(),
            LeverageError::RemainingAccountMismatch
        );
        // token_in/out vaults must match the correct reserve vault by direction
        let (expected_in_vault, expected_out_vault) = if is_lev_collateral0 {
            (ctx.accounts.reserve0_vault.key(), ctx.accounts.reserve1_vault.key())
        } else {
            (ctx.accounts.reserve1_vault.key(), ctx.accounts.reserve0_vault.key())
        };
        require_keys_eq!(
            ra[IDX_TOKEN_IN_VAULT].key(), expected_in_vault,
            LeverageError::RemainingAccountMismatch
        );
        require_keys_eq!(
            ra[IDX_TOKEN_OUT_VAULT].key(), expected_out_vault,
            LeverageError::RemainingAccountMismatch
        );
        require_keys_eq!(
            ra[IDX_OMNIPAIR_PROGRAM].key(), omnipair::ID,
            LeverageError::RemainingAccountMismatch
        );

        // ── compute amounts ────────────────────────────────────────────────
        let swap_amount_in: u64 = (lev_collateral_amount as u128)
            .checked_mul(multiplier_bps as u128).ok_or(LeverageError::Overflow)?
            .checked_div(BPS_DENOMINATOR as u128).ok_or(LeverageError::Overflow)?
            .try_into().map_err(|_| LeverageError::Overflow)?;

        let borrow_amount = swap_amount_in
            .checked_sub(lev_collateral_amount).ok_or(LeverageError::Overflow)?;

        // Flashloan fee is on the principal borrowed
        let flashloan_fee = ceil_div(
            (borrow_amount as u128)
                .checked_mul(FLASHLOAN_FEE_BPS as u128).ok_or(LeverageError::Overflow)?,
            BPS_DENOMINATOR as u128,
        ).ok_or(LeverageError::Overflow)? as u64;

        let repay_amount = borrow_amount
            .checked_add(flashloan_fee).ok_or(LeverageError::Overflow)?;

        // ── slippage: compute spot-price reference ─────────────────────────
        let pair_data = {
            let pair_account = &ra[IDX_PAIR];
            Pair::try_deserialize(&mut &**pair_account.try_borrow_data()?)?
        };

        let (reserve_in, reserve_out) = if is_lev_collateral0 {
            (pair_data.reserve0, pair_data.reserve1)
        } else {
            (pair_data.reserve1, pair_data.reserve0)
        };
        require!(reserve_in > 0 && reserve_out > 0, LeverageError::InsufficientLiquidity);

        // spot_out = lev_collateral * multiplier * reserve_out / (reserve_in * BPS)
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

        // ── encode callback params ─────────────────────────────────────────
        let mut callback_bytes = Vec::new();
        InternalCallbackData {
            is_lev_collateral0,
            swap_amount_in,
            min_amount_out,
            repay_amount,
        }.serialize(&mut callback_bytes)?;

        // ── CPI → omnipair flashloan ───────────────────────────────────────
        let (amount0, amount1) = if is_lev_collateral0 {
            (borrow_amount, 0u64)
        } else {
            (0u64, borrow_amount)
        };

        let omnipair_program = &ra[IDX_OMNIPAIR_PROGRAM];
        let event_authority  = &ra[IDX_EVENT_AUTHORITY];

        // Account order must match omnipair's Flashloan accounts struct (+ event_cpi additions)
        let mut flashloan_metas = vec![
            AccountMeta::new(ctx.accounts.pair.key(), false),
            AccountMeta::new(ctx.accounts.rate_model.key(), false),
            AccountMeta::new_readonly(ctx.accounts.futarchy_authority.key(), false),
            AccountMeta::new(ctx.accounts.reserve0_vault.key(), false),
            AccountMeta::new(ctx.accounts.reserve1_vault.key(), false),
            AccountMeta::new_readonly(ctx.accounts.token0_mint.key(), false),
            AccountMeta::new_readonly(ctx.accounts.token1_mint.key(), false),
            AccountMeta::new(ctx.accounts.repay0_vault.key(), false),
            AccountMeta::new(ctx.accounts.repay1_vault.key(), false),
            AccountMeta::new(ctx.accounts.receiver_token0_account.key(), false),
            AccountMeta::new(ctx.accounts.receiver_token1_account.key(), false),
            AccountMeta::new_readonly(ctx.accounts.receiver_program.key(), false),
            AccountMeta::new(ctx.accounts.user.key(), true),
            AccountMeta::new_readonly(ctx.accounts.token_program.key(), false),
            AccountMeta::new_readonly(ctx.accounts.token_2022_program.key(), false),
            AccountMeta::new_readonly(ctx.accounts.system_program.key(), false),
            AccountMeta::new_readonly(event_authority.key(), false),
            AccountMeta::new_readonly(omnipair_program.key(), false),
        ];
        // Append remaining_accounts — omnipair forwards these verbatim to the callback
        for acc in ra.iter() {
            flashloan_metas.push(AccountMeta {
                pubkey: acc.key(),
                is_signer: acc.is_signer,
                is_writable: acc.is_writable,
            });
        }

        let discriminator = &hash(b"global:flashloan").to_bytes()[..8];
        let mut ix_data = discriminator.to_vec();
        FlashloanArgs { amount0, amount1, data: callback_bytes }.serialize(&mut ix_data)?;

        let mut account_infos = vec![
            ctx.accounts.pair.to_account_info(),
            ctx.accounts.rate_model.to_account_info(),
            ctx.accounts.futarchy_authority.to_account_info(),
            ctx.accounts.reserve0_vault.to_account_info(),
            ctx.accounts.reserve1_vault.to_account_info(),
            ctx.accounts.token0_mint.to_account_info(),
            ctx.accounts.token1_mint.to_account_info(),
            ctx.accounts.repay0_vault.to_account_info(),
            ctx.accounts.repay1_vault.to_account_info(),
            ctx.accounts.receiver_token0_account.to_account_info(),
            ctx.accounts.receiver_token1_account.to_account_info(),
            ctx.accounts.receiver_program.to_account_info(),
            ctx.accounts.user.to_account_info(),
            ctx.accounts.token_program.to_account_info(),
            ctx.accounts.token_2022_program.to_account_info(),
            ctx.accounts.system_program.to_account_info(),
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

    /// Called by omnipair mid-flashloan.  Never called directly.
    ///
    /// Strategy:
    ///   1. swap   lev_collateral + borrow_amount → position token
    ///   2. add_collateral   deposit position token into omnipair
    ///   3. borrow           borrow repay_amount of lev-collateral token
    ///   4. repay            return principal + fee to omnipair's repay vault
    pub fn flash_loan_callback<'info>(
        ctx: Context<'_, '_, '_, 'info, FlashLoanCallback<'info>>,
        callback_data: FlashLoanCallbackData,
    ) -> Result<()> {
        let InternalCallbackData {
            is_lev_collateral0,
            swap_amount_in,
            min_amount_out,
            repay_amount,
        } = InternalCallbackData::try_from_slice(&callback_data.data)
            .map_err(|_| LeverageError::InvalidCallbackData)?;

        let ra  = ctx.remaining_accounts;
        let omnipair_program = &ra[IDX_OMNIPAIR_PROGRAM];
        let event_authority  = &ra[IDX_EVENT_AUTHORITY];

        let user         = ctx.accounts.initiator.to_account_info();
        let token_program = ctx.accounts.token_program.to_account_info();

        // Identify in/out token accounts and mints by direction
        let (token_in_mint, token_out_mint, user_token_in_ai, user_token_out_ai) =
            if is_lev_collateral0 {
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

        // Record token_out balance before the swap so we can compute the exact delta
        let balance_before_swap = if is_lev_collateral0 {
            ctx.accounts.receiver_token1_account.amount
        } else {
            ctx.accounts.receiver_token0_account.amount
        };

        // ── 1. swap: token_in → token_out ────────────────────────────────────
        {
            let swap_accounts = vec![
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

            let discriminator = &hash(b"global:swap").to_bytes()[..8];
            let mut ix_data = discriminator.to_vec();
            SwapArgs { amount_in: swap_amount_in, min_amount_out }.serialize(&mut ix_data)?;

            invoke(
                &Instruction { program_id: omnipair_program.key(), accounts: swap_accounts, data: ix_data },
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

        // ── 2. add_collateral: deposit position token ─────────────────────────
        // Reload the token_out account to get the post-swap balance, then
        // deposit only the newly received amount (delta from the swap).
        let amount_out = {
            if is_lev_collateral0 {
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

        {
            let add_collateral_accounts = vec![
                AccountMeta::new(ra[IDX_PAIR].key(), false),
                AccountMeta::new(ra[IDX_RATE_MODEL].key(), false),
                AccountMeta::new_readonly(ra[IDX_FUTARCHY].key(), false),
                AccountMeta::new(ra[IDX_USER_POSITION].key(), false),
                AccountMeta::new(ra[IDX_COLLATERAL_OUT_VAULT].key(), false),
                AccountMeta::new(user_token_out_ai.key(), false),
                AccountMeta::new_readonly(token_out_mint.key(), false),
                AccountMeta::new(user.key(), true),
                AccountMeta::new_readonly(token_program.key(), false),
                AccountMeta::new_readonly(ra[IDX_TOKEN_2022_PROGRAM].key(), false),
                AccountMeta::new_readonly(ra[IDX_SYSTEM_PROGRAM].key(), false),
                AccountMeta::new_readonly(event_authority.key(), false),
                AccountMeta::new_readonly(omnipair_program.key(), false),
            ];

            let discriminator = &hash(b"global:add_collateral").to_bytes()[..8];
            let mut ix_data = discriminator.to_vec();
            AdjustCollateralArgs { amount: amount_out }.serialize(&mut ix_data)?;

            invoke(
                &Instruction { program_id: omnipair_program.key(), accounts: add_collateral_accounts, data: ix_data },
                &[
                    ra[IDX_PAIR].to_account_info(),
                    ra[IDX_RATE_MODEL].to_account_info(),
                    ra[IDX_FUTARCHY].to_account_info(),
                    ra[IDX_USER_POSITION].to_account_info(),
                    ra[IDX_COLLATERAL_OUT_VAULT].to_account_info(),
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

        // ── 3. borrow: get repay_amount of lev-collateral token ───────────────
        // Position token is now collateral — we can borrow against it.
        {
            let borrow_accounts = vec![
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

            let discriminator = &hash(b"global:borrow").to_bytes()[..8];
            let mut ix_data = discriminator.to_vec();
            AdjustDebtArgs { amount: repay_amount }.serialize(&mut ix_data)?;

            invoke(
                &Instruction { program_id: omnipair_program.key(), accounts: borrow_accounts, data: ix_data },
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

        // ── 4. repay flashloan ────────────────────────────────────────────────
        // The borrow CPI just deposited repay_amount into user's token_in account.
        // Transfer it to the omnipair repay vault to satisfy the flashloan check.
        let (repay_vault, repay_mint) = if is_lev_collateral0 {
            (
                ctx.accounts.repay0_vault.to_account_info(),
                ctx.accounts.token0_mint.to_account_info(),
            )
        } else {
            (
                ctx.accounts.repay1_vault.to_account_info(),
                ctx.accounts.token1_mint.to_account_info(),
            )
        };

        let token_in_decimals = if is_lev_collateral0 {
            ctx.accounts.token0_mint.decimals
        } else {
            ctx.accounts.token1_mint.decimals
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
            token_in_decimals,
        )?;

        Ok(())
    }
}

// ── Account structs ───────────────────────────────────────────────────────────

#[derive(Accounts)]
pub struct Multiply<'info> {
    // omnipair pair state
    /// CHECK: validated by omnipair
    #[account(mut)]
    pub pair: UncheckedAccount<'info>,
    /// CHECK: validated by omnipair
    #[account(mut)]
    pub rate_model: UncheckedAccount<'info>,
    /// CHECK: validated by omnipair
    pub futarchy_authority: UncheckedAccount<'info>,

    // reserve vaults
    /// CHECK: validated by omnipair
    #[account(mut)]
    pub reserve0_vault: UncheckedAccount<'info>,
    /// CHECK: validated by omnipair
    #[account(mut)]
    pub reserve1_vault: UncheckedAccount<'info>,

    // mints
    /// CHECK: validated by omnipair
    pub token0_mint: UncheckedAccount<'info>,
    /// CHECK: validated by omnipair
    pub token1_mint: UncheckedAccount<'info>,

    // temporary repay vaults — omnipair's flashloan creates and closes these
    /// CHECK: PDA created by omnipair flashloan
    #[account(mut)]
    pub repay0_vault: UncheckedAccount<'info>,
    /// CHECK: PDA created by omnipair flashloan
    #[account(mut)]
    pub repay1_vault: UncheckedAccount<'info>,

    // user token accounts — borrowed tokens land here; user signs all sub-CPIs
    #[account(mut, token::authority = user)]
    pub receiver_token0_account: Account<'info, TokenAccount>,
    #[account(mut, token::authority = user)]
    pub receiver_token1_account: Account<'info, TokenAccount>,

    // leverage program is the flashloan receiver
    /// CHECK: must be this program's own ID
    #[account(address = ID)]
    pub receiver_program: UncheckedAccount<'info>,

    #[account(mut)]
    pub user: Signer<'info>,
    pub token_program: Program<'info, Token>,
    /// CHECK: SPL Token-2022 program
    pub token_2022_program: UncheckedAccount<'info>,
    pub system_program: Program<'info, System>,
    // remaining_accounts[0..10]: see layout constants
}

/// Accounts for the flashloan callback.
/// Field order must match exactly what omnipair's flashloan passes to the receiver program:
///   initiator, receiver_token0_account, receiver_token1_account,
///   token0_mint, token1_mint, repay0_vault, repay1_vault,
///   [remaining_accounts forwarded from multiply],
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

    // remaining_accounts[0..10] are accessed via ctx.remaining_accounts
    // token_program is the LAST account (appended by omnipair after remaining_accounts)
    pub token_program: Program<'info, Token>,
}

// ── Errors ───────────────────────────────────────────────────────────────────

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
    #[msg("Slippage tolerance exceeded")]
    SlippageExceeded,
    #[msg("Failed to decode internal callback data")]
    InvalidCallbackData,
    #[msg("Swap returned zero tokens")]
    SwapFailed,
    #[msg("Expected 11 remaining_accounts (pair..omnipair_program)")]
    MissingRemainingAccounts,
    #[msg("remaining_accounts key does not match the corresponding validated account")]
    RemainingAccountMismatch,
}
