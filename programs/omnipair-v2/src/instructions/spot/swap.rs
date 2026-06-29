use anchor_lang::solana_program::{
    instruction::{AccountMeta, Instruction},
    log::sol_log_data,
    program::invoke_signed,
};
use anchor_lang::{prelude::*, Discriminator};
use anchor_spl::{
    token::Token,
    token_interface::{Mint, Token2022, TokenAccount},
};

use crate::{
    constants::*,
    errors::ErrorCode,
    events::{HlpRebalanced, MarketEventMetadata, MarketHealthUpdated, SwapExecuted, SwapSettled},
    generate_market_seeds,
    math::calculate_raw_amount_out,
    shared::{
        math::ceil_div,
        token::{
            transfer_from_user_to_vault, transfer_from_vault_to_user, transfer_from_vault_to_vault,
        },
    },
    state::{FutarchyAuthority, HlpRebalanceReceipt, Market, MarketAsset},
};

use crate::instructions::common::{
    require_supported_asset_mint, token_account_credit, token_program_for_mint,
    validate_swap_accounts,
};

#[derive(AnchorSerialize, AnchorDeserialize, Clone)]
pub struct SwapArgs {
    pub exact_asset_in: u64,
    pub min_asset_out: u64,
}

#[event_cpi]
#[derive(Accounts)]
#[instruction(args: SwapArgs)]
pub struct Swap<'info> {
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

    #[account(
        seeds = [FUTARCHY_AUTHORITY_SEED_PREFIX],
        bump = futarchy_authority.bump
    )]
    pub futarchy_authority: Box<Account<'info, FutarchyAuthority>>,

    #[account(mut)]
    pub trader: Signer<'info>,

    pub asset_in_mint: Box<InterfaceAccount<'info, Mint>>,

    pub asset_out_mint: Box<InterfaceAccount<'info, Mint>>,

    #[account(mut)]
    pub reserve_in_vault: Box<InterfaceAccount<'info, TokenAccount>>,

    #[account(mut)]
    pub reserve_out_vault: Box<InterfaceAccount<'info, TokenAccount>>,

    #[account(mut)]
    pub fee_in_vault: Box<InterfaceAccount<'info, TokenAccount>>,

    #[account(mut)]
    pub trader_asset_in_account: Box<InterfaceAccount<'info, TokenAccount>>,

    #[account(mut)]
    pub trader_asset_out_account: Box<InterfaceAccount<'info, TokenAccount>>,

    pub token_program: Program<'info, Token>,
    pub token_2022_program: Program<'info, Token2022>,
}

impl<'info> Swap<'info> {
    pub fn validate(&self, args: &SwapArgs) -> Result<()> {
        self.market
            .assert_live_with_futarchy(&self.futarchy_authority)?;
        require!(args.exact_asset_in > 0, ErrorCode::AmountZero);
        require_gte!(
            self.trader_asset_in_account.amount,
            args.exact_asset_in,
            ErrorCode::InsufficientBalance
        );
        validate_swap_accounts(
            &self.market,
            self.trader.key(),
            &self.asset_in_mint,
            &self.asset_out_mint,
            &self.reserve_in_vault,
            &self.reserve_out_vault,
            &self.fee_in_vault,
            &self.trader_asset_in_account,
            &self.trader_asset_out_account,
        )?;
        require_supported_asset_mint(&self.asset_in_mint)?;
        require_supported_asset_mint(&self.asset_out_mint)?;
        Ok(())
    }

    pub fn update(&mut self) -> Result<()> {
        self.market.update()
    }

    pub fn update_and_validate(&mut self, args: &SwapArgs) -> Result<()> {
        self.update()?;
        self.validate(args)
    }

