/// forked from raydium-cp-swap
/// https://github.com/raydium-io/raydium-cp-swap/blob/master/programs/cp-swap/src/utils/token.rs
/// Handles token transfers and minting with support for old token program and spl_token_2022
use crate::errors::ErrorCode;
use anchor_lang::{
    prelude::*,
    solana_program::{program::invoke, program_pack::Pack},
    system_program,
};
use anchor_spl::{
    token::{self, Token},
    token_2022::{
        self,
        spl_token_2022::{
            self,
            extension::{
                transfer_fee::{TransferFeeConfig, MAX_FEE_BASIS_POINTS},
                ExtensionType, StateWithExtensions,
            },
        },
        Token2022,
    },
    token_interface::{
        initialize_account3, spl_token_2022::extension::BaseStateWithExtensions,
        InitializeAccount3, Mint,
    },
};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct TransferAmounts {
    pub gross: u64,
    pub transfer_fee: u64,
    pub net: u64,
}

/// Syncs native SOL balance for a WSOL token account if the mint is the native mint.
/// This ensures the token account's `amount` field reflects any native SOL that was
/// sent directly to the account.
pub fn sync_native_if_wsol<'a>(
    mint: &Pubkey,
    token_account: &AccountInfo<'a>,
    token_program: &AccountInfo<'a>,
) -> Result<()> {
    if *mint == spl_token::native_mint::id() {
        invoke(
            &spl_token::instruction::sync_native(token_program.key, token_account.key)?,
            &[token_program.clone(), token_account.clone()],
        )?;
    }
    Ok(())
}

pub fn token_program_for_mint<'info>(
    mint: &AccountInfo<'info>,
    token_program: &AccountInfo<'info>,
    token_2022_program: &AccountInfo<'info>,
) -> Result<AccountInfo<'info>> {
    if mint.owner == token_program.key {
        return Ok(token_program.clone());
    }
    if mint.owner == token_2022_program.key {
        return Ok(token_2022_program.clone());
    }
    err!(ErrorCode::InvalidTokenProgram)
}

pub fn require_supported_mint(mint_account: &InterfaceAccount<Mint>) -> Result<()> {
    require!(
        is_supported_mint(mint_account)?,
        ErrorCode::UnsupportedTokenExtension
    );
    Ok(())
}

pub fn transfer_amounts_from_gross(mint_info: &AccountInfo, gross: u64) -> Result<TransferAmounts> {
    let transfer_fee = get_transfer_fee(mint_info, gross)?;
    let net = gross
        .checked_sub(transfer_fee)
        .ok_or(ErrorCode::FeeMathOverflow)?;
    Ok(TransferAmounts {
        gross,
        transfer_fee,
        net,
    })
}

pub fn transfer_amounts_from_net(mint_info: &AccountInfo, net: u64) -> Result<TransferAmounts> {
    let transfer_fee = get_transfer_inverse_fee(mint_info, net)?;
    let gross = net
        .checked_add(transfer_fee)
        .ok_or(ErrorCode::FeeMathOverflow)?;
    Ok(TransferAmounts {
        gross,
        transfer_fee,
        net,
    })
}

pub fn transfer_from_user_to_vault<'a>(
    authority: AccountInfo<'a>,
    from: AccountInfo<'a>,
    to_vault: AccountInfo<'a>,
    mint: AccountInfo<'a>,
    token_program: AccountInfo<'a>,
    amount: u64,
    mint_decimals: u8,
) -> Result<()> {
    if amount == 0 {
        return Ok(());
    }

    if *token_program.key == Token2022::id() {
        token_2022::transfer_checked(
            CpiContext::new(
                token_program.to_account_info(),
                token_2022::TransferChecked {
                    from,
                    to: to_vault,
                    authority,
                    mint,
                },
            ),
            amount,
            mint_decimals,
        )
    } else {
        token::transfer_checked(
            CpiContext::new(
                token_program.to_account_info(),
                token::TransferChecked {
                    from,
                    to: to_vault,
                    authority,
                    mint,
                },
            ),
            amount,
            mint_decimals,
        )
    }
}

