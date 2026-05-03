use anchor_lang::{prelude::*, solana_program::program::set_return_data};
use anchor_spl::token::{self, Mint, Token, TokenAccount, TransferChecked};
use omnipair::{
    constants::{BPS_DENOMINATOR, NAD},
    instructions::{LeverageDelegationApproval, LEVERAGE_DELEGATE_CLOSE},
    state::{Pair, UserLeverageDelegation, UserLeveragePosition},
    utils::{gamm_math::CPCurve, math::ceil_div},
};
use std::cmp::min;

declare_id!("EPGF9iFrbGnhWgC3To9rC9vxinEYuDHaz4RXgLPvuRkp");

pub const ORDER_SEED_PREFIX: &[u8] = b"leverage_order";
pub const CUSTODY_AUTHORITY_SEED_PREFIX: &[u8] = b"leverage_delegate_authority";
pub const EXECUTOR_INCENTIVE_BPS: u64 = 500;
pub const ORDER_KIND_TAKE_PROFIT: u8 = 1;
pub const ORDER_KIND_STOP_LOSS: u8 = 2;

#[program]
pub mod leverage_delegate {
    use super::*;

    pub fn create_leverage_order(
        ctx: Context<CreateLeverageOrder>,
        args: CreateLeverageOrderArgs,
    ) -> Result<()> {
        CreateLeverageOrder::handle_create(ctx, args)
    }

    pub fn update_leverage_order(
        ctx: Context<UpdateLeverageOrder>,
        args: UpdateLeverageOrderArgs,
    ) -> Result<()> {
        UpdateLeverageOrder::handle_update(ctx, args)
    }

    pub fn cancel_leverage_order(
        ctx: Context<CancelLeverageOrder>,
        _args: CancelLeverageOrderArgs,
    ) -> Result<()> {
        CancelLeverageOrder::handle_cancel(ctx)
    }

    pub fn before_take_profit(ctx: Context<BeforeLeverageOrder>, args: ExecuteOrderArgs) -> Result<()> {
        BeforeLeverageOrder::handle_before(ctx, args, ORDER_KIND_TAKE_PROFIT)
    }

    pub fn before_stop_loss(ctx: Context<BeforeLeverageOrder>, args: ExecuteOrderArgs) -> Result<()> {
        BeforeLeverageOrder::handle_before(ctx, args, ORDER_KIND_STOP_LOSS)
    }

    pub fn after_close_order(ctx: Context<AfterCloseOrder>, args: ExecuteOrderArgs) -> Result<()> {
        AfterCloseOrder::handle_after(ctx, args)
    }
}

#[derive(AnchorSerialize, AnchorDeserialize, Clone)]
pub struct CreateLeverageOrderArgs {
    pub order_id: u64,
    pub kind: u8,
    pub trigger_closeout_price_nad: u64,
}

#[derive(AnchorSerialize, AnchorDeserialize, Clone)]
pub struct UpdateLeverageOrderArgs {
    pub order_id: u64,
    pub kind: u8,
    pub trigger_closeout_price_nad: u64,
}

#[derive(AnchorSerialize, AnchorDeserialize, Clone)]
pub struct CancelLeverageOrderArgs {
    pub order_id: u64,
}

#[derive(AnchorSerialize, AnchorDeserialize, Clone)]
pub struct ExecuteOrderArgs {
    pub order_id: u64,
}

#[account]
#[derive(InitSpace)]
pub struct LeverageOrder {
    pub owner: Pubkey,
    pub pair: Pubkey,
    pub position: Pubkey,
    pub order_id: u64,
    pub kind: u8,
    pub trigger_closeout_price_nad: u64,
    pub staged_margin: u64,
    pub staged_custody_token_account: Pubkey,
    pub staged_output_mint: Pubkey,
    pub staged_output_amount: u64,
    pub bump: u8,
}

