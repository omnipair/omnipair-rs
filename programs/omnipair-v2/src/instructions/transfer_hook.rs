use anchor_lang::{prelude::*, solana_program::program_error::ProgramError};
use anchor_spl::token_2022::{
    spl_token_2022::{
        extension::{
            transfer_hook::TransferHookAccount, BaseStateWithExtensions, StateWithExtensions,
        },
        state::Account as SplTokenAccount,
    },
    Token2022,
};
use spl_transfer_hook_interface::instruction::TransferHookInstruction;

use crate::{
    constants::YIELD_ACCOUNT_SEED_PREFIX,
    errors::ErrorCode,
    state::{Market, YieldAccount, YieldTokenKind},
};

const TRANSFER_HOOK_BASE_ACCOUNT_COUNT: usize = 4;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct TokenAccountSnapshot {
    owner: Pubkey,
    mint: Pubkey,
    amount: u64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct TransferBalances {
    source_pre_balance: u64,
    destination_pre_balance: u64,
}

#[derive(Clone, Copy, PartialEq, Eq)]
struct YieldContext {
    asset_mint: Pubkey,
    token_kind: YieldTokenKind,
    swap_fee_growth_index_nad: u128,
    interest_growth_index_nad: u128,
}

pub fn handle_transfer_hook<'info>(
    program_id: &Pubkey,
    accounts: &'info [AccountInfo<'info>],
    data: &[u8],
) -> Result<()> {
    match TransferHookInstruction::unpack(data).map_err(|_| error!(ErrorCode::InvalidArgument))? {
        TransferHookInstruction::Execute { amount } => handle_execute(program_id, accounts, amount),
        _ => err!(ErrorCode::InvalidArgument),
    }
}

fn handle_execute<'info>(
    program_id: &Pubkey,
    accounts: &'info [AccountInfo<'info>],
    amount: u64,
) -> Result<()> {
    require_gte!(
        accounts.len(),
        TRANSFER_HOOK_BASE_ACCOUNT_COUNT,
        ErrorCode::InvalidArgument
    );

    let source_token = parse_transferring_token_account(&accounts[0])?;
    let lp_mint = *accounts[1].key;
    let destination_token = parse_transferring_token_account(&accounts[2])?;
    require_keys_eq!(source_token.mint, lp_mint, ErrorCode::InvalidMint);
    require_keys_eq!(destination_token.mint, lp_mint, ErrorCode::InvalidMint);

    let balances = pre_transfer_balances(source_token.amount, destination_token.amount, amount)?;
    let (market_index, market, yield_context) = find_market_context(program_id, accounts, lp_mint)?;
    let market_key = *accounts[market_index].key;

    let source_yield_index = find_yield_account_index(
        program_id,
        accounts,
        source_token.owner,
        market_key,
        yield_context.asset_mint,
        yield_context.token_kind,
    )?;
    let destination_yield_index = find_yield_account_index(
        program_id,
        accounts,
        destination_token.owner,
        market_key,
        yield_context.asset_mint,
        yield_context.token_kind,
    )?;

    if source_yield_index == destination_yield_index {
        let combined_pre_balance = balances
            .source_pre_balance
            .checked_add(balances.destination_pre_balance)
            .ok_or(ErrorCode::MarketMathOverflow)?;
        checkpoint_yield_account(
            &accounts[source_yield_index],
            program_id,
            source_token.owner,
            market_key,
            yield_context,
            combined_pre_balance,
        )?;
    } else {
        checkpoint_yield_account(
            &accounts[source_yield_index],
            program_id,
            source_token.owner,
            market_key,
            yield_context,
            balances.source_pre_balance,
        )?;
        checkpoint_yield_account(
            &accounts[destination_yield_index],
            program_id,
            destination_token.owner,
            market_key,
            yield_context,
            balances.destination_pre_balance,
        )?;
    }

    // Keep the borrow checker honest that the deserialized market was consumed
    // only for validation and index selection.
    drop(market);
    Ok(())
}