pub fn transfer_from_user_to_vault_gross<'a>(
    authority: AccountInfo<'a>,
    from: AccountInfo<'a>,
    to_vault: AccountInfo<'a>,
    mint: AccountInfo<'a>,
    token_program: AccountInfo<'a>,
    gross_amount: u64,
    mint_decimals: u8,
) -> Result<TransferAmounts> {
    let transfer_amounts = transfer_amounts_from_gross(&mint, gross_amount)?;
    transfer_from_user_to_vault(
        authority,
        from,
        to_vault,
        mint,
        token_program,
        transfer_amounts.gross,
        mint_decimals,
    )?;
    Ok(transfer_amounts)
}

pub fn transfer_from_user_to_vault_net<'a>(
    authority: AccountInfo<'a>,
    from: AccountInfo<'a>,
    to_vault: AccountInfo<'a>,
    mint: AccountInfo<'a>,
    token_program: AccountInfo<'a>,
    net_amount: u64,
    mint_decimals: u8,
) -> Result<TransferAmounts> {
    let transfer_amounts = transfer_amounts_from_net(&mint, net_amount)?;
    transfer_from_user_to_vault(
        authority,
        from,
        to_vault,
        mint,
        token_program,
        transfer_amounts.gross,
        mint_decimals,
    )?;
    Ok(transfer_amounts)
}

pub fn transfer_from_vault<'a>(
    authority: AccountInfo<'a>,
    from_vault: AccountInfo<'a>,
    to: AccountInfo<'a>,
    mint: AccountInfo<'a>,
    token_program: AccountInfo<'a>,
    amount: u64,
    mint_decimals: u8,
    signer_seeds: &[&[&[u8]]],
) -> Result<()> {
    if amount == 0 {
        return Ok(());
    }
    if *token_program.key == Token2022::id() {
        token_2022::transfer_checked(
            CpiContext::new_with_signer(
                token_program.to_account_info(),
                token_2022::TransferChecked {
                    from: from_vault,
                    to,
                    authority,
                    mint,
                },
                signer_seeds,
            ),
            amount,
            mint_decimals,
        )
    } else {
        token::transfer_checked(
            CpiContext::new_with_signer(
                token_program.to_account_info(),
                token::TransferChecked {
                    from: from_vault,
                    to,
                    authority,
                    mint,
                },
                signer_seeds,
            ),
            amount,
            mint_decimals,
        )
    }
}

/// Transfers tokens from one vault account to another vault account.
///
/// This function is an explicit alias for `transfer_from_vault`, providing clearer intent for vault-to-vault token movement.
/// Arguments:
///   - `authority`: The account authorized to sign for the transfer (typically a PDA).
///   - `from_vault`: The source token account (vault).
///   - `to_vault`: The destination token account (vault).
///   - `mint`: The mint for the token being transferred.
///   - `token_program`: The token program account (can be SPL Token or Token2022).
pub fn transfer_from_vault_to_vault<'a>(
    authority: AccountInfo<'a>,
    from_vault: AccountInfo<'a>,
    to_vault: AccountInfo<'a>,
    mint: AccountInfo<'a>,
    token_program: AccountInfo<'a>,
    amount: u64,
    mint_decimals: u8,
    signer_seeds: &[&[&[u8]]],
) -> Result<()> {
    transfer_from_vault(
        authority,
        from_vault,
        to_vault,
        mint,
        token_program.to_account_info(),
        amount,
        mint_decimals,
        signer_seeds,
    )
}

pub fn transfer_from_vault_to_vault_gross<'a>(
    authority: AccountInfo<'a>,
    from_vault: AccountInfo<'a>,
    to_vault: AccountInfo<'a>,
    mint: AccountInfo<'a>,
    token_program: AccountInfo<'a>,
    gross_amount: u64,
    mint_decimals: u8,
    signer_seeds: &[&[&[u8]]],
) -> Result<TransferAmounts> {
    let transfer_amounts = transfer_amounts_from_gross(&mint, gross_amount)?;
    transfer_from_vault_to_vault(
        authority,
        from_vault,
        to_vault,
        mint,
        token_program,
        transfer_amounts.gross,
        mint_decimals,
        signer_seeds,
    )?;
    Ok(transfer_amounts)
}