#[derive(Accounts)]
#[instruction(args: CreateLeverageOrderArgs)]
pub struct CreateLeverageOrder<'info> {
    pub pair: Account<'info, Pair>,
    #[account(
        constraint = user_leverage_position.owner == owner.key() @ LeverageDelegateError::InvalidOrder,
        constraint = user_leverage_position.pair == pair.key() @ LeverageDelegateError::InvalidOrder,
    )]
    pub user_leverage_position: Account<'info, UserLeveragePosition>,
    #[account(
        init,
        payer = owner,
        space = 8 + LeverageOrder::INIT_SPACE,
        seeds = [
            ORDER_SEED_PREFIX,
            user_leverage_position.key().as_ref(),
            owner.key().as_ref(),
            &args.order_id.to_le_bytes(),
        ],
        bump
    )]
    pub order: Account<'info, LeverageOrder>,
    #[account(mut)]
    pub owner: Signer<'info>,
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
#[instruction(args: UpdateLeverageOrderArgs)]
pub struct UpdateLeverageOrder<'info> {
    pub pair: Account<'info, Pair>,
    #[account(
        constraint = user_leverage_position.owner == owner.key() @ LeverageDelegateError::InvalidOrder,
        constraint = user_leverage_position.pair == pair.key() @ LeverageDelegateError::InvalidOrder,
    )]
    pub user_leverage_position: Account<'info, UserLeveragePosition>,
    #[account(
        mut,
        seeds = [
            ORDER_SEED_PREFIX,
            user_leverage_position.key().as_ref(),
            owner.key().as_ref(),
            &args.order_id.to_le_bytes(),
        ],
        bump = order.bump,
        constraint = order.owner == owner.key() @ LeverageDelegateError::InvalidOrder,
        constraint = order.pair == pair.key() @ LeverageDelegateError::InvalidOrder,
        constraint = order.position == user_leverage_position.key() @ LeverageDelegateError::InvalidOrder,
    )]
    pub order: Account<'info, LeverageOrder>,
    #[account(mut)]
    pub owner: Signer<'info>,
}

#[derive(Accounts)]
#[instruction(args: CancelLeverageOrderArgs)]
pub struct CancelLeverageOrder<'info> {
    #[account(
        mut,
        close = owner,
        seeds = [
            ORDER_SEED_PREFIX,
            order.position.as_ref(),
            owner.key().as_ref(),
            &args.order_id.to_le_bytes(),
        ],
        bump = order.bump,
        constraint = order.owner == owner.key() @ LeverageDelegateError::InvalidOrder,
    )]
    pub order: Account<'info, LeverageOrder>,
    #[account(mut)]
    pub owner: Signer<'info>,
}

#[derive(Accounts)]
#[instruction(args: ExecuteOrderArgs)]
pub struct BeforeLeverageOrder<'info> {
    #[account(
        mut,
        seeds = [
            ORDER_SEED_PREFIX,
            user_leverage_position.key().as_ref(),
            order.owner.as_ref(),
            &args.order_id.to_le_bytes(),
        ],
        bump = order.bump,
        constraint = order.pair == pair.key() @ LeverageDelegateError::InvalidOrder,
        constraint = order.position == user_leverage_position.key() @ LeverageDelegateError::InvalidOrder,
    )]
    pub order: Account<'info, LeverageOrder>,
    pub pair: Account<'info, Pair>,
    #[account(
        constraint = user_leverage_position.owner == order.owner @ LeverageDelegateError::InvalidOrder,
        constraint = user_leverage_position.pair == pair.key() @ LeverageDelegateError::InvalidOrder,
    )]
    pub user_leverage_position: Account<'info, UserLeveragePosition>,
    #[account(
        constraint = user_leverage_delegation.owner == order.owner @ LeverageDelegateError::InvalidOrder,
        constraint = user_leverage_delegation.pair == pair.key() @ LeverageDelegateError::InvalidOrder,
        constraint = user_leverage_delegation.position == user_leverage_position.key() @ LeverageDelegateError::InvalidOrder,
        constraint = user_leverage_delegation.is_debt_token0 == user_leverage_position.is_debt_token0 @ LeverageDelegateError::InvalidOrder,
        constraint = user_leverage_delegation.delegated_program == crate::ID @ LeverageDelegateError::InvalidOrder,
    )]
    pub user_leverage_delegation: Account<'info, UserLeverageDelegation>,
    /// CHECK: PDA authority for the custody token account approved as the close recipient.
    #[account(
        seeds = [CUSTODY_AUTHORITY_SEED_PREFIX, order.key().as_ref()],
        bump
    )]
    pub custody_authority: AccountInfo<'info>,
    #[account(
        constraint = custody_token_account.owner == custody_authority.key() @ LeverageDelegateError::InvalidTokenAccount,
        constraint = custody_token_account.mint == token_mint.key() @ LeverageDelegateError::InvalidTokenAccount,
    )]
    pub custody_token_account: Account<'info, TokenAccount>,
    pub token_mint: Account<'info, Mint>,
    pub executor: Signer<'info>,
}

