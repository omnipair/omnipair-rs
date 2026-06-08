use anchor_lang::{prelude::*, solana_program::program_option::COption};
use anchor_spl::{
    token::Token,
    token_interface::{Mint, Token2022, TokenAccount},
};

use crate::{
    constants::*,
    errors::ErrorCode,
    events::{MarketClaimRedeemedV2, MarketEventMetadataV2, MarketReserveDepositedV2},
    generate_market_v2_seeds,
    state::{MarketV2, StakePositionV2},
    utils::{
        account::get_size_with_discriminator,
        token::{
            is_fee_free_mint, is_supported_mint, token_burn, token_mint_to,
            transfer_from_user_to_vault, transfer_from_vault_to_user,
        },
    },
};

#[derive(AnchorSerialize, AnchorDeserialize, Clone)]
pub struct DepositReserveV2Args {
    pub market_side_index: u8,
    pub deposit_amount: u64,
    pub min_claim_amount: u64,
    pub max_buffer_amount: u64,
}

#[derive(AnchorSerialize, AnchorDeserialize, Clone)]
pub struct RedeemClaimV2Args {
    pub market_side_index: u8,
    pub claim_amount: u64,
    pub min_asset_amount_out: u64,
}

#[event_cpi]
#[derive(Accounts)]
#[instruction(args: DepositReserveV2Args)]
pub struct DepositReserveV2<'info> {
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
    pub market: Account<'info, MarketV2>,

    #[account(mut)]
    pub owner: Signer<'info>,

    pub asset_mint: InterfaceAccount<'info, Mint>,

    #[account(mut)]
    pub claim_mint: InterfaceAccount<'info, Mint>,

    #[account(mut)]
    pub reserve_vault: InterfaceAccount<'info, TokenAccount>,

    #[account(mut)]
    pub owner_asset_account: InterfaceAccount<'info, TokenAccount>,

    #[account(mut)]
    pub owner_claim_account: InterfaceAccount<'info, TokenAccount>,

    #[account(
        init_if_needed,
        payer = owner,
        space = get_size_with_discriminator::<StakePositionV2>(),
        seeds = [
            STAKE_POSITION_V2_SEED_PREFIX,
            market.key().as_ref(),
            owner.key().as_ref(),
            asset_mint.key().as_ref(),
        ],
        bump
    )]
    pub stake_position: Account<'info, StakePositionV2>,

    pub token_program: Program<'info, Token>,
    pub token_2022_program: Program<'info, Token2022>,
    pub system_program: Program<'info, System>,
}

impl<'info> DepositReserveV2<'info> {
    pub fn validate(&self, args: &DepositReserveV2Args) -> Result<()> {
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
            &self.claim_mint,
            &self.reserve_vault,
            &self.owner_asset_account,
            &self.owner_claim_account,
        )?;
        require!(
            is_supported_mint(&self.asset_mint)?,
            ErrorCode::InvalidTokenProgram
        );
        require!(
            is_fee_free_mint(&self.claim_mint)?,
            ErrorCode::InvalidClaimMintV2
        );

        if self.stake_position.is_initialized() {
            self.stake_position.assert_position(
                self.owner.key(),
                self.market.key(),
                self.asset_mint.key(),
            )?;
        }

        Ok(())
    }

    pub fn handle_deposit(ctx: Context<Self>, args: DepositReserveV2Args) -> Result<()> {
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
            .ok_or(ErrorCode::MarketMathOverflowV2)?;
        let (claim_amount, buffer_amount, protected_claim_supply, required_buffer) = {
            let market_side = ctx.accounts.market.side_mut(args.market_side_index)?;
            let (claim_amount, buffer_amount) =
                market_side.apply_reserve_deposit(reserve_credit)?;
            (
                claim_amount,
                buffer_amount,
                market_side.claim_ledger.protected_claim_supply,
                market_side.buffer_book.required_buffer,
            )
        };
        require_gte!(
            claim_amount,
            args.min_claim_amount,
            ErrorCode::SlippageExceeded
        );
        require_gte!(
            args.max_buffer_amount,
            buffer_amount,
            ErrorCode::SlippageExceeded
        );

        ctx.accounts
            .stake_position
            .credit_buffer_shares(buffer_amount)?;

        let claim_token_program = token_program_for_mint(
            &ctx.accounts.claim_mint,
            &ctx.accounts.token_program,
            &ctx.accounts.token_2022_program,
        )?;
        token_mint_to(
            ctx.accounts.market.to_account_info(),
            claim_token_program,
            ctx.accounts.claim_mint.to_account_info(),
            ctx.accounts.owner_claim_account.to_account_info(),
            claim_amount,
            &[&generate_market_v2_seeds!(ctx.accounts.market)[..]],
        )?;

        emit_cpi!(MarketReserveDepositedV2 {
            market: market_key,
            owner: owner_key,
            asset_mint: asset_mint_key,
            reserve_credit,
            claim_amount,
            buffer_amount,
            protected_claim_supply,
            required_buffer,
            metadata: MarketEventMetadataV2::new(owner_key, market_key),
        });

        Ok(())
    }
}

#[event_cpi]
#[derive(Accounts)]
#[instruction(args: RedeemClaimV2Args)]
pub struct RedeemClaimV2<'info> {
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
    pub market: Account<'info, MarketV2>,

    #[account(mut)]
    pub owner: Signer<'info>,

    pub asset_mint: InterfaceAccount<'info, Mint>,

    #[account(mut)]
    pub claim_mint: InterfaceAccount<'info, Mint>,

    #[account(mut)]
    pub reserve_vault: InterfaceAccount<'info, TokenAccount>,

    #[account(mut)]
    pub owner_asset_account: InterfaceAccount<'info, TokenAccount>,

    #[account(mut)]
    pub owner_claim_account: InterfaceAccount<'info, TokenAccount>,

    pub token_program: Program<'info, Token>,
    pub token_2022_program: Program<'info, Token2022>,
}