    pub fn handle_swap(mut ctx: Context<'_, '_, '_, 'info, Self>, args: SwapArgs) -> Result<()> {
        let market_key = ctx.accounts.market.key();
        let trader_key = ctx.accounts.trader.key();
        let asset_in_mint_key = ctx.accounts.asset_in_mint.key();
        let asset_out_mint_key = ctx.accounts.asset_out_mint.key();
        let asset_in = ctx.accounts.market.asset_for_mint(asset_in_mint_key)?;
        let manager_fee_bps = ctx.accounts.market.config.manager_fee_bps;
        let protocol_fee_bps = ctx.accounts.futarchy_authority.revenue_share.swap_bps;
        let protocol_auction_split = ctx.accounts.futarchy_authority.protocol_auction_split;

        validate_hlp_rebalance_accounts(&ctx.accounts.market, ctx.remaining_accounts)?;
        ctx.accounts.market.refresh_risk()?;
        ctx.accounts.market.assert_risk_circuit_breakers()?;

        let reserve_credit = receive_swap_inventory(&mut ctx, args.exact_asset_in)?;
        let total_fee = ceil_div(
            (reserve_credit as u128)
                .checked_mul(ctx.accounts.market.config.swap_fee_bps as u128)
                .ok_or(ErrorCode::FeeMathOverflow)?,
            BPS_DENOMINATOR as u128,
        )
        .ok_or(ErrorCode::FeeMathOverflow)?
        .min(reserve_credit as u128) as u64;

        let fee_credit = move_swap_fee(&mut ctx, total_fee)?;
        let amount_in_after_fee = reserve_credit
            .checked_sub(total_fee)
            .ok_or(ErrorCode::MarketMathOverflow)?;
        require!(amount_in_after_fee > 0, ErrorCode::InsufficientOutputAmount);

        let amount_out = {
            let (market_side_in, market_side_out) = ctx.accounts.market.swap_sides(asset_in);
            calculate_raw_amount_out(
                market_side_in.reserves.live_reserve,
                market_side_out.reserves.live_reserve,
                amount_in_after_fee,
            )?
        };

        let trader_asset_out_balance_before = ctx.accounts.trader_asset_out_account.amount;
        let asset_out_token_program = token_program_for_mint(
            &ctx.accounts.asset_out_mint,
            &ctx.accounts.token_program,
            &ctx.accounts.token_2022_program,
        )?;
        transfer_from_vault_to_user(
            ctx.accounts.market.to_account_info(),
            ctx.accounts.reserve_out_vault.to_account_info(),
            ctx.accounts.trader_asset_out_account.to_account_info(),
            ctx.accounts.asset_out_mint.to_account_info(),
            asset_out_token_program,
            amount_out,
            ctx.accounts.asset_out_mint.decimals,
            &[&generate_market_seeds!(ctx.accounts.market)[..]],
        )?;
        ctx.accounts.trader_asset_out_account.reload()?;
        let asset_out_credit = token_account_credit(
            trader_asset_out_balance_before,
            &ctx.accounts.trader_asset_out_account,
        )?;
        require_gte!(
            asset_out_credit,
            args.min_asset_out,
            ErrorCode::SlippageExceeded
        );

        let swap_receipt = ctx.accounts.market.swap_reserves(
            asset_in,
            amount_in_after_fee,
            amount_out,
            fee_credit,
            manager_fee_bps,
            protocol_fee_bps,
            protocol_auction_split,
        )?;
        let current_slot = Clock::get()?.slot;
        let (base_hlp_rebalance, quote_hlp_rebalance) =
            ctx.accounts.market.rebalance_hlp_vaults(current_slot)?;
        let h_lp_tokens_changed = rebalance_executes_token_changes(&base_hlp_rebalance)
            || rebalance_executes_token_changes(&quote_hlp_rebalance);
        if h_lp_tokens_changed {
            refresh_risk_snapshot(&mut ctx.accounts.market)?;
        } else {
            ctx.accounts.market.refresh_risk()?;
        }

        if h_lp_tokens_changed {
            apply_hlp_rebalance_token_changes(&ctx, &base_hlp_rebalance, &quote_hlp_rebalance)?;
            emit_swap_settled_low_heap(
                market_key,
                trader_key,
                asset_in.code(),
                reserve_credit,
                swap_receipt.amount_in_after_fee,
                swap_receipt.amount_out,
                swap_receipt.fee_credit,
                ctx.accounts.market.base_hlp_vault.pending_rebalance,
                ctx.accounts.market.quote_hlp_vault.pending_rebalance,
            );
        } else {
            emit!(SwapExecuted {
                market: market_key,
                trader: trader_key,
                asset_in_mint: asset_in_mint_key,
                asset_out_mint: asset_out_mint_key,
                reserve_credit,
                amount_in_after_fee: swap_receipt.amount_in_after_fee,
                amount_out: swap_receipt.amount_out,
                fee_credit: swap_receipt.fee_credit,
                base_hlp_pending_rebalance: ctx.accounts.market.base_hlp_vault.pending_rebalance,
                quote_hlp_pending_rebalance: ctx.accounts.market.quote_hlp_vault.pending_rebalance,
                metadata: MarketEventMetadata::new(trader_key, market_key)?,
            });
            if should_emit_hlp_rebalance(
                base_hlp_rebalance.ideal_delta,
                ctx.accounts.market.base_hlp_vault.pending_rebalance,
                ctx.accounts.market.base_hlp_vault.hlp_supply,
            ) {
                emit!(HlpRebalanced {
                    market: market_key,
                    target_side: MarketAsset::Base.code(),
                    ideal_delta: base_hlp_rebalance.ideal_delta,
                    executed_delta: base_hlp_rebalance.executed_delta,
                    pending_rebalance: ctx.accounts.market.base_hlp_vault.pending_rebalance,
                    nav_nad: ctx.accounts.market.base_hlp_vault.last_nav_nad,
                    metadata: MarketEventMetadata::new(trader_key, market_key)?,
                });
            }
            if should_emit_hlp_rebalance(
                quote_hlp_rebalance.ideal_delta,
                ctx.accounts.market.quote_hlp_vault.pending_rebalance,
                ctx.accounts.market.quote_hlp_vault.hlp_supply,
            ) {
                emit!(HlpRebalanced {
                    market: market_key,
                    target_side: MarketAsset::Quote.code(),
                    ideal_delta: quote_hlp_rebalance.ideal_delta,
                    executed_delta: quote_hlp_rebalance.executed_delta,
                    pending_rebalance: ctx.accounts.market.quote_hlp_vault.pending_rebalance,
                    nav_nad: ctx.accounts.market.quote_hlp_vault.last_nav_nad,
                    metadata: MarketEventMetadata::new(trader_key, market_key)?,
                });
            }
            emit!(MarketHealthUpdated {
                market: market_key,
                recognized_base_collateral_for_quote_debt: ctx
                    .accounts
                    .market
                    .health
                    .recognized_base_collateral_for_quote_debt,
                recognized_quote_collateral_for_base_debt: ctx
                    .accounts
                    .market
                    .health
                    .recognized_quote_collateral_for_base_debt,
                effective_base_debt_nad: ctx.accounts.market.health.effective_base_debt_nad,
                effective_quote_debt_nad: ctx.accounts.market.health.effective_quote_debt_nad,
                base_debt_health_bps: ctx.accounts.market.health.base_debt_health_bps,
                quote_debt_health_bps: ctx.accounts.market.health.quote_debt_health_bps,
                metadata: MarketEventMetadata::new(trader_key, market_key)?,
            });
        }

        Ok(())
    }
}

