use anchor_lang::prelude::*;
use anchor_spl::{
    token::Token,
    token_interface::{Mint, Token2022, TokenAccount},
};

use crate::{
    constants::*,
    errors::ErrorCode,
    events::{
        MarketCollateralDepositedV2, MarketDebtUpdatedV2, MarketEventMetadataV2,
        MarketHealthUpdatedV2,
    },
    generate_market_v2_seeds,
    state::{DebtBookV2, MarginPositionV2, MarketSideV2, MarketV2},
    utils::{
        account::get_size_with_discriminator,
        market_v2_math::require_market_reserve_floor,
        token::{transfer_from_user_to_vault, transfer_from_vault_to_user},
    },
};

use super::common::{require_supported_asset_mint, token_program_for_mint};

#[derive(AnchorSerialize, AnchorDeserialize, Clone)]
pub struct DepositCollateralV2Args {
    pub market_side_index: u8,
    pub deposit_amount: u64,
}

#[derive(AnchorSerialize, AnchorDeserialize, Clone)]
pub struct BorrowV2Args {
    pub borrow_asset_is_asset0: bool,
    pub borrow_amount: u64,
    pub collateral_amount_to_recognize: u64,
    pub min_health_bps: u64,
}

#[derive(AnchorSerialize, AnchorDeserialize, Clone)]
pub struct RepayV2Args {
    pub repay_asset_is_asset0: bool,
    pub repay_amount: u64,
}

#[event_cpi]
#[derive(Accounts)]
#[instruction(args: DepositCollateralV2Args)]
pub struct DepositCollateralV2<'info> {
    #[account(
        mut,
        seeds = [
            MARKET_V2_SEED_PREFIX,
            market.asset0_mint.as_ref(),
            market.asset1_mint.as_ref(),
            market.params_hash.as_ref(),
        ],
        bump = market.bump
    )]
    pub market: Box<Account<'info, MarketV2>>,

    #[account(mut)]
    pub owner: Signer<'info>,

    pub asset_mint: Box<InterfaceAccount<'info, Mint>>,

    #[account(mut)]
    pub collateral_vault: Box<InterfaceAccount<'info, TokenAccount>>,

    #[account(mut)]
    pub owner_asset_account: Box<InterfaceAccount<'info, TokenAccount>>,

    #[account(
        init_if_needed,
        payer = owner,
        space = get_size_with_discriminator::<MarginPositionV2>(),
        seeds = [
            MARGIN_POSITION_V2_SEED_PREFIX,
            market.key().as_ref(),
            owner.key().as_ref(),
        ],
        bump
    )]
    pub margin_position: Box<Account<'info, MarginPositionV2>>,

    pub token_program: Program<'info, Token>,
    pub token_2022_program: Program<'info, Token2022>,
    pub system_program: Program<'info, System>,
}

impl<'info> DepositCollateralV2<'info> {
    pub fn validate(&self, args: &DepositCollateralV2Args) -> Result<()> {
        self.market.assert_started()?;
        require!(args.deposit_amount > 0, ErrorCode::AmountZero);
        require_gte!(
            self.owner_asset_account.amount,
            args.deposit_amount,
            ErrorCode::InsufficientBalance
        );
        validate_collateral_accounts(
            &self.market,
            args.market_side_index,
            self.owner.key(),
            &self.asset_mint,
            &self.collateral_vault,
            &self.owner_asset_account,
        )?;
        require_supported_asset_mint(&self.asset_mint)?;
        if self.margin_position.is_initialized() {
            self.margin_position
                .assert_position(self.owner.key(), self.market.key())?;
        }
        Ok(())
    }