fn parse_transferring_token_account(info: &AccountInfo) -> Result<TokenAccountSnapshot> {
    require_keys_eq!(*info.owner, Token2022::id(), ErrorCode::InvalidTokenAccount);
    let data = info.try_borrow_data()?;
    let token_account = StateWithExtensions::<SplTokenAccount>::unpack(&data)
        .map_err(|_| error!(ErrorCode::InvalidTokenAccount))?;
    let hook_account = token_account
        .get_extension::<TransferHookAccount>()
        .map_err(|_| error!(ErrorCode::InvalidTokenAccount))?;
    require!(
        bool::from(hook_account.transferring),
        ErrorCode::InvalidTokenAccount
    );
    Ok(TokenAccountSnapshot {
        owner: token_account.base.owner,
        mint: token_account.base.mint,
        amount: token_account.base.amount,
    })
}

fn pre_transfer_balances(
    source_post_balance: u64,
    destination_post_balance: u64,
    amount: u64,
) -> Result<TransferBalances> {
    let source_pre_balance = source_post_balance
        .checked_add(amount)
        .ok_or(ErrorCode::MarketMathOverflow)?;
    let destination_pre_balance = destination_post_balance
        .checked_sub(amount)
        .ok_or(ErrorCode::MarketMathOverflow)?;
    Ok(TransferBalances {
        source_pre_balance,
        destination_pre_balance,
    })
}

fn find_market_context<'info>(
    program_id: &Pubkey,
    accounts: &'info [AccountInfo<'info>],
    lp_mint: Pubkey,
) -> Result<(usize, Market, YieldContext)> {
    for (index, account_info) in accounts
        .iter()
        .enumerate()
        .skip(TRANSFER_HOOK_BASE_ACCOUNT_COUNT)
    {
        if account_info.owner != program_id {
            continue;
        }
        let Ok((market, yield_context)) = load_market_context(account_info, lp_mint) else {
            continue;
        };
        return Ok((index, market, yield_context));
    }
    err!(ErrorCode::InvalidMarket)
}

fn load_market_context(
    account_info: &AccountInfo,
    lp_mint: Pubkey,
) -> Result<(Market, YieldContext)> {
    let data = account_info.try_borrow_data()?;
    let mut data_cursor: &[u8] = &data;
    let market =
        Market::try_deserialize(&mut data_cursor).map_err(|_| error!(ErrorCode::InvalidMarket))?;
    let yield_context =
        infer_yield_context(&market, lp_mint).ok_or(error!(ErrorCode::InvalidMint))?;
    Ok((market, yield_context))
}

fn infer_yield_context(market: &Market, lp_mint: Pubkey) -> Option<YieldContext> {
    if market.base_side.ylp_mint == lp_mint {
        return Some(YieldContext {
            asset_mint: market.base_side.asset_mint,
            token_kind: YieldTokenKind::Ylp,
            swap_fee_growth_index_nad: market.base_side.fees.swap_fee_growth_index_nad,
            interest_growth_index_nad: market.base_side.fees.interest_growth_index_nad,
        });
    }
    if market.quote_side.ylp_mint == lp_mint {
        return Some(YieldContext {
            asset_mint: market.quote_side.asset_mint,
            token_kind: YieldTokenKind::Ylp,
            swap_fee_growth_index_nad: market.quote_side.fees.swap_fee_growth_index_nad,
            interest_growth_index_nad: market.quote_side.fees.interest_growth_index_nad,
        });
    }
    if market.base_side.hlp_mint == lp_mint {
        return Some(YieldContext {
            asset_mint: market.base_side.asset_mint,
            token_kind: YieldTokenKind::Hlp,
            swap_fee_growth_index_nad: market.base_hlp_vault.base_swap_fee_growth_index_nad,
            interest_growth_index_nad: market.base_hlp_vault.base_interest_growth_index_nad,
        });
    }
    if market.quote_side.hlp_mint == lp_mint {
        return Some(YieldContext {
            asset_mint: market.quote_side.asset_mint,
            token_kind: YieldTokenKind::Hlp,
            swap_fee_growth_index_nad: market.quote_hlp_vault.quote_swap_fee_growth_index_nad,
            interest_growth_index_nad: market.quote_hlp_vault.quote_interest_growth_index_nad,
        });
    }
    None
}

