use anchor_lang::{
    prelude::*, 
    solana_program::{
        program::invoke, 
        system_instruction,
        hash::hash,
    }
};
use anchor_spl::{
    token::spl_token,
    token::{Token, TokenAccount, Mint},
    token_interface::{Token2022},
    associated_token::{AssociatedToken, create_idempotent},
};
use anchor_spl::metadata::{
    create_metadata_accounts_v3,
    mpl_token_metadata::types::DataV2,
    mpl_token_metadata::ID as MPL_TOKEN_METADATA_PROGRAM_ID,
    CreateMetadataAccountsV3, Metadata,
};
use anchor_lang::solana_program::program_pack::Pack;
use crate::state::{
    pair::{Pair, VaultBumps, LastPriceEMA},
    rate_model::RateModel,
    futarchy_authority::FutarchyAuthority,
};
use crate::errors::ErrorCode;
use crate::constants::*;
use crate::utils::account::get_size_with_discriminator;
use crate::utils::token::{
    transfer_from_user_to_vault,
    token_mint_to,  
};
use crate::utils::math::SqrtU128;
use crate::events::{PairCreatedEvent, MintEvent, UserLiquidityPositionUpdatedEvent, EventMetadata};
use crate::generate_gamm_pair_seeds;

#[derive(AnchorSerialize, AnchorDeserialize)]
pub struct InitializeAndBootstrapArgs {
    pub swap_fee_bps: u16,
    pub half_life: u64,
    pub fixed_cf_bps: Option<u16>,
    pub target_util_start_bps: Option<u64>, // utilization lower bound (defaults to 50%)
    pub target_util_end_bps: Option<u64>,   // utilization upper bound (defaults to 85%)
    pub params_hash: [u8; 32],
    pub version: u8,

    pub amount0_in: u64,
    pub amount1_in: u64,
    pub min_liquidity_out: u64,

    pub lp_name: String,   // e.g. "OMFG/USDC omLP" (<= 32)
    pub lp_symbol: String, // e.g. "OM.US-OMLP" (<= 10)
    pub lp_uri: String,    // e.g. "ipfs://assets.../OMFG-USDC.json" (<= 200 chars)
}