    pub fn handle_deposit(ctx: Context<Self>, args: DepositCollateralV2Args) -> Result<()> {
        let market_key = ctx.accounts.market.key();
        let owner_key = ctx.accounts.owner.key();
        let asset_mint_key = ctx.accounts.asset_mint.key();

        if !ctx.accounts.margin_position.is_initialized() {
            ctx.accounts.margin_position.initialize(
                owner_key,
                market_key,
                ctx.bumps.margin_position,
            );
        }
        ctx.accounts
            .margin_position
            .assert_position(owner_key, market_key)?;

        let collateral_balance_before = ctx.accounts.collateral_vault.amount;
        let asset_token_program = token_program_for_mint(
            &ctx.accounts.asset_mint,
            &ctx.accounts.token_program,
            &ctx.accounts.token_2022_program,
        )?;
        transfer_from_user_to_vault(
            ctx.accounts.owner.to_account_info(),
            ctx.accounts.owner_asset_account.to_account_info(),
            ctx.accounts.collateral_vault.to_account_info(),
            ctx.accounts.asset_mint.to_account_info(),
            asset_token_program,
            args.deposit_amount,
            ctx.accounts.asset_mint.decimals,
        )?;
        ctx.accounts.collateral_vault.reload()?;
        let collateral_credit = ctx
            .accounts
            .collateral_vault
            .amount
            .checked_sub(collateral_balance_before)
            .ok_or(ErrorCode::MarketMathOverflowV2)?;
        require!(collateral_credit > 0, ErrorCode::AmountZero);

        if args.market_side_index == 0 {
            ctx.accounts.margin_position.collateral0 = ctx
                .accounts
                .margin_position
                .collateral0
                .checked_add(collateral_credit)
                .ok_or(ErrorCode::MarketMathOverflowV2)?;
        } else {
            ctx.accounts.margin_position.collateral1 = ctx
                .accounts
                .margin_position
                .collateral1
                .checked_add(collateral_credit)
                .ok_or(ErrorCode::MarketMathOverflowV2)?;
        }

        emit_cpi!(MarketCollateralDepositedV2 {
            market: market_key,
            owner: owner_key,
            asset_mint: asset_mint_key,
            collateral_credit,
            collateral0: ctx.accounts.margin_position.collateral0,
            collateral1: ctx.accounts.margin_position.collateral1,
            metadata: MarketEventMetadataV2::new(owner_key, market_key),
        });

        Ok(())
    }
}

#[event_cpi]
#[derive(Accounts)]
#[instruction(args: BorrowV2Args)]
pub struct BorrowV2<'info> {
    #[account(
        mut,
        seeds = [
            MARKET_V2_SEED_PREFIX,
            market.asset0_mint.as_ref(),
            market.asset1_mint.as_ref(),
            market.params_hash.as_ref(),
        ],
        bump = market.bump
    )]
    pub market: Box<Account<'info, MarketV2>>,

    #[account(mut)]
    pub owner: Signer<'info>,

    pub debt_asset_mint: Box<InterfaceAccount<'info, Mint>>,

    pub collateral_asset_mint: Box<InterfaceAccount<'info, Mint>>,

    #[account(mut)]
    pub reserve_vault: Box<InterfaceAccount<'info, TokenAccount>>,

    #[account(mut)]
    pub owner_debt_account: Box<InterfaceAccount<'info, TokenAccount>>,

    #[account(
        mut,
        seeds = [
            MARGIN_POSITION_V2_SEED_PREFIX,
            market.key().as_ref(),
            owner.key().as_ref(),
        ],
        bump = margin_position.bump
    )]
    pub margin_position: Box<Account<'info, MarginPositionV2>>,

    pub token_program: Program<'info, Token>,
    pub token_2022_program: Program<'info, Token2022>,
}

impl<'info> BorrowV2<'info> {
    pub fn validate(&self, args: &BorrowV2Args) -> Result<()> {
        self.market.assert_live()?;
        require!(
            !self.market.config.soft_borrow_enabled,
            ErrorCode::InvalidMarketConfigV2
        );
        require!(args.borrow_amount > 0, ErrorCode::AmountZero);
        require!(
            args.collateral_amount_to_recognize > 0,
            ErrorCode::InsufficientRecognizedCollateralV2
        );
        validate_borrow_accounts(
            &self.market,
            args.borrow_asset_is_asset0,
            self.owner.key(),
            &self.debt_asset_mint,
            &self.collateral_asset_mint,
            &self.reserve_vault,
            &self.owner_debt_account,
        )?;
        require_supported_asset_mint(&self.debt_asset_mint)?;
        self.margin_position
            .assert_position(self.owner.key(), self.market.key())?;
        Ok(())
    }

