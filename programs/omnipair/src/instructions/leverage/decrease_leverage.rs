use anchor_lang::prelude::*;
use anchor_spl::{
    token::{Mint, Token, TokenAccount},
    token_interface::Token2022,
};

use crate::{
    constants::*,
    errors::ErrorCode,
    events::{EventMetadata, LeveragePositionUpdatedEvent, SwapEvent},
    generate_gamm_pair_seeds,
    state::{FutarchyAuthority, Pair, RateModel, UserLeverageDelegation, UserLeveragePosition},
    utils::token::transfer_from_vault_to_vault,
};

use super::common::{
    approved_for, invoke_delegated_approval_callback, invoke_delegated_callback, quote_swap,
    require_leverage_not_liquidatable, split_delegated_accounts, token_program_for_mint,
    DelegatedCpiArgs, LEVERAGE_DELEGATE_DECREASE,
};

#[derive(AnchorSerialize, AnchorDeserialize, Clone)]
pub struct DecreaseLeverageArgs {
    pub is_debt_token0: bool,
    pub collateral_amount: u64,
    pub min_amount_out: u64,
}

#[derive(AnchorSerialize, AnchorDeserialize, Clone)]
pub struct DelegatedDecreaseLeverageArgs {
    pub is_debt_token0: bool,
    pub collateral_amount: u64,
    pub min_amount_out: u64,
    pub delegated: DelegatedCpiArgs,
}

#[derive(Clone, Copy)]
enum DecreaseMode {
    Owner,
    Delegate,
}

#[event_cpi]
#[derive(Accounts)]
#[instruction(args: DecreaseLeverageArgs)]
pub struct DecreaseLeverage<'info> {
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

    #[account(mut, address = pair.rate_model)]
    pub rate_model: Account<'info, RateModel>,

    #[account(seeds = [FUTARCHY_AUTHORITY_SEED_PREFIX], bump = futarchy_authority.bump)]
    pub futarchy_authority: Account<'info, FutarchyAuthority>,

    /// CHECK: Position owner. Owner mode requires this to sign via `authority`.
    #[account(address = user_leverage_position.owner)]
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
            RESERVE_VAULT_SEED_PREFIX,
            pair.key().as_ref(),
            debt_token_mint.key().as_ref(),
        ],
        bump = pair.get_reserve_vault_bump(&debt_token_mint.key())
    )]
    pub debt_token_vault: Box<Account<'info, TokenAccount>>,

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
        constraint = collateral_token_mint.key() == pair.token0 || collateral_token_mint.key() == pair.token1 @ ErrorCode::InvalidMint
    )]
    pub collateral_token_mint: Box<Account<'info, Mint>>,

    #[account(
        constraint = debt_token_mint.key() == pair.token0 || debt_token_mint.key() == pair.token1 @ ErrorCode::InvalidMint
    )]
    pub debt_token_mint: Box<Account<'info, Mint>>,

    pub user_leverage_delegation: Option<Account<'info, UserLeverageDelegation>>,

    /// CHECK: Optional delegated program, validated in delegated mode.
    pub delegated_program: Option<UncheckedAccount<'info>>,

    #[account(mut)]
    pub authority: Signer<'info>,
    pub token_program: Program<'info, Token>,
    pub token_2022_program: Program<'info, Token2022>,
    pub system_program: Program<'info, System>,
}

impl<'info> DecreaseLeverage<'info> {
    pub fn update_and_validate_decrease(
        &mut self,
        args: &DecreaseLeverageArgs,
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
        require_keys_eq!(self.debt_token_mint.key(), debt_token, ErrorCode::InvalidMint);
        require_keys_eq!(self.collateral_token_mint.key(), collateral_token, ErrorCode::InvalidMint);
        require!(args.collateral_amount > 0, ErrorCode::AmountZero);
        require!(self.user_leverage_position.debt_shares > 0, ErrorCode::ZeroDebtAmount);
        require_gt!(
            self.user_leverage_position.collateral_amount,
            args.collateral_amount,
            ErrorCode::InsufficientAmount
        );
        Ok(())
    }

