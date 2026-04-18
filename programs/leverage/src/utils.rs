use crate::{constants::*, errors::LeverageError};
use anchor_lang::prelude::*;
use anchor_lang::solana_program::{
    hash::hash,
    instruction::{AccountMeta, Instruction},
    program::invoke,
};
use omnipair::{CloseLeverageArgs, OpenLeverageArgs};

/// Flat collection of account infos needed for the native omnipair leverage CPI.
/// Constructed from either `Multiply` or `CloseMultiply` accounts before calling
/// `invoke_open_leverage_raw` or `invoke_close_leverage_raw`.
pub struct NativeLeverageAccounts<'info> {
    pub pair: AccountInfo<'info>,
    pub rate_model: AccountInfo<'info>,
    pub futarchy_authority: AccountInfo<'info>,
    pub user_position: AccountInfo<'info>,
    pub token_in_vault: AccountInfo<'info>,
    pub token_out_vault: AccountInfo<'info>,
    pub collateral_vault: AccountInfo<'info>,
    pub user_token_in_account: AccountInfo<'info>,
    pub user_token_out_account: AccountInfo<'info>,
    pub token_in_mint: AccountInfo<'info>,
    pub token_out_mint: AccountInfo<'info>,
    pub user: AccountInfo<'info>,
    pub token_program: AccountInfo<'info>,
    pub token_2022_program: AccountInfo<'info>,
    pub system_program: AccountInfo<'info>,
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
    require_keys_eq!(
        ra[IDX_PAIR].key(),
        pair.key(),
        LeverageError::RemainingAccountMismatch
    );
    require_keys_eq!(
        ra[IDX_RATE_MODEL].key(),
        rate_model.key(),
        LeverageError::RemainingAccountMismatch
    );
    require_keys_eq!(
        ra[IDX_FUTARCHY].key(),
        futarchy.key(),
        LeverageError::RemainingAccountMismatch
    );
    require_keys_eq!(
        ra[IDX_OMNIPAIR_PROGRAM].key(),
        omnipair::ID,
        LeverageError::RemainingAccountMismatch
    );
    require_keys_eq!(
        ra[IDX_USER_LEV_POSITION].key(),
        user_lev_position,
        LeverageError::RemainingAccountMismatch
    );

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
    require_keys_eq!(
        ra[IDX_TOKEN_IN_VAULT].key(),
        expected_in_vault,
        LeverageError::RemainingAccountMismatch
    );
    require_keys_eq!(
        ra[IDX_TOKEN_OUT_VAULT].key(),
        expected_out_vault,
        LeverageError::RemainingAccountMismatch
    );

    Ok(())
}

pub fn invoke_open_leverage_raw<'info>(
    a: &NativeLeverageAccounts<'info>,
    ra: &[AccountInfo<'info>],
    args: OpenLeverageArgs,
) -> Result<()> {
    let mut ix_data = hash(b"global:open_leverage").to_bytes()[..8].to_vec();
    args.serialize(&mut ix_data)?;
    invoke_native_leverage_raw(a, ra, ix_data)
}

pub fn invoke_close_leverage_raw<'info>(
    a: &NativeLeverageAccounts<'info>,
    ra: &[AccountInfo<'info>],
    args: CloseLeverageArgs,
) -> Result<()> {
    let mut ix_data = hash(b"global:close_leverage").to_bytes()[..8].to_vec();
    args.serialize(&mut ix_data)?;
    invoke_native_leverage_raw(a, ra, ix_data)
}

fn invoke_native_leverage_raw<'info>(
    a: &NativeLeverageAccounts<'info>,
    ra: &[AccountInfo<'info>],
    ix_data: Vec<u8>,
) -> Result<()> {
    let omnipair_program = &ra[IDX_OMNIPAIR_PROGRAM];
    invoke(
        &Instruction {
            program_id: omnipair_program.key(),
            accounts: native_leverage_metas(a, &ra[IDX_EVENT_AUTHORITY], omnipair_program),
            data: ix_data,
        },
        &native_leverage_infos(a, &ra[IDX_EVENT_AUTHORITY], omnipair_program),
    )?;
    Ok(())
}

fn native_leverage_metas<'info>(
    a: &NativeLeverageAccounts<'info>,
    event_authority: &AccountInfo<'info>,
    omnipair_program: &AccountInfo<'info>,
) -> Vec<AccountMeta> {
    vec![
        AccountMeta::new(a.pair.key(), false),
        AccountMeta::new(a.rate_model.key(), false),
        AccountMeta::new_readonly(a.futarchy_authority.key(), false),
        AccountMeta::new(a.user_position.key(), false),
        AccountMeta::new(a.token_in_vault.key(), false),
        AccountMeta::new(a.token_out_vault.key(), false),
        AccountMeta::new(a.collateral_vault.key(), false),
        AccountMeta::new(a.user_token_in_account.key(), false),
        AccountMeta::new(a.user_token_out_account.key(), false),
        AccountMeta::new_readonly(a.token_in_mint.key(), false),
        AccountMeta::new_readonly(a.token_out_mint.key(), false),
        AccountMeta::new(a.user.key(), true),
        AccountMeta::new_readonly(a.token_program.key(), false),
        AccountMeta::new_readonly(a.token_2022_program.key(), false),
        AccountMeta::new_readonly(a.system_program.key(), false),
        AccountMeta::new_readonly(event_authority.key(), false),
        AccountMeta::new_readonly(omnipair_program.key(), false),
    ]
}

fn native_leverage_infos<'info>(
    a: &NativeLeverageAccounts<'info>,
    event_authority: &AccountInfo<'info>,
    omnipair_program: &AccountInfo<'info>,
) -> Vec<AccountInfo<'info>> {
    vec![
        a.pair.clone(),
        a.rate_model.clone(),
        a.futarchy_authority.clone(),
        a.user_position.clone(),
        a.token_in_vault.clone(),
        a.token_out_vault.clone(),
        a.collateral_vault.clone(),
        a.user_token_in_account.clone(),
        a.user_token_out_account.clone(),
        a.token_in_mint.clone(),
        a.token_out_mint.clone(),
        a.user.clone(),
        a.token_program.clone(),
        a.token_2022_program.clone(),
        a.system_program.clone(),
        event_authority.clone(),
        omnipair_program.clone(),
    ]
}