fn should_emit_hlp_rebalance(ideal_delta: i128, pending_rebalance: i128, hlp_supply: u64) -> bool {
    hlp_supply > 0 || ideal_delta != 0 || pending_rebalance != 0
}

fn rebalance_executes_token_changes(receipt: &HlpRebalanceReceipt) -> bool {
    receipt.ylp_mint_amount > 0 || receipt.ylp_burn_amount > 0
}

fn refresh_risk_snapshot(market: &mut Market) -> Result<()> {
    let risk = market.current_risk()?;
    market.last_update_slot = risk.last_snapshot_slot;
    market.risk = risk;
    Ok(())
}

fn emit_swap_settled_low_heap(
    market: Pubkey,
    trader: Pubkey,
    asset_in_side: u8,
    reserve_credit: u64,
    amount_in_after_fee: u64,
    amount_out: u64,
    fee_credit: u64,
    base_hlp_pending_rebalance: i128,
    quote_hlp_pending_rebalance: i128,
) {
    const SWAP_SETTLED_EVENT_LEN: usize = 8 + 32 + 32 + 1 + 8 + 8 + 8 + 8 + 16 + 16;

    let mut data = [0u8; SWAP_SETTLED_EVENT_LEN];
    let mut offset = 0usize;
    data[offset..offset + 8].copy_from_slice(SwapSettled::DISCRIMINATOR);
    offset += 8;
    data[offset..offset + 32].copy_from_slice(market.as_ref());
    offset += 32;
    data[offset..offset + 32].copy_from_slice(trader.as_ref());
    offset += 32;
    data[offset] = asset_in_side;
    offset += 1;
    data[offset..offset + 8].copy_from_slice(&reserve_credit.to_le_bytes());
    offset += 8;
    data[offset..offset + 8].copy_from_slice(&amount_in_after_fee.to_le_bytes());
    offset += 8;
    data[offset..offset + 8].copy_from_slice(&amount_out.to_le_bytes());
    offset += 8;
    data[offset..offset + 8].copy_from_slice(&fee_credit.to_le_bytes());
    offset += 8;
    data[offset..offset + 16].copy_from_slice(&base_hlp_pending_rebalance.to_le_bytes());
    offset += 16;
    data[offset..offset + 16].copy_from_slice(&quote_hlp_pending_rebalance.to_le_bytes());

    sol_log_data(&[&data]);
}