#[event_cpi]
#[derive(Accounts)]
#[instruction(args: InitializeAndBootstrapArgs)]
pub struct InitializeAndBootstrap<'info> {
    #[account(mut)]
    pub deployer: Signer<'info>,

    pub token0_mint: Box<Account<'info, Mint>>,
    pub token1_mint: Box<Account<'info, Mint>>,
    
    #[account(
        init,
        payer = deployer,
        space = get_size_with_discriminator::<Pair>(),
        seeds = [
            PAIR_SEED_PREFIX, 
            token0_mint.key().as_ref(), 
            token1_mint.key().as_ref(),
            args.params_hash.as_ref(),
        ],
        bump
    )]
    pub pair: Box<Account<'info, Pair>>,

    #[account(
        seeds = [FUTARCHY_AUTHORITY_SEED_PREFIX],
        bump = futarchy_authority.bump
    )]
    pub futarchy_authority: Account<'info, FutarchyAuthority>,

    #[account(
        init,
        payer = deployer,
        space = get_size_with_discriminator::<RateModel>(),
    )]
    pub rate_model: Box<Account<'info, RateModel>>,

    #[account(
        mut,
        constraint = lp_mint.owner == token_program.key @ ErrorCode::InvalidTokenProgram,
    )]
    /// CHECK: initialized in-program via initialize_mint2; validated at runtime
    pub lp_mint: UncheckedAccount<'info>,

    #[account(
        mut,
        seeds = [METADATA_SEED_PREFIX, MPL_TOKEN_METADATA_PROGRAM_ID.as_ref(), lp_mint.key().as_ref()],
        seeds::program = MPL_TOKEN_METADATA_PROGRAM_ID,
        bump
    )]
    /// CHECK: derived/checked via seeds above
    pub lp_token_metadata: UncheckedAccount<'info>,

    #[account(mut)]
    /// CHECK: created via CPI after lp_mint is initialized
    pub deployer_lp_token_account: UncheckedAccount<'info>,

    #[account(
        init,
        seeds = [
            RESERVE_VAULT_SEED_PREFIX,
            pair.key().as_ref(),
            token0_mint.key().as_ref(),
        ],
        payer = deployer,
        token::mint = token0_mint,
        token::authority = pair,
        bump
    )]
    pub reserve0_vault: Box<Account<'info, TokenAccount>>,
    
    #[account(
        init,
        seeds = [
            RESERVE_VAULT_SEED_PREFIX,
            pair.key().as_ref(),
            token1_mint.key().as_ref(),
        ],
        payer = deployer,
        token::mint = token1_mint,
        token::authority = pair,
        bump
    )]
    pub reserve1_vault: Box<Account<'info, TokenAccount>>,

    #[account(
        init,
        seeds = [
            COLLATERAL_VAULT_SEED_PREFIX,
            pair.key().as_ref(),
            token0_mint.key().as_ref(),
        ],
        payer = deployer,
        token::mint = token0_mint,
        token::authority = pair,
        bump
    )]
    pub collateral0_vault: Box<Account<'info, TokenAccount>>,
    
    #[account(
        init,
        seeds = [
            COLLATERAL_VAULT_SEED_PREFIX,
            pair.key().as_ref(),
            token1_mint.key().as_ref(),
        ],
        payer = deployer,
        token::mint = token1_mint,
        token::authority = pair,
        bump
    )]
    pub collateral1_vault: Box<Account<'info, TokenAccount>>,
    
    #[account(
        mut,
        token::mint = token0_mint,
        token::authority = deployer,
    )]
    pub deployer_token0_account: Box<Account<'info, TokenAccount>>,
    
    #[account(
        mut,
        token::mint = token1_mint,
        token::authority = deployer,
    )]
    pub deployer_token1_account: Box<Account<'info, TokenAccount>>,

    #[account(
        mut,
        constraint = authority_wsol_account.mint == spl_token::native_mint::id(),
        constraint = authority_wsol_account.owner == futarchy_authority.key() @ ErrorCode::InvalidFutarchyAuthority,
        constraint = *authority_wsol_account.to_account_info().owner == token_program.key() @ ErrorCode::InvalidTokenProgram,
      )]
      pub authority_wsol_account: Box<Account<'info, TokenAccount>>,

    pub system_program: Program<'info, System>,
    pub token_program: Program<'info, Token>,
    pub token_2022_program: Program<'info, Token2022>,
    pub token_metadata_program: Program<'info, Metadata>,
    pub associated_token_program: Program<'info, AssociatedToken>,
    pub rent: Sysvar<'info, Rent>,
}

