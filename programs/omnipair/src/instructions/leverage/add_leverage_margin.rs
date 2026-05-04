use anchor_lang::prelude::*;
use anchor_spl::{
    token::{Mint, Token, TokenAccount},
    token_interface::Token2022,
};

use crate::{
    constants::*,
    errors::ErrorCode,
    events::{EventMetadata, LeveragePositionUpdatedEvent},
    state::{FutarchyAuthority, Pair, RateModel, UserLeverageDelegation, UserLeveragePosition},
    utils::token::transfer_from_user_to_vault,
};

use super::common::{
    approved_for, invoke_delegated_approval_callback, invoke_delegated_callback, quote_swap,
    require_leverage_not_liquidatable, split_delegated_accounts, token_program_for_mint,
    DelegatedCpiArgs, LEVERAGE_DELEGATE_ADD_MARGIN,
};

#[derive(AnchorSerialize, AnchorDeserialize, Clone)]
pub struct AddLeverageMarginArgs {
    pub is_debt_token0: bool,
    pub amount: u64,
}

#[derive(AnchorSerialize, AnchorDeserialize, Clone)]
pub struct DelegatedAddLeverageMarginArgs {
    pub is_debt_token0: bool,
    pub amount: u64,
    pub delegated: DelegatedCpiArgs,
}

#[derive(Clone, Copy)]
enum AddMarginMode {
    Owner,
    Delegate,
}

#[event_cpi]
#[derive(Accounts)]
#[instruction(args: AddLeverageMarginArgs)]
pub struct AddLeverageMargin<'info> {
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
            debt_token_mint.key().as_ref(),
        ],
        bump = pair.get_reserve_vault_bump(&debt_token_mint.key())
    )]
    pub debt_token_vault: Box<Account<'info, TokenAccount>>,

    #[account(
        mut,
        constraint = source_token_account.mint == debt_token_mint.key() @ ErrorCode::InvalidTokenAccount,
    )]
    pub source_token_account: Box<Account<'info, TokenAccount>>,

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

impl<'info> AddLeverageMargin<'info> {
    pub fn update_and_validate_add_margin(
        &mut self,
        args: &AddLeverageMarginArgs,
    ) -> Result<()> {
        let pair_key = self.pair.key();
        self.pair.update(
            &self.rate_model,
            &self.futarchy_authority,
            pair_key,
            Some(self.event_authority.to_account_info()),
        )?;

        let debt_token = if args.is_debt_token0 { self.pair.token0 } else { self.pair.token1 };
        require_keys_eq!(self.debt_token_mint.key(), debt_token, ErrorCode::InvalidMint);
        require!(args.amount > 0, ErrorCode::AmountZero);
        require!(self.user_leverage_position.debt_shares > 0, ErrorCode::ZeroDebtAmount);
        require!(
            self.user_leverage_position.collateral_amount > 0,
            ErrorCode::InsufficientAmount
        );
        Ok(())
    }

    pub fn update_and_validate_delegated_add_margin(
        &mut self,
        args: &DelegatedAddLeverageMarginArgs,
    ) -> Result<()> {
        self.update_and_validate_add_margin(&AddLeverageMarginArgs {
            is_debt_token0: args.is_debt_token0,
            amount: args.amount,
        })
    }

    pub fn handle_add_leverage_margin(
        ctx: Context<'_, '_, '_, 'info, Self>,
        args: AddLeverageMarginArgs,
    ) -> Result<()> {
        Self::execute(ctx, args, None, AddMarginMode::Owner)
    }

    pub fn handle_delegated_add_leverage_margin(
        ctx: Context<'_, '_, '_, 'info, Self>,
        args: DelegatedAddLeverageMarginArgs,
    ) -> Result<()> {
        Self::execute(
            ctx,
            AddLeverageMarginArgs {
                is_debt_token0: args.is_debt_token0,
                amount: args.amount,
            },
            Some(args.delegated),
            AddMarginMode::Delegate,
        )
    }