fn validate_hlp_rebalance_accounts(
    market: &Market,
    remaining_accounts: &[AccountInfo],
) -> Result<()> {
    let mut cursor = 0usize;
    if market.base_hlp_vault.hlp_supply > 0 {
        require_gte!(
            remaining_accounts.len(),
            cursor + 2,
            ErrorCode::NotEnoughAccounts
        );
        require_hlp_mint_account(&remaining_accounts[cursor], market.ylp_mint)?;
        require_hlp_vault_account(
            &remaining_accounts[cursor + 1],
            market.base_hlp_vault.ylp_vault,
        )?;
        cursor += 2;
    }
    if market.quote_hlp_vault.hlp_supply > 0 {
        require_gte!(
            remaining_accounts.len(),
            cursor + 2,
            ErrorCode::NotEnoughAccounts
        );
        require_hlp_mint_account(&remaining_accounts[cursor], market.ylp_mint)?;
        require_hlp_vault_account(
            &remaining_accounts[cursor + 1],
            market.quote_hlp_vault.ylp_vault,
        )?;
    }
    Ok(())
}

fn require_hlp_mint_account(account: &AccountInfo, expected_key: Pubkey) -> Result<()> {
    require_keys_eq!(account.key(), expected_key, ErrorCode::InvalidMint);
    require!(account.is_writable, ErrorCode::InvalidMint);
    Ok(())
}

fn require_hlp_vault_account(account: &AccountInfo, expected_key: Pubkey) -> Result<()> {
    require_keys_eq!(account.key(), expected_key, ErrorCode::InvalidVault);
    require!(account.is_writable, ErrorCode::InvalidVault);
    Ok(())
}

fn apply_hlp_rebalance_token_changes<'info>(
    ctx: &anchor_lang::context::Context<'_, '_, '_, 'info, Swap<'info>>,
    base_receipt: &HlpRebalanceReceipt,
    quote_receipt: &HlpRebalanceReceipt,
) -> Result<()> {
    let mut cursor = 0usize;
    let mut scratch = Token2022InstructionScratch::new(ctx.accounts.token_2022_program.key());
    if ctx.accounts.market.base_hlp_vault.hlp_supply > 0 {
        apply_single_hlp_rebalance_token_changes(ctx, base_receipt, cursor, &mut scratch)?;
        cursor += 2;
    }
    if ctx.accounts.market.quote_hlp_vault.hlp_supply > 0 {
        apply_single_hlp_rebalance_token_changes(ctx, quote_receipt, cursor, &mut scratch)?;
    }
    Ok(())
}

struct Token2022InstructionScratch {
    instruction: Instruction,
}

impl Token2022InstructionScratch {
    fn new(program_id: Pubkey) -> Self {
        Self {
            instruction: Instruction {
                program_id,
                accounts: Vec::with_capacity(3),
                data: Vec::with_capacity(9),
            },
        }
    }

    fn mint_to(&mut self, mint: Pubkey, destination: Pubkey, authority: Pubkey, amount: u64) {
        self.instruction.accounts.clear();
        self.instruction
            .accounts
            .push(AccountMeta::new(mint, false));
        self.instruction
            .accounts
            .push(AccountMeta::new(destination, false));
        self.instruction
            .accounts
            .push(AccountMeta::new_readonly(authority, true));

        self.instruction.data.clear();
        self.instruction.data.push(7);
        self.instruction
            .data
            .extend_from_slice(&amount.to_le_bytes());
    }

    fn burn(&mut self, source: Pubkey, mint: Pubkey, authority: Pubkey, amount: u64) {
        self.instruction.accounts.clear();
        self.instruction
            .accounts
            .push(AccountMeta::new(source, false));
        self.instruction
            .accounts
            .push(AccountMeta::new(mint, false));
        self.instruction
            .accounts
            .push(AccountMeta::new_readonly(authority, true));

        self.instruction.data.clear();
        self.instruction.data.push(8);
        self.instruction
            .data
            .extend_from_slice(&amount.to_le_bytes());
    }
}