#[derive(Accounts)]
#[instruction(args: ExecuteOrderArgs)]
pub struct AfterCloseOrder<'info> {
    #[account(
        mut,
        close = owner,
        seeds = [
            ORDER_SEED_PREFIX,
            order.position.as_ref(),
            order.owner.as_ref(),
            &args.order_id.to_le_bytes(),
        ],
        bump = order.bump,
    )]
    pub order: Account<'info, LeverageOrder>,
    /// CHECK: Order owner receives closed account rent and residual funds.
    #[account(mut, address = order.owner)]
    pub owner: AccountInfo<'info>,
    #[account(
        constraint = user_leverage_position.key() == order.position @ LeverageDelegateError::InvalidOrder,
        constraint = user_leverage_position.owner == order.owner @ LeverageDelegateError::InvalidOrder,
        constraint = user_leverage_position.pair == order.pair @ LeverageDelegateError::InvalidOrder,
    )]
    pub user_leverage_position: Account<'info, UserLeveragePosition>,
    /// CHECK: PDA authority for the custody token account.
    #[account(
        seeds = [CUSTODY_AUTHORITY_SEED_PREFIX, order.key().as_ref()],
        bump
    )]
    pub custody_authority: AccountInfo<'info>,
    #[account(
        mut,
        token::authority = custody_authority,
        token::mint = token_mint,
        constraint = custody_token_account.key() == order.staged_custody_token_account @ LeverageDelegateError::InvalidTokenAccount,
    )]
    pub custody_token_account: Account<'info, TokenAccount>,
    #[account(
        mut,
        constraint = executor_token_account.mint == token_mint.key() @ LeverageDelegateError::InvalidTokenAccount,
    )]
    pub executor_token_account: Account<'info, TokenAccount>,
    #[account(
        mut,
        constraint = owner_token_account.mint == token_mint.key() @ LeverageDelegateError::InvalidTokenAccount,
        constraint = owner_token_account.owner == owner.key() @ LeverageDelegateError::InvalidTokenAccount,
    )]
    pub owner_token_account: Account<'info, TokenAccount>,
    #[account(
        constraint = token_mint.key() == order.staged_output_mint @ LeverageDelegateError::InvalidTokenAccount,
    )]
    pub token_mint: Account<'info, Mint>,
    pub executor: Signer<'info>,
    pub token_program: Program<'info, Token>,
}

impl<'info> CreateLeverageOrder<'info> {
    pub fn handle_create(ctx: Context<Self>, args: CreateLeverageOrderArgs) -> Result<()> {
        validate_order_kind(args.kind)?;
        require!(args.trigger_closeout_price_nad > 0, LeverageDelegateError::InvalidOrder);
        let order = &mut ctx.accounts.order;
        order.owner = ctx.accounts.owner.key();
        order.pair = ctx.accounts.pair.key();
        order.position = ctx.accounts.user_leverage_position.key();
        order.order_id = args.order_id;
        order.kind = args.kind;
        order.trigger_closeout_price_nad = args.trigger_closeout_price_nad;
        reset_staged_settlement(order);
        order.bump = ctx.bumps.order;
        Ok(())
    }
}

