use anchor_lang::prelude::*;
use anchor_spl::{
    token::Token,
    token_interface::{Mint, Token2022, TokenAccount},
};

use crate::{
    constants::*,
    errors::ErrorCode,
    events::{MarketEventMetadata, MarketReserveDeposited},
    generate_market_seeds,
    shared::{
        account::get_size_with_discriminator,
        token::{token_mint_to, transfer_from_user_to_vault},
    },
    state::{Market, StakePosition},
    transitions::reserve::AddLiquidity as AddLiquidityTransition,
};

use crate::instructions::common::{
    require_fee_free_claim_token_mint, require_supported_asset_mint, token_program_for_mint,
    validate_reserve_accounts,
};

#[derive(AnchorSerialize, AnchorDeserialize, Clone)]
pub struct AddLiquidityArgs {
    pub market_side_index: u8,
    pub deposit_amount: u64,
    pub min_claim_amount: u64,
    pub max_buffer_amount: u64,
}

#[event_cpi]
#[derive(Accounts)]
#[instruction(args: AddLiquidityArgs)]
pub struct AddLiquidity<'info> {
    #[account(
        mut,
        seeds = [
            MARKET_V2_SEED_PREFIX,
            market.base_mint.as_ref(),
            market.quote_mint.as_ref(),
            market.params_hash.as_ref(),
        ],
        bump = market.bump
    )]
    pub market: Box<Account<'info, Market>>,

    #[account(mut)]
    pub owner: Signer<'info>,

    pub asset_mint: Box<InterfaceAccount<'info, Mint>>,

    #[account(mut)]
    pub claim_token_mint: Box<InterfaceAccount<'info, Mint>>,

    #[account(mut)]
    pub reserve_vault: Box<InterfaceAccount<'info, TokenAccount>>,

    #[account(mut)]
    pub owner_asset_account: Box<InterfaceAccount<'info, TokenAccount>>,

    #[account(mut)]
    pub owner_claim_account: Box<InterfaceAccount<'info, TokenAccount>>,

    #[account(
        init_if_needed,
        payer = owner,
        space = get_size_with_discriminator::<StakePosition>(),
        seeds = [
            STAKE_POSITION_SEED_PREFIX,
            market.key().as_ref(),
            owner.key().as_ref(),
            asset_mint.key().as_ref(),
        ],
        bump
    )]
    pub stake_position: Box<Account<'info, StakePosition>>,

    pub token_program: Program<'info, Token>,
    pub token_2022_program: Program<'info, Token2022>,
    pub system_program: Program<'info, System>,
}

impl<'info> AddLiquidity<'info> {
    pub fn validate(&self, args: &AddLiquidityArgs) -> Result<()> {
        self.market.assert_live()?;
        require!(args.deposit_amount > 0, ErrorCode::AmountZero);
        require_gte!(
            self.owner_asset_account.amount,
            args.deposit_amount,
            ErrorCode::InsufficientBalance
        );
        validate_reserve_accounts(
            &self.market,
            args.market_side_index,
            self.owner.key(),
            &self.asset_mint,
            &self.claim_token_mint,
            &self.reserve_vault,
            &self.owner_asset_account,
            &self.owner_claim_account,
        )?;
        require_supported_asset_mint(&self.asset_mint)?;
        require_fee_free_claim_token_mint(&self.claim_token_mint)?;

        if self.stake_position.is_initialized() {
            self.stake_position.assert_position(
                self.owner.key(),
                self.market.key(),
                self.asset_mint.key(),
            )?;
        }

        Ok(())
    }

    pub fn handle_add_liquidity(ctx: Context<Self>, args: AddLiquidityArgs) -> Result<()> {
        let market_key = ctx.accounts.market.key();
        let owner_key = ctx.accounts.owner.key();
        let asset_mint_key = ctx.accounts.asset_mint.key();

        if !ctx.accounts.stake_position.is_initialized() {
            ctx.accounts.stake_position.initialize(
                owner_key,
                market_key,
                asset_mint_key,
                ctx.bumps.stake_position,
            );
        }
        ctx.accounts
            .stake_position
            .assert_position(owner_key, market_key, asset_mint_key)?;

        let reserve_balance_before = ctx.accounts.reserve_vault.amount;
        let asset_token_program = token_program_for_mint(
            &ctx.accounts.asset_mint,
            &ctx.accounts.token_program,
            &ctx.accounts.token_2022_program,
        )?;

        transfer_from_user_to_vault(
            ctx.accounts.owner.to_account_info(),
            ctx.accounts.owner_asset_account.to_account_info(),
            ctx.accounts.reserve_vault.to_account_info(),
            ctx.accounts.asset_mint.to_account_info(),
            asset_token_program,
            args.deposit_amount,
            ctx.accounts.asset_mint.decimals,
        )?;
        ctx.accounts.reserve_vault.reload()?;

        let reserve_credit = ctx
            .accounts
            .reserve_vault
            .amount
            .checked_sub(reserve_balance_before)
            .ok_or(ErrorCode::MarketMathOverflow)?;
        let receipt = {
            let market_side = ctx.accounts.market.side_mut(args.market_side_index)?;
            AddLiquidityTransition::new(reserve_credit).apply(market_side)?
        };
        require_gte!(
            receipt.claim_amount,
            args.min_claim_amount,
            ErrorCode::SlippageExceeded
        );
        require_gte!(
            args.max_buffer_amount,
            receipt.buffer_amount,
            ErrorCode::SlippageExceeded
        );

        ctx.accounts
            .stake_position
            .credit_buffer_share_amount(receipt.buffer_amount)?;

        let claim_token_program = token_program_for_mint(
            &ctx.accounts.claim_token_mint,
            &ctx.accounts.token_program,
            &ctx.accounts.token_2022_program,
        )?;
        token_mint_to(
            ctx.accounts.market.to_account_info(),
            claim_token_program,
            ctx.accounts.claim_token_mint.to_account_info(),
            ctx.accounts.owner_claim_account.to_account_info(),
            receipt.claim_amount,
            &[&generate_market_seeds!(ctx.accounts.market)[..]],
        )?;

        emit_cpi!(MarketReserveDeposited {
            market: market_key,
            owner: owner_key,
            asset_mint: asset_mint_key,
            reserve_credit: receipt.reserve_credit,
            claim_amount: receipt.claim_amount,
            buffer_amount: receipt.buffer_amount,
            protected_claim_token_supply: receipt.protected_claim_token_supply,
            required_buffer: receipt.required_buffer,
            metadata: MarketEventMetadata::new(owner_key, market_key)?,
        });

        Ok(())
    }
}