    pub fn update_and_validate_delegated_decrease(
        &mut self,
        args: &DelegatedDecreaseLeverageArgs,
    ) -> Result<()> {
        self.update_and_validate_decrease(&DecreaseLeverageArgs {
            is_debt_token0: args.is_debt_token0,
            collateral_amount: args.collateral_amount,
            min_amount_out: args.min_amount_out,
        })
    }

    pub fn handle_decrease_leverage(
        ctx: Context<'_, '_, '_, 'info, Self>,
        args: DecreaseLeverageArgs,
    ) -> Result<()> {
        Self::execute(ctx, args, None, DecreaseMode::Owner)
    }

    pub fn handle_delegated_decrease_leverage(
        ctx: Context<'_, '_, '_, 'info, Self>,
        args: DelegatedDecreaseLeverageArgs,
    ) -> Result<()> {
        Self::execute(
            ctx,
            DecreaseLeverageArgs {
                is_debt_token0: args.is_debt_token0,
                collateral_amount: args.collateral_amount,
                min_amount_out: args.min_amount_out,
            },
            Some(args.delegated),
            DecreaseMode::Delegate,
        )
    }

    fn execute(
        mut ctx: Context<'_, '_, '_, 'info, Self>,
        args: DecreaseLeverageArgs,
        delegated: Option<DelegatedCpiArgs>,
        mode: DecreaseMode,
    ) -> Result<()> {
        let delegated = match mode {
            DecreaseMode::Owner => DelegatedCpiArgs::default(),
            DecreaseMode::Delegate => delegated.ok_or(ErrorCode::InvalidLeverageDelegation)?,
        };
        let accounts = &mut ctx.accounts;
        let pair = &mut accounts.pair;
        let position = &mut accounts.user_leverage_position;
        let debt_before = position.calculate_debt(pair)?;
        let is_token0_in = !args.is_debt_token0;
        let reserve_in = if is_token0_in { pair.reserve0 } else { pair.reserve1 };
        let reserve_out = if is_token0_in { pair.reserve1 } else { pair.reserve0 };
        let quote = quote_swap(
            args.collateral_amount,
            reserve_in,
            reserve_out,
            pair.swap_fee_bps,
            accounts.futarchy_authority.revenue_share.swap_bps,
        )?;
        require_gte!(
            quote.amount_out,
            args.min_amount_out,
            ErrorCode::SlippageExceeded
        );
        require_gt!(debt_before, quote.amount_out, ErrorCode::InsufficientDebt);

        let post_reserve_in = reserve_in
            .checked_add(quote.amount_in_with_lp_fee)
            .ok_or(ErrorCode::Overflow)?;
        let post_reserve_out = reserve_out
            .checked_sub(quote.amount_out)
            .ok_or(ErrorCode::ReserveUnderflow)?;
        let collateral_after = position
            .collateral_amount
            .checked_sub(args.collateral_amount)
            .ok_or(ErrorCode::InsufficientAmount)?;
        let debt_after = debt_before
            .checked_sub(quote.amount_out)
            .ok_or(ErrorCode::DebtMathOverflow)?;
        let closeout_value = quote_swap(
            collateral_after,
            post_reserve_in,
            post_reserve_out,
            pair.swap_fee_bps,
            accounts.futarchy_authority.revenue_share.swap_bps,
        )?
        .amount_out;
        require_leverage_not_liquidatable(closeout_value, debt_after)?;

        match mode {
            DecreaseMode::Owner => {
                require_keys_eq!(
                    accounts.authority.key(),
                    accounts.position_owner.key(),
                    ErrorCode::InvalidSigner
                );
            }
            DecreaseMode::Delegate => {
                let delegation = accounts
                    .user_leverage_delegation
                    .as_ref()
                    .ok_or(ErrorCode::InvalidLeverageDelegation)?;
                let delegated_program = accounts
                    .delegated_program
                    .as_ref()
                    .ok_or(ErrorCode::InvalidLeverageDelegation)?;
                require_keys_eq!(delegation.owner, position.owner, ErrorCode::InvalidLeverageDelegation);
                require_keys_eq!(delegation.pair, pair.key(), ErrorCode::InvalidLeverageDelegation);
                require_keys_eq!(delegation.position, position.key(), ErrorCode::InvalidLeverageDelegation);
                require!(delegation.is_debt_token0 == args.is_debt_token0, ErrorCode::InvalidLeverageDelegation);
                require_keys_eq!(
                    delegation.delegated_program,
                    delegated_program.key(),
                    ErrorCode::InvalidLeverageDelegation
                );
                approved_for(delegation.approved_actions, LEVERAGE_DELEGATE_DECREASE)?;
                let (before_accounts, _) =
                    split_delegated_accounts(ctx.remaining_accounts, delegated.before_accounts_len)?;
                let protected_accounts = [
                    pair.key(),
                    position.key(),
                    delegation.key(),
                    accounts.collateral_token_vault.key(),
                    accounts.debt_token_vault.key(),
                    accounts.leverage_collateral_vault.key(),
                ];
                pair.exit(&crate::ID)?;
                position.exit(&crate::ID)?;
                invoke_delegated_approval_callback(
                    delegated_program,
                    delegated.before_ix_data.clone(),
                    before_accounts,
                    &protected_accounts,
                    &[],
                    LEVERAGE_DELEGATE_DECREASE,
                    pair.key(),
                    position.owner,
                    position.key(),
                    delegation.key(),
                    args.is_debt_token0,
                    Pubkey::default(),
                    Pubkey::default(),
                    0,
                )?;
            }
        }

        let last_k = pair.k();
        match is_token0_in {
            true => {
                pair.reserve0 = post_reserve_in;
                pair.cash_reserve0 = pair
                    .cash_reserve0
                    .checked_add(quote.amount_in_with_lp_fee)
                    .ok_or(ErrorCode::Overflow)?;
                pair.reserve1 = post_reserve_out;
            }
            false => {
                pair.reserve1 = post_reserve_in;
                pair.cash_reserve1 = pair
                    .cash_reserve1
                    .checked_add(quote.amount_in_with_lp_fee)
                    .ok_or(ErrorCode::Overflow)?;
                pair.reserve0 = post_reserve_out;
            }
        }
        require_gte!(pair.k(), last_k, ErrorCode::BrokenInvariant);

        position.reduce_debt_from_closeout(pair, quote.amount_out)?;
        position.collateral_amount = collateral_after;

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
            args.collateral_amount,
            accounts.collateral_token_mint.decimals,
            &[&generate_gamm_pair_seeds!(pair)[..]],
        )?;
        let debt_after = position.calculate_debt(pair)?;