    pub fn handle_borrow(ctx: Context<Self>, args: BorrowV2Args) -> Result<()> {
        let market_key = ctx.accounts.market.key();
        let owner_key = ctx.accounts.owner.key();
        let debt_asset_mint_key = ctx.accounts.debt_asset_mint.key();
        let debt_delta = i64::try_from(args.borrow_amount).map_err(|_| ErrorCode::Overflow)?;

        apply_borrow_state(
            &mut ctx.accounts.market,
            &mut ctx.accounts.margin_position,
            args.borrow_asset_is_asset0,
            args.borrow_amount,
            args.collateral_amount_to_recognize,
            args.min_health_bps,
        )?;

        let debt_token_program = token_program_for_mint(
            &ctx.accounts.debt_asset_mint,
            &ctx.accounts.token_program,
            &ctx.accounts.token_2022_program,
        )?;
        transfer_from_vault_to_user(
            ctx.accounts.market.to_account_info(),
            ctx.accounts.reserve_vault.to_account_info(),
            ctx.accounts.owner_debt_account.to_account_info(),
            ctx.accounts.debt_asset_mint.to_account_info(),
            debt_token_program,
            args.borrow_amount,
            ctx.accounts.debt_asset_mint.decimals,
            &[&generate_market_v2_seeds!(ctx.accounts.market)[..]],
        )?;

        emit_cpi!(MarketDebtUpdatedV2 {
            market: market_key,
            owner: owner_key,
            debt_asset_mint: debt_asset_mint_key,
            debt_delta,
            fixed_debt0: ctx.accounts.market.debt_book.fixed_debt0()?,
            fixed_debt1: ctx.accounts.market.debt_book.fixed_debt1()?,
            health0_bps: ctx.accounts.market.health.health0_bps,
            health1_bps: ctx.accounts.market.health.health1_bps,
            metadata: MarketEventMetadataV2::new(owner_key, market_key),
        });
        emit_cpi!(MarketHealthUpdatedV2 {
            market: market_key,
            recognized_collateral0_for_debt1: ctx
                .accounts
                .market
                .health
                .recognized_collateral0_for_debt1,
            recognized_collateral1_for_debt0: ctx
                .accounts
                .market
                .health
                .recognized_collateral1_for_debt0,
            effective_debt0_nad: ctx.accounts.market.health.effective_debt0_nad,
            effective_debt1_nad: ctx.accounts.market.health.effective_debt1_nad,
            health0_bps: ctx.accounts.market.health.health0_bps,
            health1_bps: ctx.accounts.market.health.health1_bps,
            metadata: MarketEventMetadataV2::new(owner_key, market_key),
        });
        Ok(())
    }
}

#[event_cpi]
#[derive(Accounts)]
#[instruction(args: RepayV2Args)]
pub struct RepayV2<'info> {
    #[account(
        mut,
        seeds = [
            MARKET_V2_SEED_PREFIX,
            market.asset0_mint.as_ref(),
            market.asset1_mint.as_ref(),
            market.params_hash.as_ref(),
        ],
        bump = market.bump
    )]
    pub market: Box<Account<'info, MarketV2>>,

    #[account(mut)]
    pub owner: Signer<'info>,

    pub debt_asset_mint: Box<InterfaceAccount<'info, Mint>>,

    #[account(mut)]
    pub reserve_vault: Box<InterfaceAccount<'info, TokenAccount>>,

    #[account(mut)]
    pub owner_debt_account: Box<InterfaceAccount<'info, TokenAccount>>,

    #[account(
        mut,
        seeds = [
            MARGIN_POSITION_V2_SEED_PREFIX,
            market.key().as_ref(),
            owner.key().as_ref(),
        ],
        bump = margin_position.bump
    )]
    pub margin_position: Box<Account<'info, MarginPositionV2>>,

    pub token_program: Program<'info, Token>,
    pub token_2022_program: Program<'info, Token2022>,
}

impl<'info> RepayV2<'info> {
    pub fn validate(&self, args: &RepayV2Args) -> Result<()> {
        self.market.assert_started()?;
        require!(args.repay_amount > 0, ErrorCode::AmountZero);
        require_gte!(
            self.owner_debt_account.amount,
            args.repay_amount,
            ErrorCode::InsufficientBalance
        );
        validate_repay_accounts(
            &self.market,
            args.repay_asset_is_asset0,
            self.owner.key(),
            &self.debt_asset_mint,
            &self.reserve_vault,
            &self.owner_debt_account,
        )?;
        require_supported_asset_mint(&self.debt_asset_mint)?;
        self.margin_position
            .assert_position(self.owner.key(), self.market.key())?;
        Ok(())
    }