fn find_yield_account_index<'info>(
    program_id: &Pubkey,
    accounts: &'info [AccountInfo<'info>],
    owner: Pubkey,
    market: Pubkey,
    asset_mint: Pubkey,
    token_kind: YieldTokenKind,
) -> Result<usize> {
    for (index, account_info) in accounts
        .iter()
        .enumerate()
        .skip(TRANSFER_HOOK_BASE_ACCOUNT_COUNT)
    {
        if account_info.owner != program_id {
            continue;
        }
        let Ok(yield_account) = load_yield_account(account_info) else {
            continue;
        };
        if yield_account
            .assert_account(owner, market, asset_mint, token_kind)
            .is_ok()
            && validate_yield_account_pda(
                account_info.key,
                program_id,
                owner,
                market,
                asset_mint,
                token_kind,
                yield_account.bump,
            )
            .is_ok()
        {
            return Ok(index);
        }
    }
    err!(ErrorCode::InvalidYieldAccount)
}

fn checkpoint_yield_account(
    account_info: &AccountInfo,
    program_id: &Pubkey,
    owner: Pubkey,
    market: Pubkey,
    yield_context: YieldContext,
    pre_transfer_balance: u64,
) -> Result<()> {
    require_keys_eq!(
        *account_info.owner,
        *program_id,
        ErrorCode::InvalidYieldAccount
    );
    let mut data = account_info.try_borrow_mut_data()?;
    let mut data_cursor: &[u8] = &data;
    let mut yield_account = YieldAccount::try_deserialize(&mut data_cursor)
        .map_err(|_| error!(ErrorCode::InvalidYieldAccount))?;
    yield_account.assert_account(
        owner,
        market,
        yield_context.asset_mint,
        yield_context.token_kind,
    )?;
    validate_yield_account_pda(
        account_info.key,
        program_id,
        owner,
        market,
        yield_context.asset_mint,
        yield_context.token_kind,
        yield_account.bump,
    )?;
    checkpoint_yield_account_state(&mut yield_account, yield_context, pre_transfer_balance)?;
    let mut write_cursor = &mut data[..];
    yield_account
        .try_serialize(&mut write_cursor)
        .map_err(|_| ProgramError::InvalidAccountData)?;
    Ok(())
}

fn checkpoint_yield_account_state(
    yield_account: &mut YieldAccount,
    yield_context: YieldContext,
    pre_transfer_balance: u64,
) -> Result<()> {
    yield_account.accrue(
        pre_transfer_balance,
        yield_context.swap_fee_growth_index_nad,
        yield_context.interest_growth_index_nad,
    )
}

fn validate_yield_account_pda(
    account_key: &Pubkey,
    program_id: &Pubkey,
    owner: Pubkey,
    market: Pubkey,
    asset_mint: Pubkey,
    token_kind: YieldTokenKind,
    bump: u8,
) -> Result<()> {
    let (expected_key, expected_bump) = Pubkey::find_program_address(
        &[
            YIELD_ACCOUNT_SEED_PREFIX,
            market.as_ref(),
            owner.as_ref(),
            asset_mint.as_ref(),
            &[token_kind.code()],
        ],
        program_id,
    );
    require_keys_eq!(*account_key, expected_key, ErrorCode::InvalidYieldAccount);
    require_eq!(bump, expected_bump, ErrorCode::InvalidYieldAccount);
    Ok(())
}

