use anchor_lang::prelude::*;
use anchor_spl::{
    token::{Mint, Token, TokenAccount},
    token_interface::Token2022,
};

use crate::{
    constants::*,
    errors::ErrorCode,
    events::{EventMetadata, LeveragePositionClosedEvent, SwapEvent},
    generate_gamm_pair_seeds,
    state::{FutarchyAuthority, Pair, RateModel, UserLeverageDelegation, UserLeveragePosition},
    utils::token::{transfer_from_vault_to_user, transfer_from_vault_to_vault},
};

use super::common::{
    approved_for, invoke_delegated_approval_callback, invoke_delegated_callback, quote_swap,
    split_delegated_accounts, token_program_for_mint, DelegatedCpiArgs, LEVERAGE_DELEGATE_CLOSE,
};

#[derive(AnchorSerialize, AnchorDeserialize, Clone)]
pub struct CloseLeverageArgs {
    pub is_debt_token0: bool,
    pub min_amount_out: u64,
}

#[derive(AnchorSerialize, AnchorDeserialize, Clone)]
pub struct DelegatedCloseLeverageArgs {
    pub is_debt_token0: bool,
    pub min_amount_out: u64,
    pub delegated: DelegatedCpiArgs,
}

#[derive(Clone, Copy)]
enum CloseMode {
    Owner,
    Delegate,
}

#[event_cpi]
#[derive(Accounts)]
#[instruction(args: CloseLeverageArgs)]
pub struct CloseLeverage<'info> {
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

    /// CHECK: Position owner receives closed account rent.
    #[account(mut, address = user_leverage_position.owner)]
    pub position_owner: AccountInfo<'info>,

    #[account(
        mut,
        close = position_owner,
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
        mut,
        seeds = [
            LEVERAGE_COLLATERAL_VAULT_SEED_PREFIX,
            pair.key().as_ref(),
            token_in_mint.key().as_ref(),
        ],
        bump,
        constraint = leverage_collateral_vault.mint == token_in_mint.key() @ ErrorCode::InvalidVault,
        constraint = leverage_collateral_vault.owner == pair.key() @ ErrorCode::InvalidVault
    )]
    pub leverage_collateral_vault: Box<Account<'info, TokenAccount>>,

    #[account(
        mut,
        constraint = recipient_token_out_account.mint == token_out_mint.key() @ ErrorCode::InvalidTokenAccount,
    )]
    pub recipient_token_out_account: Box<Account<'info, TokenAccount>>,

    #[account(
        constraint = token_in_mint.key() == pair.token0 || token_in_mint.key() == pair.token1 @ ErrorCode::InvalidMint
    )]
    pub token_in_mint: Box<Account<'info, Mint>>,

    #[account(
        constraint = token_out_mint.key() == pair.token0 || token_out_mint.key() == pair.token1 @ ErrorCode::InvalidMint
    )]
    pub token_out_mint: Box<Account<'info, Mint>>,

    pub user_leverage_delegation: Option<Account<'info, UserLeverageDelegation>>,

    /// CHECK: Optional delegated program, validated in delegated mode.
    pub delegated_program: Option<UncheckedAccount<'info>>,

    #[account(mut)]
    pub authority: Signer<'info>,
    pub token_program: Program<'info, Token>,
    pub token_2022_program: Program<'info, Token2022>,
    pub system_program: Program<'info, System>,
}

impl<'info> CloseLeverage<'info> {
    pub fn update_and_validate_close(&mut self, args: &CloseLeverageArgs) -> Result<()> {
        let pair_key = self.pair.key();
        self.pair.update(
            &self.rate_model,
            &self.futarchy_authority,
            pair_key,
            Some(self.event_authority.to_account_info()),
        )?;

        let debt_token = if args.is_debt_token0 { self.pair.token0 } else { self.pair.token1 };
        let collateral_token = self.pair.get_token_y(&debt_token);
        require_keys_eq!(self.token_in_mint.key(), collateral_token, ErrorCode::InvalidMint);
        require_keys_eq!(self.token_out_mint.key(), debt_token, ErrorCode::InvalidMint);
        require!(self.user_leverage_position.debt_shares > 0, ErrorCode::ZeroDebtAmount);
        require!(
            self.user_leverage_position.collateral_amount > 0,
            ErrorCode::InsufficientAmount
        );
        Ok(())
    }