    pub fn handle_repay(ctx: Context<Self>, args: RepayV2Args) -> Result<()> {
        let market_key = ctx.accounts.market.key();
        let owner_key = ctx.accounts.owner.key();
        let debt_asset_mint_key = ctx.accounts.debt_asset_mint.key();
        let reserve_balance_before = ctx.accounts.reserve_vault.amount;
        let debt_token_program = token_program_for_mint(
            &ctx.accounts.debt_asset_mint,
            &ctx.accounts.token_program,
            &ctx.accounts.token_2022_program,
        )?;
        transfer_from_user_to_vault(
            ctx.accounts.owner.to_account_info(),
            ctx.accounts.owner_debt_account.to_account_info(),
            ctx.accounts.reserve_vault.to_account_info(),
            ctx.accounts.debt_asset_mint.to_account_info(),
            debt_token_program,
            args.repay_amount,
            ctx.accounts.debt_asset_mint.decimals,
        )?;
        ctx.accounts.reserve_vault.reload()?;
        let repay_credit = ctx
            .accounts
            .reserve_vault
            .amount
            .checked_sub(reserve_balance_before)
            .ok_or(ErrorCode::MarketMathOverflowV2)?;
        require!(repay_credit > 0, ErrorCode::AmountZero);
        let debt_delta = -i64::try_from(repay_credit).map_err(|_| ErrorCode::Overflow)?;

        apply_repay_state(
            &mut ctx.accounts.market,
            &mut ctx.accounts.margin_position,
            args.repay_asset_is_asset0,
            repay_credit,
        )?;

        emit_cpi!(MarketDebtUpdatedV2 {
            market: market_key,
            owner: owner_key,
            debt_asset_mint: debt_asset_mint_key,
            debt_delta,
            fixed_debt0: ctx.accounts.market.debt_book.fixed_debt0()?,
            fixed_debt1: ctx.accounts.market.debt_book.fixed_debt1()?,
            health0_bps: ctx.accounts.market.health.health0_bps,
            health1_bps: ctx.accounts.market.health.health1_bps,
            metadata: MarketEventMetadataV2::new(owner_key, market_key),
        });
        emit_cpi!(MarketHealthUpdatedV2 {
            market: market_key,
            recognized_collateral0_for_debt1: ctx
                .accounts
                .market
                .health
                .recognized_collateral0_for_debt1,
            recognized_collateral1_for_debt0: ctx
                .accounts
                .market
                .health
                .recognized_collateral1_for_debt0,
            effective_debt0_nad: ctx.accounts.market.health.effective_debt0_nad,
            effective_debt1_nad: ctx.accounts.market.health.effective_debt1_nad,
            health0_bps: ctx.accounts.market.health.health0_bps,
            health1_bps: ctx.accounts.market.health.health1_bps,
            metadata: MarketEventMetadataV2::new(owner_key, market_key),
        });
        Ok(())
    }
}

fn validate_collateral_accounts<'info>(
    market: &Account<'info, MarketV2>,
    market_side_index: u8,
    owner: Pubkey,
    asset_mint: &InterfaceAccount<'info, Mint>,
    collateral_vault: &InterfaceAccount<'info, TokenAccount>,
    owner_asset_account: &InterfaceAccount<'info, TokenAccount>,
) -> Result<()> {
    let market_side = market.side(market_side_index)?;
    require_keys_eq!(
        market_side.asset_mint,
        asset_mint.key(),
        ErrorCode::InvalidMint
    );
    require_keys_eq!(
        market_side.collateral_vault,
        collateral_vault.key(),
        ErrorCode::InvalidVault
    );
    require_keys_eq!(
        collateral_vault.mint,
        asset_mint.key(),
        ErrorCode::InvalidVault
    );
    require_keys_eq!(
        collateral_vault.owner,
        market.key(),
        ErrorCode::InvalidVault
    );
    require_keys_eq!(
        owner_asset_account.mint,
        asset_mint.key(),
        ErrorCode::InvalidTokenAccount
    );
    require_keys_eq!(
        owner_asset_account.owner,
        owner,
        ErrorCode::InvalidTokenAccount
    );
    Ok(())
}