fn load_yield_account(account_info: &AccountInfo) -> Result<YieldAccount> {
    let data = account_info.try_borrow_data()?;
    let mut data_cursor: &[u8] = &data;
    YieldAccount::try_deserialize(&mut data_cursor)
        .map_err(|_| error!(ErrorCode::InvalidYieldAccount))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::constants::NAD;

    #[test]
    fn reconstructs_pre_transfer_balances_from_post_transfer_state() {
        let balances = pre_transfer_balances(700, 350, 50).unwrap();
        assert_eq!(
            balances,
            TransferBalances {
                source_pre_balance: 750,
                destination_pre_balance: 300
            }
        );
    }

    #[test]
    fn rejects_post_transfer_destination_underflow() {
        let result = pre_transfer_balances(700, 49, 50);
        assert!(result.is_err());
    }

    #[test]
    fn checkpoints_yield_account_with_pre_transfer_balance() {
        let owner = Pubkey::new_unique();
        let market = Pubkey::new_unique();
        let asset_mint = Pubkey::new_unique();
        let mut yield_account = YieldAccount {
            owner: Pubkey::default(),
            market: Pubkey::default(),
            asset_mint: Pubkey::default(),
            token_kind: 0,
            recipient: Pubkey::default(),
            swap_fee_checkpoint_nad: 0,
            interest_checkpoint_nad: 0,
            accrued_swap_fee_amount: 0,
            accrued_interest_amount: 0,
            bump: 0,
        };
        yield_account.initialize(owner, market, asset_mint, YieldTokenKind::Ylp, owner, 255);
        let yield_context = YieldContext {
            asset_mint,
            token_kind: YieldTokenKind::Ylp,
            swap_fee_growth_index_nad: 3 * NAD as u128,
            interest_growth_index_nad: 2 * NAD as u128,
        };

        checkpoint_yield_account_state(&mut yield_account, yield_context, 10).unwrap();

        assert_eq!(yield_account.accrued_swap_fee_amount, 30);
        assert_eq!(yield_account.accrued_interest_amount, 20);
        assert_eq!(
            yield_account.swap_fee_checkpoint_nad,
            yield_context.swap_fee_growth_index_nad
        );
        assert_eq!(
            yield_account.interest_checkpoint_nad,
            yield_context.interest_growth_index_nad
        );
    }

    #[test]
    fn accepts_only_canonical_yield_account_pda() {
        let program_id = Pubkey::new_unique();
        let owner = Pubkey::new_unique();
        let market = Pubkey::new_unique();
        let asset_mint = Pubkey::new_unique();
        let (yield_account_key, bump) = Pubkey::find_program_address(
            &[
                YIELD_ACCOUNT_SEED_PREFIX,
                market.as_ref(),
                owner.as_ref(),
                asset_mint.as_ref(),
                &[YieldTokenKind::Ylp.code()],
            ],
            &program_id,
        );

        validate_yield_account_pda(
            &yield_account_key,
            &program_id,
            owner,
            market,
            asset_mint,
            YieldTokenKind::Ylp,
            bump,
        )
        .unwrap();
    }

    #[test]
    fn rejects_matching_yield_account_data_at_wrong_address() {
        let program_id = Pubkey::new_unique();
        let owner = Pubkey::new_unique();
        let market = Pubkey::new_unique();
        let asset_mint = Pubkey::new_unique();
        let (_, bump) = Pubkey::find_program_address(
            &[
                YIELD_ACCOUNT_SEED_PREFIX,
                market.as_ref(),
                owner.as_ref(),
                asset_mint.as_ref(),
                &[YieldTokenKind::Ylp.code()],
            ],
            &program_id,
        );
        let wrong_key = Pubkey::new_unique();

        let err = validate_yield_account_pda(
            &wrong_key,
            &program_id,
            owner,
            market,
            asset_mint,
            YieldTokenKind::Ylp,
            bump,
        )
        .unwrap_err();

        assert_eq!(err, error!(ErrorCode::InvalidYieldAccount));
    }

    #[test]
    fn rejects_matching_yield_account_data_with_wrong_bump() {
        let program_id = Pubkey::new_unique();
        let owner = Pubkey::new_unique();
        let market = Pubkey::new_unique();
        let asset_mint = Pubkey::new_unique();
        let (yield_account_key, bump) = Pubkey::find_program_address(
            &[
                YIELD_ACCOUNT_SEED_PREFIX,
                market.as_ref(),
                owner.as_ref(),
                asset_mint.as_ref(),
                &[YieldTokenKind::Ylp.code()],
            ],
            &program_id,
        );

        let err = validate_yield_account_pda(
            &yield_account_key,
            &program_id,
            owner,
            market,
            asset_mint,
            YieldTokenKind::Ylp,
            bump.wrapping_add(1),
        )
        .unwrap_err();

        assert_eq!(err, error!(ErrorCode::InvalidYieldAccount));
    }
}