fn apply_single_hlp_rebalance_token_changes<'info>(
    ctx: &anchor_lang::context::Context<'_, '_, '_, 'info, Swap<'info>>,
    receipt: &HlpRebalanceReceipt,
    cursor: usize,
    scratch: &mut Token2022InstructionScratch,
) -> Result<()> {
    let ylp_mint = &ctx.remaining_accounts[cursor];
    let ylp_vault = &ctx.remaining_accounts[cursor + 1];
    let market_seeds = generate_market_seeds!(ctx.accounts.market);
    let signer_seeds = [&market_seeds[..]];
    let market = ctx.accounts.market.to_account_info();
    let token_2022_program = ctx.accounts.token_2022_program.to_account_info();

    if receipt.ylp_mint_amount > 0 {
        token_2022_mint_to_with_scratch(
            scratch,
            market.clone(),
            token_2022_program.clone(),
            ylp_mint.clone(),
            ylp_vault.clone(),
            receipt.ylp_mint_amount,
            &signer_seeds,
        )?;
    }
    if receipt.ylp_burn_amount > 0 {
        token_2022_burn_with_scratch(
            scratch,
            market,
            token_2022_program,
            ylp_mint.clone(),
            ylp_vault.clone(),
            receipt.ylp_burn_amount,
            &signer_seeds,
        )?;
    }
    Ok(())
}

fn token_2022_mint_to_with_scratch<'info>(
    scratch: &mut Token2022InstructionScratch,
    authority: AccountInfo<'info>,
    token_program: AccountInfo<'info>,
    mint: AccountInfo<'info>,
    destination: AccountInfo<'info>,
    amount: u64,
    signer_seeds: &[&[&[u8]]],
) -> Result<()> {
    scratch.mint_to(*mint.key, *destination.key, *authority.key, amount);
    invoke_signed(
        &scratch.instruction,
        &[mint, destination, authority, token_program],
        signer_seeds,
    )
    .map_err(Into::into)
}

fn token_2022_burn_with_scratch<'info>(
    scratch: &mut Token2022InstructionScratch,
    authority: AccountInfo<'info>,
    token_program: AccountInfo<'info>,
    mint: AccountInfo<'info>,
    source: AccountInfo<'info>,
    amount: u64,
    signer_seeds: &[&[&[u8]]],
) -> Result<()> {
    scratch.burn(*source.key, *mint.key, *authority.key, amount);
    invoke_signed(
        &scratch.instruction,
        &[source, mint, authority, token_program],
        signer_seeds,
    )
    .map_err(Into::into)
}

fn receive_swap_inventory<'info>(
    ctx: &mut Context<'_, '_, '_, 'info, Swap<'info>>,
    exact_asset_in: u64,
) -> Result<u64> {
    let reserve_balance_before = ctx.accounts.reserve_in_vault.amount;
    let asset_in_token_program = token_program_for_mint(
        &ctx.accounts.asset_in_mint,
        &ctx.accounts.token_program,
        &ctx.accounts.token_2022_program,
    )?;
    transfer_from_user_to_vault(
        ctx.accounts.trader.to_account_info(),
        ctx.accounts.trader_asset_in_account.to_account_info(),
        ctx.accounts.reserve_in_vault.to_account_info(),
        ctx.accounts.asset_in_mint.to_account_info(),
        asset_in_token_program,
        exact_asset_in,
        ctx.accounts.asset_in_mint.decimals,
    )?;
    ctx.accounts.reserve_in_vault.reload()?;
    ctx.accounts
        .reserve_in_vault
        .amount
        .checked_sub(reserve_balance_before)
        .ok_or(ErrorCode::MarketMathOverflow.into())
}

fn move_swap_fee<'info>(
    ctx: &mut Context<'_, '_, '_, 'info, Swap<'info>>,
    total_fee: u64,
) -> Result<u64> {
    if total_fee == 0 {
        return Ok(0);
    }
    let fee_balance_before = ctx.accounts.fee_in_vault.amount;
    let asset_in_token_program = token_program_for_mint(
        &ctx.accounts.asset_in_mint,
        &ctx.accounts.token_program,
        &ctx.accounts.token_2022_program,
    )?;
    transfer_from_vault_to_vault(
        ctx.accounts.market.to_account_info(),
        ctx.accounts.reserve_in_vault.to_account_info(),
        ctx.accounts.fee_in_vault.to_account_info(),
        ctx.accounts.asset_in_mint.to_account_info(),
        asset_in_token_program,
        total_fee,
        ctx.accounts.asset_in_mint.decimals,
        &[&generate_market_seeds!(ctx.accounts.market)[..]],
    )?;
    ctx.accounts.reserve_in_vault.reload()?;
    ctx.accounts.fee_in_vault.reload()?;
    ctx.accounts
        .fee_in_vault
        .amount
        .checked_sub(fee_balance_before)
        .ok_or(ErrorCode::MarketMathOverflow.into())
}
