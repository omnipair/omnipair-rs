use anchor_lang::prelude::*;
use anchor_lang::solana_program::{
    instruction::{AccountMeta, Instruction},
    program::invoke,
    hash::hash,
};
use omnipair::FlashloanArgs;
use crate::{constants::*, errors::LeverageError, types::InternalCallbackData};

/// Flat collection of account infos needed for the flashloan CPI.
/// Constructed from either `Multiply` or `CloseMultiply` accounts before calling
/// `invoke_flashloan_raw`.
pub struct FlashloanAccounts<'info> {
    pub pair: AccountInfo<'info>,
    pub rate_model: AccountInfo<'info>,
    pub futarchy_authority: AccountInfo<'info>,
    pub reserve0_vault: AccountInfo<'info>,
    pub reserve1_vault: AccountInfo<'info>,
    pub token0_mint: AccountInfo<'info>,
    pub token1_mint: AccountInfo<'info>,
    pub repay0_vault: AccountInfo<'info>,
    pub repay1_vault: AccountInfo<'info>,
    pub receiver_token0_account: AccountInfo<'info>,
    pub receiver_token1_account: AccountInfo<'info>,
    pub receiver_program: AccountInfo<'info>,
    pub user: AccountInfo<'info>,
    pub token_program: AccountInfo<'info>,
    pub token_2022_program: AccountInfo<'info>,
    pub system_program: AccountInfo<'info>,
    pub user_leverage_position: AccountInfo<'info>,
}

/// Validates that remaining_accounts keys match the corresponding validated
/// outer accounts. Prevents a malicious client from substituting a different
/// pair or vault in remaining_accounts while using correct ones on the outer CPI.
pub fn validate_remaining_accounts<'info>(
    ra: &[AccountInfo<'info>],
    pair: &AccountInfo<'info>,
    rate_model: &AccountInfo<'info>,
    futarchy: &AccountInfo<'info>,
    reserve0_vault: Pubkey,
    reserve1_vault: Pubkey,
    is_lev_collateral0: bool,
    is_close: bool,
    user_lev_position: Pubkey,
) -> Result<()> {
    require_keys_eq!(ra[IDX_PAIR].key(), pair.key(), LeverageError::RemainingAccountMismatch);
    require_keys_eq!(ra[IDX_RATE_MODEL].key(), rate_model.key(), LeverageError::RemainingAccountMismatch);
    require_keys_eq!(ra[IDX_FUTARCHY].key(), futarchy.key(), LeverageError::RemainingAccountMismatch);
    require_keys_eq!(ra[IDX_OMNIPAIR_PROGRAM].key(), omnipair::ID, LeverageError::RemainingAccountMismatch);
    require_keys_eq!(ra[IDX_USER_LEV_POSITION].key(), user_lev_position, LeverageError::RemainingAccountMismatch);

    // TOKEN_IN/OUT vault direction depends on whether this is open or close:
    //   open:  TOKEN_IN = lev_collateral reserve, TOKEN_OUT = position token reserve
    //   close: TOKEN_IN = position token reserve, TOKEN_OUT = lev_collateral reserve
    let (lev_collateral_vault, position_token_vault) = if is_lev_collateral0 {
        (reserve0_vault, reserve1_vault)
    } else {
        (reserve1_vault, reserve0_vault)
    };
    let (expected_in_vault, expected_out_vault) = if is_close {
        (position_token_vault, lev_collateral_vault)
    } else {
        (lev_collateral_vault, position_token_vault)
    };
    require_keys_eq!(ra[IDX_TOKEN_IN_VAULT].key(), expected_in_vault, LeverageError::RemainingAccountMismatch);
    require_keys_eq!(ra[IDX_TOKEN_OUT_VAULT].key(), expected_out_vault, LeverageError::RemainingAccountMismatch);

    Ok(())
}

/// Builds and invokes the omnipair flashloan CPI.
/// Remaining accounts are forwarded verbatim to the callback.
pub fn invoke_flashloan_raw<'info>(
    a: &FlashloanAccounts<'info>,
    ra: &[AccountInfo<'info>],
    amount0: u64,
    amount1: u64,
    callback_data: InternalCallbackData,
) -> Result<()> {
    let omnipair_program = &ra[IDX_OMNIPAIR_PROGRAM];
    let event_authority  = &ra[IDX_EVENT_AUTHORITY];

    let mut flashloan_metas = vec![
        AccountMeta::new(a.pair.key(), false),
        AccountMeta::new(a.rate_model.key(), false),
        AccountMeta::new_readonly(a.futarchy_authority.key(), false),
        AccountMeta::new(a.reserve0_vault.key(), false),
        AccountMeta::new(a.reserve1_vault.key(), false),
        AccountMeta::new_readonly(a.token0_mint.key(), false),
        AccountMeta::new_readonly(a.token1_mint.key(), false),
        AccountMeta::new(a.repay0_vault.key(), false),
        AccountMeta::new(a.repay1_vault.key(), false),
        AccountMeta::new(a.receiver_token0_account.key(), false),
        AccountMeta::new(a.receiver_token1_account.key(), false),
        AccountMeta::new_readonly(a.receiver_program.key(), false),
        AccountMeta::new(a.user.key(), true),
        AccountMeta::new_readonly(a.token_program.key(), false),
        AccountMeta::new_readonly(a.token_2022_program.key(), false),
        AccountMeta::new_readonly(a.system_program.key(), false),
        AccountMeta::new_readonly(event_authority.key(), false),
        AccountMeta::new_readonly(omnipair_program.key(), false),
    ];
    // remaining_accounts[0..11] forwarded verbatim to the callback
    for acc in ra.iter() {
        flashloan_metas.push(AccountMeta {
            pubkey: acc.key(),
            is_signer: acc.is_signer,
            is_writable: acc.is_writable,
        });
    }

    let mut callback_bytes = Vec::new();
    callback_data.serialize(&mut callback_bytes)?;

    let mut ix_data = hash(b"global:flashloan").to_bytes()[..8].to_vec();
    FlashloanArgs { amount0, amount1, data: callback_bytes }.serialize(&mut ix_data)?;

    let mut account_infos = vec![
        a.pair.clone(),
        a.rate_model.clone(),
        a.futarchy_authority.clone(),
        a.reserve0_vault.clone(),
        a.reserve1_vault.clone(),
        a.token0_mint.clone(),
        a.token1_mint.clone(),
        a.repay0_vault.clone(),
        a.repay1_vault.clone(),
        a.receiver_token0_account.clone(),
        a.receiver_token1_account.clone(),
        a.receiver_program.clone(),
        a.user.clone(),
        a.token_program.clone(),
        a.token_2022_program.clone(),
        a.system_program.clone(),
        event_authority.to_account_info(),
        omnipair_program.to_account_info(),
    ];
    for acc in ra.iter() {
        account_infos.push(acc.to_account_info());
    }

    invoke(
        &Instruction {
            program_id: omnipair_program.key(),
            accounts: flashloan_metas,
            data: ix_data,
        },
        &account_infos,
    )?;
    Ok(())
}