impl<'info> InitializeAndBootstrap<'info> {
    pub fn validate(&self, args: &InitializeAndBootstrapArgs) -> Result<()> {
        let InitializeAndBootstrapArgs { 
            version,
            swap_fee_bps, 
            half_life,
            fixed_cf_bps,
            target_util_start_bps,
            target_util_end_bps,
            params_hash,
            amount0_in,
            amount1_in,
            lp_name,
            lp_symbol,
            lp_uri,
            ..
        } = args;

        // tokens canonical order check (token0 > token1)
        // this prevents the same token pair from having two valid addresses (0,1) and (1,0)
        require_gt!(self.token1_mint.key(), self.token0_mint.key(), ErrorCode::InvalidTokenOrder);

        // validate pool parameters
        require_eq!(*version, VERSION, ErrorCode::InvalidVersion);
        require_gte!(BPS_DENOMINATOR, *swap_fee_bps, ErrorCode::InvalidSwapFeeBps); // 0 <= swap_fee_bps <= 100%
        require_gte!(*half_life, MIN_HALF_LIFE_MS, ErrorCode::InvalidHalfLife); // half_life >= 1 minute
        require_gte!(MAX_HALF_LIFE_MS, *half_life, ErrorCode::InvalidHalfLife); // half_life <= 12 hours

        // validate fixed_cf_bps if provided
        if let Some(cf_bps) = fixed_cf_bps {
            require_gte!(BPS_DENOMINATOR, *cf_bps, ErrorCode::InvalidArgument); // 0 <= fixed_cf_bps <= 100%
            require_gte!(*cf_bps, 100, ErrorCode::InvalidArgument); // fixed_cf_bps >= 1% (100 bps) minimum
        }

        // validate utilization bounds if provided (both must be provided together, or neither)
        let util_start = target_util_start_bps.unwrap_or(TARGET_UTIL_START_BPS);
        let util_end = target_util_end_bps.unwrap_or(TARGET_UTIL_END_BPS);
        require!(RateModel::validate_util_bounds(util_start, util_end), ErrorCode::InvalidUtilBounds);

        // Verify params_hash matches the computed hash
        // SHA256(VERSION || swap_fee_bps || half_life || fixed_cf_bps || target_util_start_bps || target_util_end_bps)
        let mut hash_data = Vec::new();
        hash_data.extend_from_slice(&VERSION.to_le_bytes());
        hash_data.extend_from_slice(&swap_fee_bps.to_le_bytes());
        hash_data.extend_from_slice(&half_life.to_le_bytes());
        hash_data.extend_from_slice(&fixed_cf_bps.unwrap_or(0).to_le_bytes());
        hash_data.extend_from_slice(&target_util_start_bps.unwrap_or(0).to_le_bytes());
        hash_data.extend_from_slice(&target_util_end_bps.unwrap_or(0).to_le_bytes());
        let computed_hash = hash(&hash_data).to_bytes();
        let hashes_match = computed_hash.iter().zip(params_hash.iter()).all(|(a, b)| a == b);
        require!(hashes_match, ErrorCode::InvalidParamsHash);

        // validate bootstrap parameters
        require!(*amount0_in > 0 && *amount1_in > 0, ErrorCode::AmountZero);
        require_gte!(self.deployer_token0_account.amount, *amount0_in, ErrorCode::InsufficientAmount0In);
        require_gte!(self.deployer_token1_account.amount, *amount1_in, ErrorCode::InsufficientAmount1In);

        #[cfg(feature = "production")]
        {
            let lp_mint_key: String = self.lp_mint.key().to_string();
            let start_idx = lp_mint_key.len().checked_sub(4).ok_or(ErrorCode::InvalidLpMintKey)?;
            let last_4_chars = &lp_mint_key[start_idx..];
            require_eq!("omLP", last_4_chars, ErrorCode::InvalidLpMintKey);
        }
    
        require!(lp_name.len() <= 32, ErrorCode::InvalidLpName);
        require!(lp_name.is_ascii(), ErrorCode::InvalidLpName);
        require!(lp_symbol.len() <= 10, ErrorCode::InvalidLpSymbol);
        require!(lp_symbol.is_ascii(), ErrorCode::InvalidLpSymbol);
        require!(lp_uri.len() <= 200, ErrorCode::InvalidLpUri);
        require!(lp_uri.starts_with("http"), ErrorCode::InvalidLpUri);
        
        Ok(())
    }