fn validate_borrow_accounts<'info>(
    market: &Account<'info, MarketV2>,
    borrow_asset_is_asset0: bool,
    owner: Pubkey,
    debt_asset_mint: &InterfaceAccount<'info, Mint>,
    collateral_asset_mint: &InterfaceAccount<'info, Mint>,
    reserve_vault: &InterfaceAccount<'info, TokenAccount>,
    owner_debt_account: &InterfaceAccount<'info, TokenAccount>,
) -> Result<()> {
    let (debt_side, collateral_side) = if borrow_asset_is_asset0 {
        (&market.side0, &market.side1)
    } else {
        (&market.side1, &market.side0)
    };
    validate_debt_reserve_accounts(
        market,
        debt_side,
        owner,
        debt_asset_mint,
        reserve_vault,
        owner_debt_account,
    )?;
    require_keys_eq!(
        collateral_side.asset_mint,
        collateral_asset_mint.key(),
        ErrorCode::InvalidMint
    );
    Ok(())
}

fn validate_repay_accounts<'info>(
    market: &Account<'info, MarketV2>,
    repay_asset_is_asset0: bool,
    owner: Pubkey,
    debt_asset_mint: &InterfaceAccount<'info, Mint>,
    reserve_vault: &InterfaceAccount<'info, TokenAccount>,
    owner_debt_account: &InterfaceAccount<'info, TokenAccount>,
) -> Result<()> {
    let debt_side = if repay_asset_is_asset0 {
        &market.side0
    } else {
        &market.side1
    };
    validate_debt_reserve_accounts(
        market,
        debt_side,
        owner,
        debt_asset_mint,
        reserve_vault,
        owner_debt_account,
    )
}

fn validate_debt_reserve_accounts<'info>(
    market: &Account<'info, MarketV2>,
    debt_side: &MarketSideV2,
    owner: Pubkey,
    debt_asset_mint: &InterfaceAccount<'info, Mint>,
    reserve_vault: &InterfaceAccount<'info, TokenAccount>,
    owner_debt_account: &InterfaceAccount<'info, TokenAccount>,
) -> Result<()> {
    require_keys_eq!(
        debt_side.asset_mint,
        debt_asset_mint.key(),
        ErrorCode::InvalidMint
    );
    require_keys_eq!(
        debt_side.reserve_vault,
        reserve_vault.key(),
        ErrorCode::InvalidVault
    );
    require_keys_eq!(
        reserve_vault.mint,
        debt_asset_mint.key(),
        ErrorCode::InvalidVault
    );
    require_keys_eq!(reserve_vault.owner, market.key(), ErrorCode::InvalidVault);
    require_keys_eq!(
        owner_debt_account.mint,
        debt_asset_mint.key(),
        ErrorCode::InvalidTokenAccount
    );
    require_keys_eq!(
        owner_debt_account.owner,
        owner,
        ErrorCode::InvalidTokenAccount
    );
    Ok(())
}