    fn execute(
        mut ctx: Context<'_, '_, '_, 'info, Self>,
        args: AddLeverageMarginArgs,
        delegated: Option<DelegatedCpiArgs>,
        mode: AddMarginMode,
    ) -> Result<()> {
        let delegated = match mode {
            AddMarginMode::Owner => DelegatedCpiArgs::default(),
            AddMarginMode::Delegate => delegated.ok_or(ErrorCode::InvalidLeverageDelegation)?,
        };
        let accounts = &mut ctx.accounts;
        let pair = &mut accounts.pair;
        let position = &mut accounts.user_leverage_position;
        let debt_amount = position.calculate_debt(pair)?;
        require_gt!(debt_amount, args.amount, ErrorCode::InsufficientDebt);
        let collateral_token0 = !args.is_debt_token0;
        let reserve_in = if collateral_token0 { pair.reserve0 } else { pair.reserve1 };
        let reserve_out = if collateral_token0 { pair.reserve1 } else { pair.reserve0 };
        let closeout_value = quote_swap(
            position.collateral_amount,
            reserve_in,
            reserve_out,
            pair.swap_fee_bps,
            accounts.futarchy_authority.revenue_share.swap_bps,
        )?
        .amount_out;
        let debt_after = debt_amount
            .checked_sub(args.amount)
            .ok_or(ErrorCode::DebtMathOverflow)?;
        require_leverage_not_liquidatable(closeout_value, debt_after)?;

        match mode {
            AddMarginMode::Owner => {
                require_keys_eq!(
                    accounts.authority.key(),
                    accounts.position_owner.key(),
                    ErrorCode::InvalidSigner
                );
                require_keys_eq!(
                    accounts.source_token_account.owner,
                    accounts.authority.key(),
                    ErrorCode::InvalidTokenAccount
                );
                require_gte!(
                    accounts.source_token_account.amount,
                    args.amount,
                    ErrorCode::InsufficientBalance
                );
            }
            AddMarginMode::Delegate => {
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
                approved_for(delegation.approved_actions, LEVERAGE_DELEGATE_ADD_MARGIN)?;
                let vault_balance_before = accounts.debt_token_vault.amount;
                let (before_accounts, _) =
                    split_delegated_accounts(ctx.remaining_accounts, delegated.before_accounts_len)?;
                let protected_accounts = [
                    pair.key(),
                    position.key(),
                    delegation.key(),
                    accounts.debt_token_vault.key(),
                ];
                let writable_protected_accounts = [accounts.debt_token_vault.key()];
                pair.exit(&crate::ID)?;
                position.exit(&crate::ID)?;
                invoke_delegated_approval_callback(
                    delegated_program,
                    delegated.before_ix_data.clone(),
                    before_accounts,
                    &protected_accounts,
                    &writable_protected_accounts,
                    LEVERAGE_DELEGATE_ADD_MARGIN,
                    pair.key(),
                    position.owner,
                    position.key(),
                    delegation.key(),
                    args.is_debt_token0,
                    Pubkey::default(),
                    Pubkey::default(),
                    0,
                )?;
                accounts.debt_token_vault.reload()?;
                require_gte!(
                    accounts.debt_token_vault.amount.saturating_sub(vault_balance_before),
                    args.amount,
                    ErrorCode::InvalidLeverageDelegation
                );
            }
        }

        if matches!(mode, AddMarginMode::Owner) {
            transfer_from_user_to_vault(
                accounts.authority.to_account_info(),
                accounts.source_token_account.to_account_info(),
                accounts.debt_token_vault.to_account_info(),
                accounts.debt_token_mint.to_account_info(),
                token_program_for_mint(
                    &accounts.debt_token_mint.to_account_info(),
                    &accounts.token_program.to_account_info(),
                    &accounts.token_2022_program.to_account_info(),
                ),
                args.amount,
                accounts.debt_token_mint.decimals,
            )?;
        }

        position.decrease_debt(pair, args.amount)?;
        position.margin_amount = position
            .margin_amount
            .checked_add(args.amount)
            .ok_or(ErrorCode::Overflow)?;
        let debt_after = position.calculate_debt(pair)?;

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
            AddMarginMode::Owner => Ok(()),
            AddMarginMode::Delegate => {
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
                    accounts.debt_token_vault.key(),
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