    pub fn handle_initialize(ctx: Context<Self>, args: InitializeAndBootstrapArgs) -> Result<()> {
        let current_slot = Clock::get()?.slot;
        let pair_key = ctx.accounts.pair.key();
        let pair = &mut ctx.accounts.pair;
        
        let InitializeAndBootstrapArgs { 
            swap_fee_bps, 
            half_life, 
            fixed_cf_bps,
            target_util_start_bps,
            target_util_end_bps,
            params_hash,
            version,
            amount0_in,
            amount1_in,
            min_liquidity_out,
            lp_name,
            lp_symbol,
            lp_uri,
        } = args;

        // Collect pair creation fee from deployer to futarchy authority
        invoke(
            &system_instruction::transfer(
                ctx.accounts.deployer.key,
                &ctx.accounts.authority_wsol_account.key(),
                PAIR_CREATION_FEE_LAMPORTS,
            ),
            &[
                ctx.accounts.deployer.to_account_info(),
                ctx.accounts.authority_wsol_account.to_account_info(),
                ctx.accounts.system_program.to_account_info(),
            ],
        )?;

        invoke(
            &spl_token::instruction::sync_native(
                ctx.accounts.token_program.key,
                &ctx.accounts.authority_wsol_account.key(),
            )?,
            &[
                ctx.accounts.token_program.to_account_info(),
                ctx.accounts.authority_wsol_account.to_account_info(),
            ],
        )?;
        
        let (
            token0, 
            token1, 
            token0_decimals,
            token1_decimals,
            rate_model_key,
            lp_mint_key
        ) = (
            ctx.accounts.token0_mint.key(), 
            ctx.accounts.token1_mint.key(), 
            ctx.accounts.token0_mint.decimals,
            ctx.accounts.token1_mint.decimals,
            ctx.accounts.rate_model.key(),
            ctx.accounts.lp_mint.key(),
        );

        // Initialize rate model with optional custom utilization bounds
        let util_start = target_util_start_bps.unwrap_or(TARGET_UTIL_START_BPS);
        let util_end = target_util_end_bps.unwrap_or(TARGET_UTIL_END_BPS);
        ctx.accounts.rate_model.set_inner(RateModel::new(util_start, util_end));

        // Initialize pair (before LP mint is initialized, but we store the key)
        let vault_bumps = VaultBumps {
            reserve0: ctx.bumps.reserve0_vault,
            reserve1: ctx.bumps.reserve1_vault,
            collateral0: ctx.bumps.collateral0_vault,
            collateral1: ctx.bumps.collateral1_vault,
        };

        pair.set_inner(Pair::initialize(
            token0,
            token1,
            lp_mint_key,
            token0_decimals,
            token1_decimals,
            rate_model_key,
            swap_fee_bps,
            half_life,
            fixed_cf_bps,
            current_slot,
            params_hash,
            version,
            ctx.bumps.pair,
            vault_bumps,
        ));

        // Transfer tokens from deployer to vaults
        transfer_from_user_to_vault(
            ctx.accounts.deployer.to_account_info(),
            ctx.accounts.deployer_token0_account.to_account_info(),
            ctx.accounts.reserve0_vault.to_account_info(),
            ctx.accounts.token0_mint.to_account_info(),
            match ctx.accounts.token0_mint.to_account_info().owner == ctx.accounts.token_program.key {
                true => ctx.accounts.token_program.to_account_info(),
                false => ctx.accounts.token_2022_program.to_account_info(),
            },
            amount0_in,
            ctx.accounts.token0_mint.decimals,
        )?;

        transfer_from_user_to_vault(
            ctx.accounts.deployer.to_account_info(),
            ctx.accounts.deployer_token1_account.to_account_info(),
            ctx.accounts.reserve1_vault.to_account_info(),
            ctx.accounts.token1_mint.to_account_info(),
            match ctx.accounts.token1_mint.to_account_info().owner == ctx.accounts.token_program.key {
                true => ctx.accounts.token_program.to_account_info(),
                false => ctx.accounts.token_2022_program.to_account_info(),
            },
            amount1_in,
            ctx.accounts.token1_mint.decimals,
        )?;
        
        // Initialize LP mint
        require!(
            ctx.accounts.lp_mint.data_len() == 82,
            ErrorCode::InvalidMintLen
        );
        let mint_unchecked = spl_token::state::Mint::unpack_unchecked(
            &ctx.accounts.lp_mint.to_account_info().data.borrow()
        )?;
        require!(!mint_unchecked.is_initialized, ErrorCode::AccountNotEmpty);
        
        let ix = spl_token::instruction::initialize_mint2(
            &ctx.accounts.token_program.key(),
            &ctx.accounts.lp_mint.key(),
            &pair_key,
            None,
            9,
        )?;
        invoke(
            &ix,
            &[
                ctx.accounts.lp_mint.to_account_info(),
            ],
        )?;

        // lp mint post-initialize checks
        let lp_mint_account = ctx.accounts.lp_mint.to_account_info();
        let mint = spl_token::state::Mint::unpack(&lp_mint_account.data.borrow())?;
        require_keys_eq!(mint.mint_authority.unwrap(), pair_key, ErrorCode::InvalidMintAuthority);
        require!(mint.freeze_authority.is_none(), ErrorCode::FrozenLpMint);
        require!(mint.supply == 0, ErrorCode::NonZeroSupply);
        require_eq!(mint.decimals, 9, ErrorCode::WrongLpDecimals);

        // Create associated token account for deployer LP token
        create_idempotent(
            CpiContext::new(
                ctx.accounts.associated_token_program.to_account_info(),
                anchor_spl::associated_token::Create {
                    payer: ctx.accounts.deployer.to_account_info(),
                    associated_token: ctx.accounts.deployer_lp_token_account.to_account_info(),
                    authority: ctx.accounts.deployer.to_account_info(),
                    mint: ctx.accounts.lp_mint.to_account_info(),
                    system_program: ctx.accounts.system_program.to_account_info(),
                    token_program: ctx.accounts.token_program.to_account_info(),
                },
            ),
        )?;
        
        // --- Create Metaplex metadata for LP mint ---
        let data = DataV2 {
            name:   lp_name.clone(),
            symbol: lp_symbol.clone(),
            uri:    lp_uri.clone(),
            seller_fee_basis_points: 0,
            creators: None,
            collection: None,
            uses: None,
        };
        
        let cpi_accounts = CreateMetadataAccountsV3 {
            metadata:         ctx.accounts.lp_token_metadata.to_account_info(),
            mint:             ctx.accounts.lp_mint.to_account_info(),
            mint_authority:   pair.to_account_info(),   // pair PDA signs
            payer:            ctx.accounts.deployer.to_account_info(),
            update_authority: pair.to_account_info(),   // keep program-controlled
            system_program:   ctx.accounts.system_program.to_account_info(),
            rent:             ctx.accounts.rent.to_account_info(),
        };
        
        create_metadata_accounts_v3(
            CpiContext::new(ctx.accounts.token_metadata_program.to_account_info(), cpi_accounts)
                .with_signer(&[&generate_gamm_pair_seeds!(pair)[..]]),
            data,
            true,  // is_mutable
            true,  // update_authority_is_signer (pair PDA)
            None,  // token_standard
        )?;

        // Calculate liquidity
        // sqrt(amount0_in * amount1_in) - MINIMUM_LIQUIDITY
        // MINIMUM_LIQUIDITY = 1000
        // 9 decimals: 1000 / 10^9 = 1e-6 full LP tokens
        // 1000 units are burned permanently.
        // This burn (~1e-6 of supply) is larger than Uniswap V2's 1e-15 burn (with 18 decimals),
        // but still negligible for users and significantly raises the cost of share inflation attacks.
        let liquidity: u64 = (amount0_in as u128)
            .checked_mul(amount1_in as u128)
            .ok_or(ErrorCode::LiquidityMathOverflow)?
            .sqrt()
            .ok_or(ErrorCode::LiquiditySqrtOverflow)?
            .checked_sub(MIN_LIQUIDITY as u128)
            .ok_or(ErrorCode::LiquidityUnderflow)?
            .try_into()
            .map_err(|_| ErrorCode::LiquidityConversionOverflow)?;

        require!(
            liquidity >= min_liquidity_out,
            ErrorCode::SlippageExceeded
        );
        
        // Mint LP tokens to deployer
        token_mint_to(
            pair.to_account_info(),
            ctx.accounts.token_program.to_account_info(),
            ctx.accounts.lp_mint.to_account_info(),
            ctx.accounts.deployer_lp_token_account.to_account_info(),
            liquidity,
            &[&generate_gamm_pair_seeds!(pair)[..]]
        )?;
        
        // Update reserves
        pair.reserve0 = pair.reserve0
            .checked_add(amount0_in)
            .ok_or(ErrorCode::ReserveOverflow)?;
        pair.reserve1 = pair.reserve1
            .checked_add(amount1_in)
            .ok_or(ErrorCode::ReserveOverflow)?;
        pair.total_supply = pair.total_supply
            .checked_add(liquidity)
            .ok_or(ErrorCode::SupplyOverflow)?;

        // Update cash reserves (initial state, r_debt = 0 => r_cash = r_virtual)
        pair.cash_reserve0 = pair.reserve0;
        pair.cash_reserve1 = pair.reserve1;

        // Initialize EMA prices based on initial liquidity
        pair.last_price0_ema = LastPriceEMA {
            symmetric: pair.spot_price0_nad(),
            directional: pair.spot_price0_nad(),
        };
        pair.last_price1_ema = LastPriceEMA {
            symmetric: pair.spot_price1_nad(),
            directional: pair.spot_price1_nad(),
        };

        let deployer_lp_balance = liquidity;
        let deployer_token0_amount = (deployer_lp_balance as u128)
            .checked_mul(pair.reserve0 as u128)
            .ok_or(ErrorCode::LiquidityMathOverflow)?
            .checked_div(pair.total_supply as u128)
            .ok_or(ErrorCode::LiquidityMathOverflow)?
            .try_into()
            .map_err(|_| ErrorCode::LiquidityConversionOverflow)?;
        let deployer_token1_amount = (deployer_lp_balance as u128)
            .checked_mul(pair.reserve1 as u128)
            .ok_or(ErrorCode::LiquidityMathOverflow)?
            .checked_div(pair.total_supply as u128)
            .ok_or(ErrorCode::LiquidityMathOverflow)?
            .try_into()
            .map_err(|_| ErrorCode::LiquidityConversionOverflow)?;

        emit_cpi!(PairCreatedEvent {
            metadata: EventMetadata::new(ctx.accounts.deployer.key(), pair.key()),
            token0: ctx.accounts.token0_mint.key(),
            token1: ctx.accounts.token1_mint.key(),
            lp_mint: ctx.accounts.lp_mint.key(),
            token0_decimals: ctx.accounts.token0_mint.decimals,
            token1_decimals: ctx.accounts.token1_mint.decimals,
            rate_model: ctx.accounts.rate_model.key(),
            swap_fee_bps: pair.swap_fee_bps,
            half_life: pair.half_life,
            fixed_cf_bps: pair.fixed_cf_bps,
            params_hash: pair.params_hash,
            version: pair.version,
        });

        emit_cpi!(MintEvent {
            metadata: EventMetadata::new(ctx.accounts.deployer.key(), pair.key()),
            amount0: amount0_in,
            amount1: amount1_in,
            liquidity: liquidity,
        });

        emit_cpi!(UserLiquidityPositionUpdatedEvent {
            metadata: EventMetadata::new(ctx.accounts.deployer.key(), pair.key()),
            token0_amount: deployer_token0_amount,
            token1_amount: deployer_token1_amount,
            lp_amount: deployer_lp_balance,
            token0_mint: pair.token0,
            token1_mint: pair.token1,
            lp_mint: ctx.accounts.lp_mint.key(),
        });
        

        Ok(())
    }   
}