impl<'info> UpdateLeverageOrder<'info> {
    pub fn handle_update(ctx: Context<Self>, args: UpdateLeverageOrderArgs) -> Result<()> {
        validate_order_kind(args.kind)?;
        require!(args.trigger_closeout_price_nad > 0, LeverageDelegateError::InvalidOrder);
        let order = &mut ctx.accounts.order;
        order.kind = args.kind;
        order.trigger_closeout_price_nad = args.trigger_closeout_price_nad;
        reset_staged_settlement(order);
        Ok(())
    }
}

impl<'info> CancelLeverageOrder<'info> {
    pub fn handle_cancel(_ctx: Context<Self>) -> Result<()> {
        Ok(())
    }
}

impl<'info> BeforeLeverageOrder<'info> {
    pub fn handle_before(ctx: Context<Self>, _args: ExecuteOrderArgs, expected_kind: u8) -> Result<()> {
        let order = &mut ctx.accounts.order;
        require!(order.kind == expected_kind, LeverageDelegateError::InvalidOrder);
        let closeout_price_nad = closeout_price_per_unit_nad(
            &ctx.accounts.pair,
            &ctx.accounts.user_leverage_position,
        )?;
        require_trigger_met(expected_kind, closeout_price_nad, order.trigger_closeout_price_nad)?;
        let debt_token = match ctx.accounts.user_leverage_position.is_debt_token0 {
            true => ctx.accounts.pair.token0,
            false => ctx.accounts.pair.token1,
        };
        require_keys_eq!(
            ctx.accounts.token_mint.key(),
            debt_token,
            LeverageDelegateError::InvalidTokenAccount
        );
        require!(
            ctx.accounts.custody_token_account.amount == 0,
            LeverageDelegateError::InvalidTokenAccount
        );
        let closeout_value = closeout_value(
            &ctx.accounts.pair,
            &ctx.accounts.user_leverage_position,
        )?;
        let debt_amount = ctx
            .accounts
            .user_leverage_position
            .calculate_debt(&ctx.accounts.pair)?;
        let residual = closeout_value
            .checked_sub(debt_amount)
            .ok_or(LeverageDelegateError::InvalidOrder)?;
        stage_close_settlement(
            order,
            ctx.accounts.user_leverage_position.margin_amount,
            ctx.accounts.custody_token_account.key(),
            ctx.accounts.token_mint.key(),
            residual,
        );
        let approval = LeverageDelegationApproval::new(
            LEVERAGE_DELEGATE_CLOSE,
            ctx.accounts.pair.key(),
            order.owner,
            ctx.accounts.user_leverage_position.key(),
            ctx.accounts.user_leverage_delegation.key(),
            ctx.accounts.user_leverage_position.is_debt_token0,
            ctx.accounts.custody_token_account.key(),
            ctx.accounts.token_mint.key(),
            residual,
        );
        let mut data = Vec::new();
        approval
            .serialize(&mut data)
            .map_err(|_| LeverageDelegateError::ApprovalSerializationFailed)?;
        set_return_data(&data);
        Ok(())
    }
}