        emit!(SwapEvent {
            metadata: EventMetadata::new(accounts.authority.key(), pair.key()),
            reserve0: pair.reserve0,
            reserve1: pair.reserve1,
            is_token0_in,
            amount_in: args.collateral_amount,
            amount_out: quote.amount_out,
            amount_in_after_fee: quote.amount_in_after_swap_fee,
            lp_fee: quote.lp_fee,
            protocol_fee: quote.protocol_fee,
        });
        emit!(LeveragePositionUpdatedEvent {
            metadata: EventMetadata::new(accounts.authority.key(), pair.key()),
            position: position.key(),
            owner: position.owner,
            is_debt_token0: args.is_debt_token0,
            collateral_amount: position.collateral_amount,
            debt_amount: debt_after,
            debt_shares: position.debt_shares,
            margin_amount: position.margin_amount,
            closeout_value,
            equity: closeout_value as i128 - debt_after as i128,
        });

        match mode {
            DecreaseMode::Owner => Ok(()),
            DecreaseMode::Delegate => {
                let delegated_program = accounts
                    .delegated_program
                    .as_ref()
                    .ok_or(ErrorCode::InvalidLeverageDelegation)?;
                let delegation_key = accounts
                    .user_leverage_delegation
                    .as_ref()
                    .ok_or(ErrorCode::InvalidLeverageDelegation)?
                    .key();
                let protected_accounts = [
                    pair.key(),
                    position.key(),
                    delegation_key,
                    accounts.collateral_token_vault.key(),
                    accounts.debt_token_vault.key(),
                    accounts.leverage_collateral_vault.key(),
                ];
                pair.exit(&crate::ID)?;
                position.exit(&crate::ID)?;
                let (_, after_accounts) =
                    split_delegated_accounts(ctx.remaining_accounts, delegated.before_accounts_len)?;
                invoke_delegated_callback(
                    delegated_program,
                    delegated.after_ix_data,
                    after_accounts,
                    &protected_accounts,
                    &[],
                )
            }
        }
    }
}
