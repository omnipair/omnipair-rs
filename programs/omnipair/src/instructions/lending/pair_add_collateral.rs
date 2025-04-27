use anchor_lang::prelude::*;
use crate::{
    errors::ErrorCode,
    events::AdjustCollateralEvent,
    utils::{token::transfer_from_user_to_pool_vault, account::get_size_with_discriminator},
    instructions::lending::common::{BaseAdjustPosition, AdjustPositionArgs},
    state::{user_position::UserPosition, pair::Pair},
    constants::*,
};


#[derive(Accounts)]
pub struct AddCollateral<'info> {
    #[account(mut)]
    pub user: Signer<'info>,

    // #[account(
    //     mut,
    //     seeds = [
    //         PAIR_SEED_PREFIX, 
    //         pair.token0.as_ref(),
    //         pair.token1.as_ref()
    //     ],
    //     bump
    // )]
    // pub pair: Account<'info, Pair>,

    #[account(
        init_if_needed,
        payer = user,
        space = get_size_with_discriminator::<UserPosition>(),
        seeds = [
            POSITION_SEED_PREFIX,
            common.pair.key().as_ref(),
            common.user.key().as_ref()
        ],
        bump
    )]
    pub user_position: Account<'info, UserPosition>,

    #[account(mut)]
    pub common: BaseAdjustPosition<'info>,

    pub system_program: Program<'info, System>,
}


impl<'info> BaseAdjustPosition<'info> {
    pub fn validate_add(&self, args: &AdjustPositionArgs) -> Result<()> {
        let AdjustPositionArgs { amount } = args;
        
        require!(*amount > 0, ErrorCode::AmountZero);
        
        require_gte!(
            self.user_collateral_token_account.amount,
            *amount,
            ErrorCode::InsufficientBalanceForCollateral
        );
        
        Ok(())
    }

    pub fn validate_add_and_update(&mut self, args: &AdjustPositionArgs) -> Result<()> {
        self.validate_add(args)?;
        self.update()?;
        Ok(())
    }

    pub fn handle_add_collateral(ctx: Context<Self>, args: AdjustPositionArgs) -> Result<()> {
        let BaseAdjustPosition {
            pair,
            collateral_vault,
            user_collateral_token_account,
            collateral_token_mint,
            token_program,
            token_2022_program,
            user,
            ..
        } = ctx.accounts;

        // Update pair state
        pair.update(&ctx.accounts.rate_model)?;

        // Transfer tokens from user to collateral vault
        match user_collateral_token_account.mint == pair.token0 {
            true => {
                transfer_from_user_to_pool_vault(
                    user.to_account_info(),
                    user_collateral_token_account.to_account_info(),
                    collateral_vault.to_account_info(),
                    collateral_token_mint.to_account_info(),
                    match collateral_token_mint.to_account_info().owner == token_program.key {
                        true => token_program.to_account_info(),
                        false => token_2022_program.to_account_info(),
                    },
                    args.amount,
                    collateral_token_mint.decimals,
                )?;
                
                // Update collateral
                pair.total_collateral0 = pair.total_collateral0.checked_add(args.amount).unwrap();
                
            },
            false => {
                transfer_from_user_to_pool_vault(
                    user.to_account_info(),
                    user_collateral_token_account.to_account_info(),
                    collateral_vault.to_account_info(),
                    collateral_token_mint.to_account_info(),
                    match collateral_token_mint.to_account_info().owner == token_program.key {
                        true => token_program.to_account_info(),
                        false => token_2022_program.to_account_info(),
                    },
                    args.amount,
                    collateral_token_mint.decimals,
                )?;
                
                // Update collateral
                pair.total_collateral1 = pair.total_collateral1.checked_add(args.amount).unwrap();
            }
        }

        // Emit event
        let (amount0, amount1) = if user_collateral_token_account.mint == pair.token0 {
            (args.amount as i64, 0)
        } else {
            (0, args.amount as i64)
        };
        
        emit!(AdjustCollateralEvent {
            user: user.key(),
            amount0,
            amount1,
            timestamp: Clock::get()?.unix_timestamp,
        });

        Ok(())
    }
}