impl<'info> AfterCloseOrder<'info> {
    pub fn handle_after(ctx: Context<Self>, _args: ExecuteOrderArgs) -> Result<()> {
        require_closed_leverage_position(&ctx.accounts.user_leverage_position)?;
        require_staged_settlement(
            &ctx.accounts.order,
            ctx.accounts.custody_token_account.key(),
            ctx.accounts.token_mint.key(),
            ctx.accounts.custody_token_account.amount,
        )?;
        let amount = ctx.accounts.custody_token_account.amount;
        if amount == 0 {
            return Ok(());
        }

        let incentive = executor_incentive(amount, ctx.accounts.order.staged_margin)?;
        let owner_amount = amount
            .checked_sub(incentive)
            .ok_or(LeverageDelegateError::MathOverflow)?;
        let order_key = ctx.accounts.order.key();
        let bump = ctx.bumps.custody_authority;
        let signer_seeds = &[
            CUSTODY_AUTHORITY_SEED_PREFIX,
            order_key.as_ref(),
            &[bump],
        ];
        let signer = &[&signer_seeds[..]];

        if incentive > 0 {
            token::transfer_checked(
                CpiContext::new_with_signer(
                    ctx.accounts.token_program.to_account_info(),
                    TransferChecked {
                        from: ctx.accounts.custody_token_account.to_account_info(),
                        mint: ctx.accounts.token_mint.to_account_info(),
                        to: ctx.accounts.executor_token_account.to_account_info(),
                        authority: ctx.accounts.custody_authority.to_account_info(),
                    },
                    signer,
                ),
                incentive,
                ctx.accounts.token_mint.decimals,
            )?;
        }
        if owner_amount > 0 {
            token::transfer_checked(
                CpiContext::new_with_signer(
                    ctx.accounts.token_program.to_account_info(),
                    TransferChecked {
                        from: ctx.accounts.custody_token_account.to_account_info(),
                        mint: ctx.accounts.token_mint.to_account_info(),
                        to: ctx.accounts.owner_token_account.to_account_info(),
                        authority: ctx.accounts.custody_authority.to_account_info(),
                    },
                    signer,
                ),
                owner_amount,
                ctx.accounts.token_mint.decimals,
            )?;
        }
        Ok(())
    }
}

fn reset_staged_settlement(order: &mut LeverageOrder) {
    order.staged_margin = 0;
    order.staged_custody_token_account = Pubkey::default();
    order.staged_output_mint = Pubkey::default();
    order.staged_output_amount = 0;
}

fn stage_close_settlement(
    order: &mut LeverageOrder,
    margin: u64,
    custody_token_account: Pubkey,
    output_mint: Pubkey,
    output_amount: u64,
) {
    order.staged_margin = margin;
    order.staged_custody_token_account = custody_token_account;
    order.staged_output_mint = output_mint;
    order.staged_output_amount = output_amount;
}

fn require_staged_settlement(
    order: &LeverageOrder,
    custody_token_account: Pubkey,
    output_mint: Pubkey,
    output_amount: u64,
) -> Result<()> {
    require_keys_eq!(
        order.staged_custody_token_account,
        custody_token_account,
        LeverageDelegateError::InvalidTokenAccount
    );
    require_keys_eq!(
        order.staged_output_mint,
        output_mint,
        LeverageDelegateError::InvalidTokenAccount
    );
    require!(
        order.staged_output_amount == output_amount,
        LeverageDelegateError::InvalidTokenAccount
    );
    Ok(())
}

fn validate_order_kind(kind: u8) -> Result<()> {
    require!(
        kind == ORDER_KIND_TAKE_PROFIT || kind == ORDER_KIND_STOP_LOSS,
        LeverageDelegateError::InvalidOrder
    );
    Ok(())
}

fn require_trigger_met(kind: u8, closeout_price_nad: u64, trigger_closeout_price_nad: u64) -> Result<()> {
    match kind {
        ORDER_KIND_TAKE_PROFIT => require!(
            closeout_price_nad >= trigger_closeout_price_nad,
            LeverageDelegateError::TriggerNotMet
        ),
        ORDER_KIND_STOP_LOSS => require!(
            closeout_price_nad <= trigger_closeout_price_nad,
            LeverageDelegateError::TriggerNotMet
        ),
        _ => return err!(LeverageDelegateError::InvalidOrder),
    }
    Ok(())
}