/// Transfers tokens from one vault account to a user's token account.
///
/// This function is an explicit alias for `transfer_from_vault`, providing clearer intent for vault-to-user token movement.
/// Arguments:
///   - `authority`: The account authorized to sign for the transfer (typically a PDA).
///   - `from_vault`: The source token account (vault).
///   - `to_vault`: The destination token account (vault).
///   - `mint`: The mint for the token being transferred.
///   - `token_program`: The token program account (can be SPL Token or Token2022).
///   - `amount`: Number of tokens to transfer.
///   - `mint_decimals`: Decimals for the mint (to support checked instruction).
///   - `signer_seeds`: Seeds used for PDA authority (for cross-program invocation).
/// Returns:
///   - Result containing unit on success or an error on failure.
pub fn transfer_from_vault_to_user<'a>(
    authority: AccountInfo<'a>,
    from_vault: AccountInfo<'a>,
    to: AccountInfo<'a>,
    mint: AccountInfo<'a>,
    token_program: AccountInfo<'a>,
    amount: u64,
    mint_decimals: u8,
    signer_seeds: &[&[&[u8]]],
) -> Result<()> {
    transfer_from_vault(
        authority,
        from_vault,
        to,
        mint,
        token_program.to_account_info(),
        amount,
        mint_decimals,
        signer_seeds,
    )
}

pub fn transfer_from_vault_to_user_gross<'a>(
    authority: AccountInfo<'a>,
    from_vault: AccountInfo<'a>,
    to: AccountInfo<'a>,
    mint: AccountInfo<'a>,
    token_program: AccountInfo<'a>,
    gross_amount: u64,
    mint_decimals: u8,
    signer_seeds: &[&[&[u8]]],
) -> Result<TransferAmounts> {
    let transfer_amounts = transfer_amounts_from_gross(&mint, gross_amount)?;
    transfer_from_vault_to_user(
        authority,
        from_vault,
        to,
        mint,
        token_program,
        transfer_amounts.gross,
        mint_decimals,
        signer_seeds,
    )?;
    Ok(transfer_amounts)
}

/// Issue a spl_token `MintTo` instruction.
pub fn token_mint_to<'a>(
    authority: AccountInfo<'a>,
    token_program: AccountInfo<'a>,
    mint: AccountInfo<'a>,
    destination: AccountInfo<'a>,
    amount: u64,
    signer_seeds: &[&[&[u8]]],
) -> Result<()> {
    if amount == 0 {
        return Ok(());
    }
    token_2022::mint_to(
        CpiContext::new_with_signer(
            token_program,
            token_2022::MintTo {
                to: destination,
                authority,
                mint,
            },
            signer_seeds,
        ),
        amount,
    )
}

pub fn token_burn<'a>(
    authority: AccountInfo<'a>,
    token_program: AccountInfo<'a>,
    mint: AccountInfo<'a>,
    from: AccountInfo<'a>,
    amount: u64,
    signer_seeds: &[&[&[u8]]],
) -> Result<()> {
    if amount == 0 {
        return Ok(());
    }
    token_2022::burn(
        CpiContext::new_with_signer(
            token_program.to_account_info(),
            token_2022::Burn {
                from,
                authority,
                mint,
            },
            signer_seeds,
        ),
        amount,
    )
}

/// Calculate the fee for output amount
pub fn get_transfer_inverse_fee(mint_info: &AccountInfo, post_fee_amount: u64) -> Result<u64> {
    if *mint_info.owner == Token::id() {
        return Ok(0);
    }
    if post_fee_amount == 0 {
        return Ok(0);
    }
    let mint_data = mint_info.try_borrow_data()?;
    let mint = StateWithExtensions::<spl_token_2022::state::Mint>::unpack(&mint_data)?;

    let fee = if let Ok(transfer_fee_config) = mint.get_extension::<TransferFeeConfig>() {
        let epoch = Clock::get()?.epoch;

        let transfer_fee = transfer_fee_config.get_epoch_fee(epoch);
        if u16::from(transfer_fee.transfer_fee_basis_points) == MAX_FEE_BASIS_POINTS {
            u64::from(transfer_fee.maximum_fee)
        } else {
            transfer_fee_config
                .calculate_inverse_epoch_fee(epoch, post_fee_amount)
                .ok_or(ErrorCode::FeeMathOverflow)?
        }
    } else {
        0
    };
    Ok(fee)
}