fn apply_borrow_state(
    market: &mut MarketV2,
    margin_position: &mut MarginPositionV2,
    borrow_asset_is_asset0: bool,
    borrow_amount: u64,
    collateral_amount_to_recognize: u64,
    min_health_bps: u64,
) -> Result<()> {
    let debt_shares = if borrow_asset_is_asset0 {
        DebtBookV2::debt_to_shares(borrow_amount, market.debt_book.borrow_index0_nad)?
    } else {
        DebtBookV2::debt_to_shares(borrow_amount, market.debt_book.borrow_index1_nad)?
    };
    let debt_side = if borrow_asset_is_asset0 {
        &mut market.side0
    } else {
        &mut market.side1
    };
    require_borrow_headroom(debt_side, borrow_amount)?;
    debt_side.reserve_ledger.live_reserve = debt_side
        .reserve_ledger
        .live_reserve
        .checked_sub(borrow_amount)
        .ok_or(ErrorCode::ReserveUnderflow)?;
    debt_side.reserve_ledger.cash_reserve = debt_side
        .reserve_ledger
        .cash_reserve
        .checked_sub(borrow_amount)
        .ok_or(ErrorCode::CashReserveUnderflow)?;

    if borrow_asset_is_asset0 {
        require_gte!(
            margin_position.idle_collateral1()?,
            collateral_amount_to_recognize,
            ErrorCode::InsufficientRecognizedCollateralV2
        );
        margin_position.recognized_collateral1_for_debt0 = margin_position
            .recognized_collateral1_for_debt0
            .checked_add(collateral_amount_to_recognize)
            .ok_or(ErrorCode::MarketMathOverflowV2)?;
        margin_position.fixed_debt0_shares = margin_position
            .fixed_debt0_shares
            .checked_add(debt_shares)
            .ok_or(ErrorCode::MarketMathOverflowV2)?;
        market.debt_book.fixed_debt0_shares = market
            .debt_book
            .fixed_debt0_shares
            .checked_add(debt_shares)
            .ok_or(ErrorCode::MarketMathOverflowV2)?;
        market.recognition_ledger.debt_bearing_collateral1_for_debt0 = market
            .recognition_ledger
            .debt_bearing_collateral1_for_debt0
            .checked_add(collateral_amount_to_recognize)
            .ok_or(ErrorCode::MarketMathOverflowV2)?;
    } else {
        require_gte!(
            margin_position.idle_collateral0()?,
            collateral_amount_to_recognize,
            ErrorCode::InsufficientRecognizedCollateralV2
        );
        margin_position.recognized_collateral0_for_debt1 = margin_position
            .recognized_collateral0_for_debt1
            .checked_add(collateral_amount_to_recognize)
            .ok_or(ErrorCode::MarketMathOverflowV2)?;
        margin_position.fixed_debt1_shares = margin_position
            .fixed_debt1_shares
            .checked_add(debt_shares)
            .ok_or(ErrorCode::MarketMathOverflowV2)?;
        market.debt_book.fixed_debt1_shares = market
            .debt_book
            .fixed_debt1_shares
            .checked_add(debt_shares)
            .ok_or(ErrorCode::MarketMathOverflowV2)?;
        market.recognition_ledger.debt_bearing_collateral0_for_debt1 = market
            .recognition_ledger
            .debt_bearing_collateral0_for_debt1
            .checked_add(collateral_amount_to_recognize)
            .ok_or(ErrorCode::MarketMathOverflowV2)?;
    }
    market.recognition_ledger.last_recognition_slot = Clock::get()?.slot;
    market.refresh_market_health()?;
    market.assert_market_health()?;
    market.assert_recognition_cap(margin_position, borrow_asset_is_asset0)?;
    market.assert_position_health(margin_position, borrow_asset_is_asset0, min_health_bps)?;
    let health = if borrow_asset_is_asset0 {
        market.position_health_bps(margin_position, true)?
    } else {
        market.position_health_bps(margin_position, false)?
    };
    require_gte!(
        health,
        min_health_bps,
        ErrorCode::InsufficientMarketHealthV2
    );
    Ok(())
}