fn executor_incentive(amount: u64, staged_margin: u64) -> Result<u64> {
    Ok(min(
        amount,
        ceil_div(
            (staged_margin as u128)
                .checked_mul(EXECUTOR_INCENTIVE_BPS as u128)
                .ok_or(LeverageDelegateError::MathOverflow)?,
            BPS_DENOMINATOR as u128,
        )
        .ok_or(LeverageDelegateError::MathOverflow)? as u64,
    ))
}

fn require_closed_leverage_position(position: &UserLeveragePosition) -> Result<()> {
    require!(
        position.debt_shares == 0 && position.collateral_amount == 0,
        LeverageDelegateError::InvalidOrder
    );
    Ok(())
}

fn closeout_value(pair: &Pair, position: &UserLeveragePosition) -> Result<u64> {
    require!(position.collateral_amount > 0, LeverageDelegateError::InvalidOrder);
    let is_collateral_token0 = !position.is_debt_token0;
    let reserve_in = if is_collateral_token0 { pair.reserve0 } else { pair.reserve1 };
    let reserve_out = if is_collateral_token0 { pair.reserve1 } else { pair.reserve0 };
    let swap_fee = ceil_div(
        (position.collateral_amount as u128)
            .checked_mul(pair.swap_fee_bps as u128)
            .ok_or(LeverageDelegateError::MathOverflow)?,
        BPS_DENOMINATOR as u128,
    )
    .ok_or(LeverageDelegateError::MathOverflow)? as u64;
    let amount_in_after_fee = position
        .collateral_amount
        .checked_sub(swap_fee)
        .ok_or(LeverageDelegateError::MathOverflow)?;
    CPCurve::calculate_amount_out(reserve_in, reserve_out, amount_in_after_fee)
}

fn closeout_price_per_unit_nad(pair: &Pair, position: &UserLeveragePosition) -> Result<u64> {
    let closeout_value = closeout_value(pair, position)?;
    Ok((closeout_value as u128)
        .checked_mul(NAD as u128)
        .ok_or(LeverageDelegateError::MathOverflow)?
        .checked_div(position.collateral_amount as u128)
        .ok_or(LeverageDelegateError::MathOverflow)?
        .try_into()
        .map_err(|_| LeverageDelegateError::MathOverflow)?)
}

#[error_code]
pub enum LeverageDelegateError {
    #[msg("Invalid leverage order")]
    InvalidOrder,
    #[msg("Order trigger is not met")]
    TriggerNotMet,
    #[msg("Invalid token account")]
    InvalidTokenAccount,
    #[msg("Math overflow")]
    MathOverflow,
    #[msg("Approval serialization failed")]
    ApprovalSerializationFailed,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pair_with_reserves(reserve0: u64, reserve1: u64, swap_fee_bps: u16) -> Pair {
        Pair {
            token0: Pubkey::new_unique(),
            token1: Pubkey::new_unique(),
            lp_mint: Pubkey::new_unique(),
            rate_model: Pubkey::new_unique(),
            swap_fee_bps,
            half_life: 0,
            fixed_cf_bps: None,
            reserve0,
            reserve1,
            cash_reserve0: reserve0,
            cash_reserve1: reserve1,
            last_price0_ema: Default::default(),
            last_price1_ema: Default::default(),
            last_update: 0,
            last_rate0: 0,
            last_rate1: 0,
            total_debt0: 0,
            total_debt1: 0,
            total_debt0_shares: 0,
            total_debt1_shares: 0,
            total_supply: 1_000,
            total_collateral0: 0,
            total_collateral1: 0,
            token0_decimals: 6,
            token1_decimals: 6,
            params_hash: [0; 32],
            version: 1,
            bump: 255,
            vault_bumps: Default::default(),
            reduce_only: false,
        }
    }