impl<'info> RedeemClaimV2<'info> {
    pub fn validate(&self, args: &RedeemClaimV2Args) -> Result<()> {
        self.market.assert_started()?;
        require!(args.claim_amount > 0, ErrorCode::AmountZero);
        require_gte!(
            self.owner_claim_account.amount,
            args.claim_amount,
            ErrorCode::InsufficientBalance
        );
        require_gte!(
            args.claim_amount,
            args.min_asset_amount_out,
            ErrorCode::SlippageExceeded
        );
        validate_reserve_accounts(
            &self.market,
            args.market_side_index,
            self.owner.key(),
            &self.asset_mint,
            &self.claim_mint,
            &self.reserve_vault,
            &self.owner_asset_account,
            &self.owner_claim_account,
        )?;
        require!(
            is_supported_mint(&self.asset_mint)?,
            ErrorCode::InvalidTokenProgram
        );
        require!(
            is_fee_free_mint(&self.claim_mint)?,
            ErrorCode::InvalidClaimMintV2
        );
        Ok(())
    }

    pub fn handle_redeem(ctx: Context<Self>, args: RedeemClaimV2Args) -> Result<()> {
        let market_key = ctx.accounts.market.key();
        let owner_key = ctx.accounts.owner.key();
        let asset_mint_key = ctx.accounts.asset_mint.key();

        let claim_token_program = token_program_for_mint(
            &ctx.accounts.claim_mint,
            &ctx.accounts.token_program,
            &ctx.accounts.token_2022_program,
        )?;
        token_burn(
            ctx.accounts.owner.to_account_info(),
            claim_token_program,
            ctx.accounts.claim_mint.to_account_info(),
            ctx.accounts.owner_claim_account.to_account_info(),
            args.claim_amount,
            &[],
        )?;

        let asset_token_program = token_program_for_mint(
            &ctx.accounts.asset_mint,
            &ctx.accounts.token_program,
            &ctx.accounts.token_2022_program,
        )?;
        transfer_from_vault_to_user(
            ctx.accounts.market.to_account_info(),
            ctx.accounts.reserve_vault.to_account_info(),
            ctx.accounts.owner_asset_account.to_account_info(),
            ctx.accounts.asset_mint.to_account_info(),
            asset_token_program,
            args.claim_amount,
            ctx.accounts.asset_mint.decimals,
            &[&generate_market_v2_seeds!(ctx.accounts.market)[..]],
        )?;

        let (protected_claim_supply, required_buffer) = {
            let market_side = ctx.accounts.market.side_mut(args.market_side_index)?;
            market_side.apply_claim_redemption(args.claim_amount)?;
            (
                market_side.claim_ledger.protected_claim_supply,
                market_side.buffer_book.required_buffer,
            )
        };

        emit_cpi!(MarketClaimRedeemedV2 {
            market: market_key,
            owner: owner_key,
            asset_mint: asset_mint_key,
            claim_amount: args.claim_amount,
            protected_claim_supply,
            required_buffer,
            metadata: MarketEventMetadataV2::new(owner_key, market_key),
        });

        Ok(())
    }
}

fn validate_reserve_accounts<'info>(
    market: &Account<'info, MarketV2>,
    market_side_index: u8,
    owner: Pubkey,
    asset_mint: &InterfaceAccount<'info, Mint>,
    claim_mint: &InterfaceAccount<'info, Mint>,
    reserve_vault: &InterfaceAccount<'info, TokenAccount>,
    owner_asset_account: &InterfaceAccount<'info, TokenAccount>,
    owner_claim_account: &InterfaceAccount<'info, TokenAccount>,
) -> Result<()> {
    let market_side = market.side(market_side_index)?;
    require_keys_eq!(
        market_side.asset_mint,
        asset_mint.key(),
        ErrorCode::InvalidMint
    );
    require_keys_eq!(
        market_side.claim_mint,
        claim_mint.key(),
        ErrorCode::InvalidClaimMintV2
    );
    require_keys_eq!(
        market_side.reserve_vault,
        reserve_vault.key(),
        ErrorCode::InvalidVault
    );
    require_keys_eq!(
        reserve_vault.mint,
        asset_mint.key(),
        ErrorCode::InvalidVault
    );
    require_keys_eq!(reserve_vault.owner, market.key(), ErrorCode::InvalidVault);
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
    require_keys_eq!(
        owner_claim_account.mint,
        claim_mint.key(),
        ErrorCode::InvalidTokenAccount
    );
    require_keys_eq!(
        owner_claim_account.owner,
        owner,
        ErrorCode::InvalidTokenAccount
    );
    require!(
        claim_mint.mint_authority == COption::Some(market.key()),
        ErrorCode::InvalidClaimMintV2
    );
    Ok(())
}

fn token_program_for_mint<'info>(
    mint: &InterfaceAccount<'info, Mint>,
    token_program: &Program<'info, Token>,
    token_2022_program: &Program<'info, Token2022>,
) -> Result<AccountInfo<'info>> {
    let mint_info = mint.to_account_info();
    if *mint_info.owner == token_program.key() {
        Ok(token_program.to_account_info())
    } else if *mint_info.owner == token_2022_program.key() {
        Ok(token_2022_program.to_account_info())
    } else {
        err!(ErrorCode::InvalidTokenProgram)
    }
}