    pub fn update_and_validate_delegated_close(
        &mut self,
        args: &DelegatedCloseLeverageArgs,
    ) -> Result<()> {
        self.update_and_validate_close(&CloseLeverageArgs {
            is_debt_token0: args.is_debt_token0,
            min_amount_out: args.min_amount_out,
        })
    }

    pub fn handle_close_leverage(
        ctx: Context<'_, '_, '_, 'info, Self>,
        args: CloseLeverageArgs,
    ) -> Result<()> {
        Self::execute(ctx, args, None, CloseMode::Owner)
    }

    pub fn handle_delegated_close_leverage(
        ctx: Context<'_, '_, '_, 'info, Self>,
        args: DelegatedCloseLeverageArgs,
    ) -> Result<()> {
        Self::execute(
            ctx,
            CloseLeverageArgs {
                is_debt_token0: args.is_debt_token0,
                min_amount_out: args.min_amount_out,
            },
            Some(args.delegated),
            CloseMode::Delegate,
        )
    }

    fn execute(
        mut ctx: Context<'_, '_, '_, 'info, Self>,
        args: CloseLeverageArgs,
        delegated: Option<DelegatedCpiArgs>,
        mode: CloseMode,
    ) -> Result<()> {
        let delegated = match mode {
            CloseMode::Owner => DelegatedCpiArgs::default(),
            CloseMode::Delegate => delegated.ok_or(ErrorCode::InvalidLeverageDelegation)?,
        };
        let accounts = &mut ctx.accounts;
        let pair = &mut accounts.pair;
        let position = &mut accounts.user_leverage_position;
        let debt_amount = position.calculate_debt(pair)?;
        require_gt!(debt_amount, 0, ErrorCode::ZeroDebtAmount);

        let is_token0_in = !args.is_debt_token0;
        let reserve_in = if is_token0_in { pair.reserve0 } else { pair.reserve1 };
        let reserve_out = if is_token0_in { pair.reserve1 } else { pair.reserve0 };
        let quote = quote_swap(
            position.collateral_amount,
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
        require_gte!(quote.amount_out, debt_amount, ErrorCode::InsufficientAmount);

        let residual = quote
            .amount_out
            .checked_sub(debt_amount)
            .ok_or(ErrorCode::Overflow)?;
        match args.is_debt_token0 {
            true => require_gte!(pair.cash_reserve0, residual, ErrorCode::InsufficientCashReserve0),
            false => require_gte!(pair.cash_reserve1, residual, ErrorCode::InsufficientCashReserve1),
        }

        match mode {
            CloseMode::Owner => {
                require_keys_eq!(
                    accounts.authority.key(),
                    accounts.position_owner.key(),
                    ErrorCode::InvalidSigner
                );
            }
            CloseMode::Delegate => {
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
                approved_for(delegation.approved_actions, LEVERAGE_DELEGATE_CLOSE)?;
                let (before_accounts, _) =
                    split_delegated_accounts(ctx.remaining_accounts, delegated.before_accounts_len)?;
                let protected_accounts = [
                    pair.key(),
                    position.key(),
                    delegation.key(),
                    accounts.token_in_vault.key(),
                    accounts.token_out_vault.key(),
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
                    LEVERAGE_DELEGATE_CLOSE,
                    pair.key(),
                    position.owner,
                    position.key(),
                    delegation.key(),
                    args.is_debt_token0,
                    accounts.recipient_token_out_account.key(),
                    accounts.token_out_mint.key(),
                    residual,
                )?;
            }
        }

        let last_k = pair.k();
        match is_token0_in {
            true => {
                pair.reserve0 = pair
                    .reserve0
                    .checked_add(quote.amount_in_with_lp_fee)
                    .ok_or(ErrorCode::Overflow)?;
                pair.cash_reserve0 = pair
                    .cash_reserve0
                    .checked_add(quote.amount_in_with_lp_fee)
                    .ok_or(ErrorCode::Overflow)?;
                pair.reserve1 = pair
                    .reserve1
                    .checked_sub(quote.amount_out)
                    .ok_or(ErrorCode::ReserveUnderflow)?;
                pair.cash_reserve1 = pair
                    .cash_reserve1
                    .checked_sub(residual)
                    .ok_or(ErrorCode::CashReserveUnderflow)?;
            }
            false => {
                pair.reserve1 = pair
                    .reserve1
                    .checked_add(quote.amount_in_with_lp_fee)
                    .ok_or(ErrorCode::Overflow)?;
                pair.cash_reserve1 = pair
                    .cash_reserve1
                    .checked_add(quote.amount_in_with_lp_fee)
                    .ok_or(ErrorCode::Overflow)?;
                pair.reserve0 = pair
                    .reserve0
                    .checked_sub(quote.amount_out)
                    .ok_or(ErrorCode::ReserveUnderflow)?;
                pair.cash_reserve0 = pair
                    .cash_reserve0
                    .checked_sub(residual)
                    .ok_or(ErrorCode::CashReserveUnderflow)?;
            }
        }
        require_gte!(pair.k(), last_k, ErrorCode::BrokenInvariant);

        position.clear_debt(pair, debt_amount)?;

        transfer_from_vault_to_vault(
            pair.to_account_info(),
            accounts.leverage_collateral_vault.to_account_info(),
            accounts.token_in_vault.to_account_info(),
            accounts.token_in_mint.to_account_info(),
            token_program_for_mint(
                &accounts.token_in_mint.to_account_info(),
                &accounts.token_program.to_account_info(),
                &accounts.token_2022_program.to_account_info(),
            ),
            position.collateral_amount,
            accounts.token_in_mint.decimals,
            &[&generate_gamm_pair_seeds!(pair)[..]],
        )?;
        if residual > 0 {
            transfer_from_vault_to_user(
                pair.to_account_info(),
                accounts.token_out_vault.to_account_info(),
                accounts.recipient_token_out_account.to_account_info(),
                accounts.token_out_mint.to_account_info(),
                token_program_for_mint(
                    &accounts.token_out_mint.to_account_info(),
                    &accounts.token_program.to_account_info(),
                    &accounts.token_2022_program.to_account_info(),
                ),
                residual,
                accounts.token_out_mint.decimals,
                &[&generate_gamm_pair_seeds!(pair)[..]],
            )?;
        }

        let collateral_sold = position.collateral_amount;
        position.collateral_amount = 0;

        emit!(SwapEvent {
            metadata: EventMetadata::new(accounts.authority.key(), pair.key()),
            reserve0: pair.reserve0,
            reserve1: pair.reserve1,
            is_token0_in,
            amount_in: collateral_sold,
            amount_out: quote.amount_out,
            amount_in_after_fee: quote.amount_in_after_swap_fee,
            lp_fee: quote.lp_fee,
            protocol_fee: quote.protocol_fee,
        });
        emit!(LeveragePositionClosedEvent {
            metadata: EventMetadata::new(accounts.authority.key(), pair.key()),
            position: position.key(),
            owner: accounts.position_owner.key(),
            is_debt_token0: args.is_debt_token0,
            debt_repaid: debt_amount,
            collateral_sold,
            closeout_value: quote.amount_out,
            residual,
        });

        match mode {
            CloseMode::Owner => Ok(()),
            CloseMode::Delegate => {
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
                    accounts.token_in_vault.key(),
                    accounts.token_out_vault.key(),
                    accounts.leverage_collateral_vault.key(),
                    accounts.recipient_token_out_account.key(),
                ];
                let writable_protected_accounts = [accounts.recipient_token_out_account.key()];
                pair.exit(&crate::ID)?;
                position.exit(&crate::ID)?;
                let (_, after_accounts) =
                    split_delegated_accounts(ctx.remaining_accounts, delegated.before_accounts_len)?;
                invoke_delegated_callback(
                    delegated_program,
                    delegated.after_ix_data,
                    after_accounts,
                    &protected_accounts,
                    &writable_protected_accounts,
                )
            }
        }
    }
}