    fn leverage_position(is_debt_token0: bool, collateral_amount: u64) -> UserLeveragePosition {
        UserLeveragePosition {
            owner: Pubkey::new_unique(),
            pair: Pubkey::new_unique(),
            is_debt_token0,
            collateral_amount,
            margin_amount: 0,
            open_notional: 0,
            debt_amount: 0,
            debt_shares: 0,
            multiplier_bps: 0,
            opened_at: 0,
            opened_slot: 0,
            bump: 0,
        }
    }

    fn leverage_order() -> LeverageOrder {
        LeverageOrder {
            owner: Pubkey::new_unique(),
            pair: Pubkey::new_unique(),
            position: Pubkey::new_unique(),
            order_id: 1,
            kind: ORDER_KIND_TAKE_PROFIT,
            trigger_closeout_price_nad: NAD,
            staged_margin: 0,
            staged_custody_token_account: Pubkey::default(),
            staged_output_mint: Pubkey::default(),
            staged_output_amount: 0,
            bump: 255,
        }
    }

    #[test]
    fn order_kind_validation_accepts_only_tp_or_sl() {
        assert!(validate_order_kind(ORDER_KIND_TAKE_PROFIT).is_ok());
        assert!(validate_order_kind(ORDER_KIND_STOP_LOSS).is_ok());
        assert!(validate_order_kind(0).is_err());
    }

    #[test]
    fn executor_incentive_is_five_percent_of_margin_capped_by_residual() {
        assert_eq!(executor_incentive(1_000, 10_000).unwrap(), 500);
        assert_eq!(executor_incentive(300, 10_000).unwrap(), 300);
    }

    #[test]
    fn executor_incentive_rounds_up() {
        assert_eq!(executor_incentive(10, 1).unwrap(), 1);
    }

    #[test]
    fn after_close_requires_zeroed_position() {
        let mut position = leverage_position(false, 0);
        position.debt_shares = 0;
        assert!(require_closed_leverage_position(&position).is_ok());

        position.debt_shares = 1;
        assert!(require_closed_leverage_position(&position).is_err());

        position.debt_shares = 0;
        position.collateral_amount = 1;
        assert!(require_closed_leverage_position(&position).is_err());
    }

    #[test]
    fn staged_settlement_defaults_reject_direct_after_close_cleanup() {
        let order = leverage_order();
        let custody = Pubkey::new_unique();
        let mint = Pubkey::new_unique();

        assert!(require_staged_settlement(&order, custody, mint, 0).is_err());
    }

    #[test]
    fn stage_close_settlement_binds_custody_mint_and_amount() {
        let mut order = leverage_order();
        let custody = Pubkey::new_unique();
        let mint = Pubkey::new_unique();
        stage_close_settlement(&mut order, 10_000, custody, mint, 123);

        assert_eq!(order.staged_margin, 10_000);
        assert_eq!(order.staged_custody_token_account, custody);
        assert_eq!(order.staged_output_mint, mint);
        assert_eq!(order.staged_output_amount, 123);
        assert!(require_staged_settlement(&order, custody, mint, 123).is_ok());
    }

    #[test]
    fn staged_settlement_rejects_wrong_custody_mint_or_amount() {
        let mut order = leverage_order();
        let custody = Pubkey::new_unique();
        let mint = Pubkey::new_unique();
        stage_close_settlement(&mut order, 10_000, custody, mint, 123);

        assert!(require_staged_settlement(&order, Pubkey::new_unique(), mint, 123).is_err());
        assert!(require_staged_settlement(&order, custody, Pubkey::new_unique(), 123).is_err());
        assert!(require_staged_settlement(&order, custody, mint, 122).is_err());
    }

    #[test]
    fn staged_zero_residual_can_only_settle_against_staged_account() {
        let mut order = leverage_order();
        let custody = Pubkey::new_unique();
        let mint = Pubkey::new_unique();
        stage_close_settlement(&mut order, 10_000, custody, mint, 0);

        assert!(require_staged_settlement(&order, custody, mint, 0).is_ok());
        assert!(require_staged_settlement(&order, Pubkey::new_unique(), mint, 0).is_err());
    }