/// Calculate the fee for input amount
pub fn get_transfer_fee(mint_info: &AccountInfo, pre_fee_amount: u64) -> Result<u64> {
    if *mint_info.owner == Token::id() {
        return Ok(0);
    }
    let mint_data = mint_info.try_borrow_data()?;
    let mint = StateWithExtensions::<spl_token_2022::state::Mint>::unpack(&mint_data)?;

    let fee = if let Ok(transfer_fee_config) = mint.get_extension::<TransferFeeConfig>() {
        transfer_fee_config
            .calculate_epoch_fee(Clock::get()?.epoch, pre_fee_amount)
            .ok_or(ErrorCode::FeeMathOverflow)?
    } else {
        0
    };
    Ok(fee)
}

pub fn is_supported_mint(mint_account: &InterfaceAccount<Mint>) -> Result<bool> {
    let mint_info = mint_account.to_account_info();
    if *mint_info.owner == Token::id() {
        return Ok(true);
    }
    if *mint_info.owner != Token2022::id() {
        return Ok(false);
    }

    let mint_data = mint_info.try_borrow_data()?;
    let mint = StateWithExtensions::<spl_token_2022::state::Mint>::unpack(&mint_data)?;
    let extensions = mint.get_extension_types()?;
    for e in extensions {
        if !is_supported_extension_type(e) {
            return Ok(false);
        }
    }
    Ok(true)
}

fn is_supported_extension_type(extension: ExtensionType) -> bool {
    matches!(
        extension,
        ExtensionType::TransferFeeConfig
            | ExtensionType::MetadataPointer
            | ExtensionType::TokenMetadata
    )
}

pub fn create_token_account<'a>(
    authority: &AccountInfo<'a>,
    payer: &AccountInfo<'a>,
    token_account: &AccountInfo<'a>,
    mint_account: &AccountInfo<'a>,
    system_program: &AccountInfo<'a>,
    token_program: &AccountInfo<'a>,
    signer_seeds: &[&[u8]],
) -> Result<()> {
    if token_account.owner == token_program.key && token_account.data_len() > 0 {
        let account_data = token_account.try_borrow_data()?;
        let token_account_state =
            StateWithExtensions::<spl_token_2022::state::Account>::unpack(&account_data)?;
        require_keys_eq!(
            token_account_state.base.mint,
            mint_account.key(),
            ErrorCode::InvalidTokenAccount
        );
        require_keys_eq!(
            token_account_state.base.owner,
            authority.key(),
            ErrorCode::InvalidTokenAccount
        );
        return Ok(());
    }

    let space = {
        let mint_info = mint_account.to_account_info();
        if *mint_info.owner == token_2022::Token2022::id() {
            let mint_data = mint_info.try_borrow_data()?;
            let mint_state =
                StateWithExtensions::<spl_token_2022::state::Mint>::unpack(&mint_data)?;
            let mint_extensions = mint_state.get_extension_types()?;
            let required_extensions =
                ExtensionType::get_required_init_account_extensions(&mint_extensions);
            ExtensionType::try_calculate_account_len::<spl_token_2022::state::Account>(
                &required_extensions,
            )?
        } else {
            spl_token::state::Account::LEN
        }
    };
    create_or_allocate_account(
        token_program.key,
        payer.to_account_info(),
        system_program.to_account_info(),
        token_account.to_account_info(),
        signer_seeds,
        space,
    )?;
    initialize_account3(CpiContext::new(
        token_program.to_account_info(),
        InitializeAccount3 {
            account: token_account.to_account_info(),
            mint: mint_account.to_account_info(),
            authority: authority.to_account_info(),
        },
    ))
}