fn apply_repay_state(
    market: &mut MarketV2,
    margin_position: &mut MarginPositionV2,
    repay_asset_is_asset0: bool,
    repay_credit: u64,
) -> Result<()> {
    if repay_asset_is_asset0 {
        let debt_before = margin_position.fixed_debt0(&market.debt_book)?;
        require_gte!(
            debt_before,
            repay_credit as u128,
            ErrorCode::InsufficientDebt
        );
        let shares_before = margin_position.fixed_debt0_shares;
        let shares_to_burn = if repay_credit as u128 == debt_before {
            shares_before
        } else {
            DebtBookV2::debt_to_shares(repay_credit, market.debt_book.borrow_index0_nad)?
                .min(shares_before)
        };
        let release_collateral = proportional_release(
            margin_position.recognized_collateral1_for_debt0,
            shares_to_burn,
            shares_before,
        )?;
        margin_position.fixed_debt0_shares = margin_position
            .fixed_debt0_shares
            .checked_sub(shares_to_burn)
            .ok_or(ErrorCode::MarketMathOverflowV2)?;
        margin_position.recognized_collateral1_for_debt0 = margin_position
            .recognized_collateral1_for_debt0
            .checked_sub(release_collateral)
            .ok_or(ErrorCode::MarketMathOverflowV2)?;
        market.debt_book.fixed_debt0_shares = market
            .debt_book
            .fixed_debt0_shares
            .checked_sub(shares_to_burn)
            .ok_or(ErrorCode::MarketMathOverflowV2)?;
        market.recognition_ledger.debt_bearing_collateral1_for_debt0 = market
            .recognition_ledger
            .debt_bearing_collateral1_for_debt0
            .checked_sub(release_collateral)
            .ok_or(ErrorCode::MarketMathOverflowV2)?;
        market.side0.reserve_ledger.live_reserve = market
            .side0
            .reserve_ledger
            .live_reserve
            .checked_add(repay_credit)
            .ok_or(ErrorCode::ReserveOverflow)?;
        market.side0.reserve_ledger.cash_reserve = market
            .side0
            .reserve_ledger
            .cash_reserve
            .checked_add(repay_credit)
            .ok_or(ErrorCode::ReserveOverflow)?;
    } else {
        let debt_before = margin_position.fixed_debt1(&market.debt_book)?;
        require_gte!(
            debt_before,
            repay_credit as u128,
            ErrorCode::InsufficientDebt
        );
        let shares_before = margin_position.fixed_debt1_shares;
        let shares_to_burn = if repay_credit as u128 == debt_before {
            shares_before
        } else {
            DebtBookV2::debt_to_shares(repay_credit, market.debt_book.borrow_index1_nad)?
                .min(shares_before)
        };
        let release_collateral = proportional_release(
            margin_position.recognized_collateral0_for_debt1,
            shares_to_burn,
            shares_before,
        )?;
        margin_position.fixed_debt1_shares = margin_position
            .fixed_debt1_shares
            .checked_sub(shares_to_burn)
            .ok_or(ErrorCode::MarketMathOverflowV2)?;
        margin_position.recognized_collateral0_for_debt1 = margin_position
            .recognized_collateral0_for_debt1
            .checked_sub(release_collateral)
            .ok_or(ErrorCode::MarketMathOverflowV2)?;
        market.debt_book.fixed_debt1_shares = market
            .debt_book
            .fixed_debt1_shares
            .checked_sub(shares_to_burn)
            .ok_or(ErrorCode::MarketMathOverflowV2)?;
        market.recognition_ledger.debt_bearing_collateral0_for_debt1 = market
            .recognition_ledger
            .debt_bearing_collateral0_for_debt1
            .checked_sub(release_collateral)
            .ok_or(ErrorCode::MarketMathOverflowV2)?;
        market.side1.reserve_ledger.live_reserve = market
            .side1
            .reserve_ledger
            .live_reserve
            .checked_add(repay_credit)
            .ok_or(ErrorCode::ReserveOverflow)?;
        market.side1.reserve_ledger.cash_reserve = market
            .side1
            .reserve_ledger
            .cash_reserve
            .checked_add(repay_credit)
            .ok_or(ErrorCode::ReserveOverflow)?;
    }
    market.refresh_market_health()?;
    Ok(())
}

fn require_borrow_headroom(debt_side: &MarketSideV2, borrow_amount: u64) -> Result<()> {
    require_gte!(
        debt_side.reserve_ledger.cash_reserve,
        borrow_amount,
        ErrorCode::InsufficientBorrowHeadroomV2
    );
    let next_reserve = debt_side
        .reserve_ledger
        .live_reserve
        .checked_sub(borrow_amount)
        .ok_or(ErrorCode::ReserveUnderflow)?;
    require_market_reserve_floor(
        next_reserve,
        debt_side.claim_ledger.protected_claim_supply,
        debt_side.buffer_book.required_buffer,
    )
}

fn proportional_release(recognized: u64, shares_to_burn: u128, shares_before: u128) -> Result<u64> {
    require!(shares_before > 0, ErrorCode::InsufficientDebt);
    if shares_to_burn == shares_before {
        return Ok(recognized);
    }
    let release = (recognized as u128)
        .checked_mul(shares_to_burn)
        .and_then(|value| value.checked_div(shares_before))
        .ok_or(ErrorCode::MarketMathOverflowV2)?;
    u64::try_from(release).map_err(|_| ErrorCode::MarketMathOverflowV2.into())
}