    #[test]
    fn reset_staged_settlement_clears_prior_approval() {
        let mut order = leverage_order();
        stage_close_settlement(
            &mut order,
            10_000,
            Pubkey::new_unique(),
            Pubkey::new_unique(),
            123,
        );
        reset_staged_settlement(&mut order);

        assert_eq!(order.staged_margin, 0);
        assert_eq!(order.staged_custody_token_account, Pubkey::default());
        assert_eq!(order.staged_output_mint, Pubkey::default());
        assert_eq!(order.staged_output_amount, 0);
    }

    #[test]
    fn trigger_rules_match_take_profit_and_stop_loss_direction() {
        assert!(require_trigger_met(ORDER_KIND_TAKE_PROFIT, 101, 100).is_ok());
        assert!(require_trigger_met(ORDER_KIND_TAKE_PROFIT, 99, 100).is_err());
        assert!(require_trigger_met(ORDER_KIND_STOP_LOSS, 99, 100).is_ok());
        assert!(require_trigger_met(ORDER_KIND_STOP_LOSS, 101, 100).is_err());
        assert!(require_trigger_met(0, 100, 100).is_err());
    }

    #[test]
    fn approval_payload_binds_close_action_and_delegation() {
        let pair = Pubkey::new_unique();
        let owner = Pubkey::new_unique();
        let position = Pubkey::new_unique();
        let delegation = Pubkey::new_unique();
        let recipient = Pubkey::new_unique();
        let mint = Pubkey::new_unique();
        let approval = LeverageDelegationApproval::new(
            LEVERAGE_DELEGATE_CLOSE,
            pair,
            owner,
            position,
            delegation,
            true,
            recipient,
            mint,
            123,
        );
        let mut data = Vec::new();
        approval.serialize(&mut data).unwrap();
        let decoded = LeverageDelegationApproval::deserialize(&mut data.as_slice()).unwrap();

        assert_eq!(decoded.action, LEVERAGE_DELEGATE_CLOSE);
        assert_eq!(decoded.pair, pair);
        assert_eq!(decoded.owner, owner);
        assert_eq!(decoded.position, position);
        assert_eq!(decoded.delegation, delegation);
        assert!(decoded.is_debt_token0);
        assert_eq!(decoded.recipient_token_account, recipient);
        assert_eq!(decoded.output_mint, mint);
        assert_eq!(decoded.output_amount, 123);
    }

    #[test]
    fn closeout_price_per_unit_uses_swap_fee_before_curve_output() {
        let pair = pair_with_reserves(1_000_000, 1_000_000, 100);
        let position = leverage_position(false, 1_000);
        let expected_value = CPCurve::calculate_amount_out(1_000_000, 1_000_000, 990).unwrap();
        let expected_price = (expected_value as u128 * NAD as u128 / 1_000) as u64;

        assert_eq!(
            closeout_price_per_unit_nad(&pair, &position).unwrap(),
            expected_price
        );
    }

    #[test]
    fn closeout_price_per_unit_uses_inverse_reserve_direction_for_token1_debt() {
        let pair = pair_with_reserves(2_000_000, 1_000_000, 0);
        let position = leverage_position(true, 1_000);
        let expected_value = CPCurve::calculate_amount_out(1_000_000, 2_000_000, 1_000).unwrap();
        let expected_price = (expected_value as u128 * NAD as u128 / 1_000) as u64;

        assert_eq!(
            closeout_price_per_unit_nad(&pair, &position).unwrap(),
            expected_price
        );
    }

    #[test]
    fn closeout_price_per_unit_rejects_zero_collateral() {
        let pair = pair_with_reserves(1_000_000, 1_000_000, 0);
        let position = leverage_position(false, 0);

        assert!(closeout_price_per_unit_nad(&pair, &position).is_err());
    }
}