pub fn create_or_allocate_account<'a>(
    program_id: &Pubkey,
    payer: AccountInfo<'a>,
    system_program: AccountInfo<'a>,
    target_account: AccountInfo<'a>,
    siger_seed: &[&[u8]],
    space: usize,
) -> Result<()> {
    let rent = Rent::get()?;
    let current_lamports = target_account.lamports();

    if current_lamports == 0 {
        let lamports = rent.minimum_balance(space);
        let cpi_accounts = system_program::CreateAccount {
            from: payer,
            to: target_account.clone(),
        };
        let cpi_context = CpiContext::new(system_program.clone(), cpi_accounts);
        system_program::create_account(
            cpi_context.with_signer(&[siger_seed]),
            lamports,
            u64::try_from(space).unwrap(),
            program_id,
        )?;
    } else {
        let required_lamports = rent
            .minimum_balance(space)
            .max(1)
            .saturating_sub(current_lamports);
        if required_lamports > 0 {
            let cpi_accounts = system_program::Transfer {
                from: payer.to_account_info(),
                to: target_account.clone(),
            };
            let cpi_context = CpiContext::new(system_program.clone(), cpi_accounts);
            system_program::transfer(cpi_context, required_lamports)?;
        }
        let cpi_accounts = system_program::Allocate {
            account_to_allocate: target_account.clone(),
        };
        let cpi_context = CpiContext::new(system_program.clone(), cpi_accounts);
        system_program::allocate(
            cpi_context.with_signer(&[siger_seed]),
            u64::try_from(space).unwrap(),
        )?;

        let cpi_accounts = system_program::Assign {
            account_to_assign: target_account.clone(),
        };
        let cpi_context = CpiContext::new(system_program.clone(), cpi_accounts);
        system_program::assign(cpi_context.with_signer(&[siger_seed]), program_id)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_mint_account(owner: Pubkey) -> AccountInfo<'static> {
        let key = Box::leak(Box::new(Pubkey::new_unique()));
        let lamports = Box::leak(Box::new(0));
        let data = Box::leak(Vec::new().into_boxed_slice());
        let owner = Box::leak(Box::new(owner));
        AccountInfo::new(key, false, false, lamports, data, owner, false, 0)
    }

    #[test]
    fn supported_extension_filter_allows_declared_safe_set() {
        assert!(is_supported_extension_type(
            ExtensionType::TransferFeeConfig
        ));
        assert!(is_supported_extension_type(ExtensionType::MetadataPointer));
        assert!(is_supported_extension_type(ExtensionType::TokenMetadata));
    }

    #[test]
    fn supported_extension_filter_rejects_extensions_requiring_extra_handling() {
        assert!(!is_supported_extension_type(
            ExtensionType::MintCloseAuthority
        ));
        assert!(!is_supported_extension_type(
            ExtensionType::PermanentDelegate
        ));
        assert!(!is_supported_extension_type(
            ExtensionType::DefaultAccountState
        ));
        assert!(!is_supported_extension_type(ExtensionType::TransferHook));
    }

    #[test]
    fn classic_spl_transfer_amounts_have_no_fee() {
        let mint = test_mint_account(Token::id());

        assert_eq!(
            transfer_amounts_from_gross(&mint, 1_234).unwrap(),
            TransferAmounts {
                gross: 1_234,
                transfer_fee: 0,
                net: 1_234,
            }
        );
        assert_eq!(
            transfer_amounts_from_net(&mint, 5_678).unwrap(),
            TransferAmounts {
                gross: 5_678,
                transfer_fee: 0,
                net: 5_678,
            }
        );
    }

    #[test]
    fn classic_spl_zero_amount_transfer_math_is_stable() {
        let mint = test_mint_account(Token::id());

        assert_eq!(
            transfer_amounts_from_gross(&mint, 0).unwrap(),
            TransferAmounts::default()
        );
        assert_eq!(
            transfer_amounts_from_net(&mint, 0).unwrap(),
            TransferAmounts::default()
        );
    }
}
