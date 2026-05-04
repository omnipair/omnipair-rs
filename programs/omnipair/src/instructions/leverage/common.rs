use anchor_lang::{
    prelude::*,
    solana_program::{
        instruction::{AccountMeta, Instruction},
        program::{get_return_data, invoke},
    },
};

use crate::{
    constants::*,
    errors::ErrorCode,
    utils::{gamm_math::CPCurve, math::ceil_div},
};

pub const LEVERAGE_DELEGATE_CLOSE: u32 = 1 << 0;
pub const LEVERAGE_DELEGATE_ADD_MARGIN: u32 = 1 << 1;
pub const LEVERAGE_DELEGATE_REMOVE_MARGIN: u32 = 1 << 2;
pub const LEVERAGE_DELEGATE_INCREASE: u32 = 1 << 3;
pub const LEVERAGE_DELEGATE_DECREASE: u32 = 1 << 4;
pub const LEVERAGE_DELEGATION_APPROVAL_MAGIC: [u8; 8] = *b"OMNILVDA";
pub const LEVERAGE_DELEGATION_APPROVAL_VERSION: u8 = 1;

#[derive(AnchorSerialize, AnchorDeserialize, Clone, Default)]
pub struct DelegatedCpiArgs {
    pub before_ix_data: Vec<u8>,
    pub after_ix_data: Vec<u8>,
    pub before_accounts_len: u16,
}

#[derive(AnchorSerialize, AnchorDeserialize, Clone, Debug, PartialEq, Eq)]
pub struct LeverageDelegationApproval {
    pub magic: [u8; 8],
    pub version: u8,
    pub action: u32,
    pub pair: Pubkey,
    pub owner: Pubkey,
    pub position: Pubkey,
    pub delegation: Pubkey,
    pub is_debt_token0: bool,
    pub recipient_token_account: Pubkey,
    pub output_mint: Pubkey,
    pub output_amount: u64,
}

impl LeverageDelegationApproval {
    pub fn new(
        action: u32,
        pair: Pubkey,
        owner: Pubkey,
        position: Pubkey,
        delegation: Pubkey,
        is_debt_token0: bool,
        recipient_token_account: Pubkey,
        output_mint: Pubkey,
        output_amount: u64,
    ) -> Self {
        Self {
            magic: LEVERAGE_DELEGATION_APPROVAL_MAGIC,
            version: LEVERAGE_DELEGATION_APPROVAL_VERSION,
            action,
            pair,
            owner,
            position,
            delegation,
            is_debt_token0,
            recipient_token_account,
            output_mint,
            output_amount,
        }
    }
}

#[derive(Clone, Copy)]
pub struct SwapQuote {
    pub amount_out: u64,
    pub amount_in_after_swap_fee: u64,
    pub amount_in_with_lp_fee: u64,
    pub lp_fee: u64,
    pub protocol_fee: u64,
}

pub fn quote_swap(
    amount_in: u64,
    reserve_in: u64,
    reserve_out: u64,
    swap_fee_bps: u16,
    protocol_share_bps: u16,
) -> Result<SwapQuote> {
    require!(amount_in > 0, ErrorCode::AmountZero);
    require!(reserve_in > 0 && reserve_out > 0, ErrorCode::InsufficientLiquidity);

    let swap_fee = ceil_div(
        (amount_in as u128)
            .checked_mul(swap_fee_bps as u128)
            .ok_or(ErrorCode::FeeMathOverflow)?,
        BPS_DENOMINATOR as u128,
    )
    .ok_or(ErrorCode::FeeMathOverflow)? as u64;
    let protocol_fee = ceil_div(
        (swap_fee as u128)
            .checked_mul(protocol_share_bps as u128)
            .ok_or(ErrorCode::FeeMathOverflow)?,
        BPS_DENOMINATOR as u128,
    )
    .ok_or(ErrorCode::FeeMathOverflow)? as u64;
    let lp_fee = swap_fee.checked_sub(protocol_fee).unwrap_or(0);
    let amount_in_after_swap_fee = amount_in
        .checked_sub(swap_fee)
        .ok_or(ErrorCode::FeeMathOverflow)?;
    let amount_in_with_lp_fee = amount_in
        .checked_sub(protocol_fee)
        .ok_or(ErrorCode::FeeMathOverflow)?;
    let amount_out =
        CPCurve::calculate_amount_out(reserve_in, reserve_out, amount_in_after_swap_fee)?;

    Ok(SwapQuote {
        amount_out,
        amount_in_after_swap_fee,
        amount_in_with_lp_fee,
        lp_fee,
        protocol_fee,
    })
}

pub fn spot_value_from_reserves(
    collateral_amount: u64,
    collateral_reserve: u64,
    debt_reserve: u64,
) -> Result<u64> {
    require!(collateral_reserve > 0, ErrorCode::InsufficientLiquidity);
    Ok((collateral_amount as u128)
        .checked_mul(debt_reserve as u128)
        .ok_or(ErrorCode::Overflow)?
        .checked_div(collateral_reserve as u128)
        .ok_or(ErrorCode::Overflow)?
        .try_into()
        .map_err(|_| ErrorCode::Overflow)?)
}

pub fn unwind_impact_bps(spot_value: u64, closeout_value: u64) -> Result<u128> {
    require!(spot_value > 0, ErrorCode::InsufficientLiquidity);
    if closeout_value >= spot_value {
        return Ok(0);
    }

    Ok((spot_value as u128)
        .checked_sub(closeout_value as u128)
        .ok_or(ErrorCode::Overflow)?
        .checked_mul(BPS_DENOMINATOR as u128)
        .ok_or(ErrorCode::Overflow)?
        .checked_div(spot_value as u128)
        .ok_or(ErrorCode::Overflow)?)
}

pub fn equity_bps(closeout_value: u64, debt_amount: u64) -> Result<u128> {
    match closeout_value {
        0 => Ok(0),
        _ => Ok((closeout_value.saturating_sub(debt_amount) as u128)
            .checked_mul(BPS_DENOMINATOR as u128)
            .ok_or(ErrorCode::Overflow)?
            .checked_div(closeout_value as u128)
            .ok_or(ErrorCode::Overflow)?),
    }
}

pub fn require_initial_leverage_health(
    collateral_amount: u64,
    collateral_reserve: u64,
    debt_reserve: u64,
    closeout_value: u64,
    debt_amount: u64,
) -> Result<()> {
    require_gt!(closeout_value, debt_amount, ErrorCode::LeverageInitialMarginTooLow);
    let margin_bps = equity_bps(closeout_value, debt_amount)?;
    require_gte!(
        margin_bps,
        LEVERAGE_INITIAL_MARGIN_BPS as u128,
        ErrorCode::LeverageInitialMarginTooLow
    );

    let spot_value = spot_value_from_reserves(collateral_amount, collateral_reserve, debt_reserve)?;
    let unwind_bps = unwind_impact_bps(spot_value, closeout_value)?;
    require_gte!(
        LEVERAGE_MAX_UNWIND_IMPACT_BPS as u128,
        unwind_bps,
        ErrorCode::LeverageUnwindImpactTooHigh
    );
    Ok(())
}

pub fn require_leverage_not_liquidatable(closeout_value: u64, debt_amount: u64) -> Result<()> {
    let margin_bps = equity_bps(closeout_value, debt_amount)?;
    require!(
        closeout_value > debt_amount && margin_bps > LEVERAGE_MAINTENANCE_BUFFER_BPS as u128,
        ErrorCode::LeveragePositionNotLiquidatable
    );
    Ok(())
}

pub fn token_program_for_mint<'info>(
    mint: &AccountInfo<'info>,
    token_program: &AccountInfo<'info>,
    token_2022_program: &AccountInfo<'info>,
) -> AccountInfo<'info> {
    match mint.owner == token_program.key {
        true => token_program.clone(),
        false => token_2022_program.clone(),
    }
}

pub fn approved_for(approved_actions: u32, action: u32) -> Result<()> {
    require!(
        approved_actions & action == action,
        ErrorCode::InvalidLeverageDelegation
    );
    Ok(())
}

pub fn split_delegated_accounts<'a, 'info>(
    accounts: &'a [AccountInfo<'info>],
    before_accounts_len: u16,
) -> Result<(&'a [AccountInfo<'info>], &'a [AccountInfo<'info>])> {
    let before_accounts_len = before_accounts_len as usize;
    require!(
        before_accounts_len <= accounts.len(),
        ErrorCode::InvalidLeverageDelegation
    );
    Ok(accounts.split_at(before_accounts_len))
}

pub fn invoke_delegated_callback<'info>(
    delegated_program: &UncheckedAccount<'info>,
    data: Vec<u8>,
    accounts: &[AccountInfo<'info>],
    protected_accounts: &[Pubkey],
    writable_protected_accounts: &[Pubkey],
) -> Result<()> {
    require!(!data.is_empty(), ErrorCode::InvalidLeverageDelegation);
    require!(
        delegated_program.executable,
        ErrorCode::InvalidLeverageDelegation
    );

    let account_metas =
        delegated_account_metas(accounts, protected_accounts, writable_protected_accounts)?;

    let mut account_infos = Vec::with_capacity(accounts.len() + 1);
    account_infos.push(delegated_program.to_account_info());
    account_infos.extend(accounts.iter().cloned());

    invoke(
        &Instruction {
            program_id: delegated_program.key(),
            accounts: account_metas,
            data,
        },
        &account_infos,
    )
    .map_err(Into::into)
}

pub fn invoke_delegated_approval_callback<'info>(
    delegated_program: &UncheckedAccount<'info>,
    data: Vec<u8>,
    accounts: &[AccountInfo<'info>],
    protected_accounts: &[Pubkey],
    writable_protected_accounts: &[Pubkey],
    expected_action: u32,
    expected_pair: Pubkey,
    expected_owner: Pubkey,
    expected_position: Pubkey,
    expected_delegation: Pubkey,
    expected_is_debt_token0: bool,
    expected_recipient_token_account: Pubkey,
    expected_output_mint: Pubkey,
    expected_output_amount: u64,
) -> Result<()> {
    invoke_delegated_callback(
        delegated_program,
        data,
        accounts,
        protected_accounts,
        writable_protected_accounts,
    )?;

    let (program_id, data) = get_return_data().ok_or(ErrorCode::InvalidLeverageDelegation)?;
    validate_delegation_approval(
        program_id,
        &data,
        delegated_program.key(),
        expected_action,
        expected_pair,
        expected_owner,
        expected_position,
        expected_delegation,
        expected_is_debt_token0,
        expected_recipient_token_account,
        expected_output_mint,
        expected_output_amount,
    )
}

pub fn validate_delegation_approval(
    program_id: Pubkey,
    data: &[u8],
    expected_program: Pubkey,
    expected_action: u32,
    expected_pair: Pubkey,
    expected_owner: Pubkey,
    expected_position: Pubkey,
    expected_delegation: Pubkey,
    expected_is_debt_token0: bool,
    expected_recipient_token_account: Pubkey,
    expected_output_mint: Pubkey,
    expected_output_amount: u64,
) -> Result<()> {
    require_keys_eq!(program_id, expected_program, ErrorCode::InvalidLeverageDelegation);
    let mut data_ref = data;
    let approval = LeverageDelegationApproval::deserialize(&mut data_ref)
        .map_err(|_| ErrorCode::InvalidLeverageDelegation)?;
    require!(data_ref.is_empty(), ErrorCode::InvalidLeverageDelegation);
    require!(
        approval.magic == LEVERAGE_DELEGATION_APPROVAL_MAGIC,
        ErrorCode::InvalidLeverageDelegation
    );
    require!(
        approval.version == LEVERAGE_DELEGATION_APPROVAL_VERSION,
        ErrorCode::InvalidLeverageDelegation
    );
    require!(approval.action == expected_action, ErrorCode::InvalidLeverageDelegation);
    require_keys_eq!(approval.pair, expected_pair, ErrorCode::InvalidLeverageDelegation);
    require_keys_eq!(approval.owner, expected_owner, ErrorCode::InvalidLeverageDelegation);
    require_keys_eq!(
        approval.position,
        expected_position,
        ErrorCode::InvalidLeverageDelegation
    );
    require_keys_eq!(
        approval.delegation,
        expected_delegation,
        ErrorCode::InvalidLeverageDelegation
    );
    require!(
        approval.is_debt_token0 == expected_is_debt_token0,
        ErrorCode::InvalidLeverageDelegation
    );
    require_keys_eq!(
        approval.recipient_token_account,
        expected_recipient_token_account,
        ErrorCode::InvalidLeverageDelegation
    );
    require_keys_eq!(
        approval.output_mint,
        expected_output_mint,
        ErrorCode::InvalidLeverageDelegation
    );
    require!(
        approval.output_amount == expected_output_amount,
        ErrorCode::InvalidLeverageDelegation
    );
    Ok(())
}

fn delegated_account_metas(
    accounts: &[AccountInfo],
    protected_accounts: &[Pubkey],
    writable_protected_accounts: &[Pubkey],
) -> Result<Vec<AccountMeta>> {
    for (index, account) in accounts.iter().enumerate() {
        for prior in accounts.iter().take(index) {
            require_keys_neq!(account.key(), prior.key(), ErrorCode::InvalidLeverageDelegation);
        }
    }

    let mut account_metas = Vec::with_capacity(accounts.len());
    for account in accounts {
        let is_protected = protected_accounts.contains(account.key);
        let is_writable_protected = writable_protected_accounts.contains(account.key);
        if is_protected && !is_writable_protected {
            account_metas.push(AccountMeta::new_readonly(account.key(), false));
            continue;
        }
        if is_protected {
            require!(!account.is_signer, ErrorCode::InvalidLeverageDelegation);
        }
        account_metas.push(AccountMeta {
            pubkey: account.key(),
            is_signer: account.is_signer,
            is_writable: account.is_writable,
        });
    }
    Ok(account_metas)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_account(key: Pubkey, is_signer: bool, is_writable: bool) -> AccountInfo<'static> {
        let key = Box::leak(Box::new(key));
        let lamports = Box::leak(Box::new(0));
        let data = Box::leak(Vec::new().into_boxed_slice());
        let owner = Box::leak(Box::new(Pubkey::new_unique()));
        AccountInfo::new(key, is_signer, is_writable, lamports, data, owner, false, 0)
    }

    #[test]
    fn quote_swap_charges_fee_before_output() {
        let quote = quote_swap(1_000, 1_000_000, 1_000_000, 30, 1_000).unwrap();
        assert_eq!(quote.amount_in_after_swap_fee, 997);
        assert_eq!(quote.amount_in_with_lp_fee, 999);
        assert_eq!(quote.protocol_fee, 1);
        assert!(quote.amount_out < 1_000);
    }

    #[test]
    fn unwind_impact_is_zero_when_closeout_beats_spot() {
        assert_eq!(unwind_impact_bps(100, 101).unwrap(), 0);
    }

    #[test]
    fn unwind_impact_tracks_closeout_discount() {
        assert_eq!(unwind_impact_bps(1_000, 980).unwrap(), 200);
    }

    #[test]
    fn approved_for_checks_permission_bit() {
        assert!(approved_for(LEVERAGE_DELEGATE_CLOSE, LEVERAGE_DELEGATE_CLOSE).is_ok());
        assert!(approved_for(LEVERAGE_DELEGATE_CLOSE, LEVERAGE_DELEGATE_INCREASE).is_err());
    }

    #[test]
    fn split_delegated_accounts_respects_before_len() {
        let a = test_account(Pubkey::new_unique(), false, false);
        let b = test_account(Pubkey::new_unique(), false, false);
        let c = test_account(Pubkey::new_unique(), false, false);
        let accounts = vec![a, b, c];

        let (before, after) = split_delegated_accounts(&accounts, 2).unwrap();
        assert_eq!(before.len(), 2);
        assert_eq!(after.len(), 1);
        assert!(split_delegated_accounts(&accounts, 4).is_err());
    }

    #[test]
    fn delegated_account_metas_reject_duplicate_accounts() {
        let key = Pubkey::new_unique();
        let accounts = vec![
            test_account(key, false, false),
            test_account(key, false, false),
        ];

        assert!(delegated_account_metas(&accounts, &[], &[]).is_err());
    }

    #[test]
    fn delegated_account_metas_downgrade_protected_accounts_to_readonly() {
        let protected = Pubkey::new_unique();
        let open = Pubkey::new_unique();
        let accounts = vec![
            test_account(protected, true, true),
            test_account(open, true, true),
        ];

        let metas = delegated_account_metas(&accounts, &[protected], &[]).unwrap();
        assert_eq!(metas[0], AccountMeta::new_readonly(protected, false));
        assert_eq!(metas[1], AccountMeta::new(open, true));
    }

    #[test]
    fn delegated_account_metas_allow_explicit_writable_protected_non_signer() {
        let protected = Pubkey::new_unique();
        let accounts = vec![test_account(protected, false, true)];

        let metas = delegated_account_metas(&accounts, &[protected], &[protected]).unwrap();
        assert_eq!(metas[0], AccountMeta::new(protected, false));
    }

    #[test]
    fn delegated_account_metas_reject_writable_protected_signer() {
        let protected = Pubkey::new_unique();
        let accounts = vec![test_account(protected, true, true)];

        assert!(delegated_account_metas(&accounts, &[protected], &[protected]).is_err());
    }

    #[test]
    fn delegation_approval_accepts_exact_context() {
        let program = Pubkey::new_unique();
        let pair = Pubkey::new_unique();
        let owner = Pubkey::new_unique();
        let position = Pubkey::new_unique();
        let delegation = Pubkey::new_unique();
        let recipient = Pubkey::new_unique();
        let mint = Pubkey::new_unique();
        let approval = LeverageDelegationApproval::new(
            LEVERAGE_DELEGATE_CLOSE,
            pair,
            owner,
            position,
            delegation,
            true,
            recipient,
            mint,
            42,
        );
        let mut data = Vec::new();
        approval.serialize(&mut data).unwrap();

        assert!(validate_delegation_approval(
            program,
            &data,
            program,
            LEVERAGE_DELEGATE_CLOSE,
            pair,
            owner,
            position,
            delegation,
            true,
            recipient,
            mint,
            42,
        )
        .is_ok());
    }

    #[test]
    fn delegation_approval_rejects_wrong_context() {
        let program = Pubkey::new_unique();
        let pair = Pubkey::new_unique();
        let owner = Pubkey::new_unique();
        let position = Pubkey::new_unique();
        let delegation = Pubkey::new_unique();
        let recipient = Pubkey::new_unique();
        let mint = Pubkey::new_unique();
        let approval = LeverageDelegationApproval::new(
            LEVERAGE_DELEGATE_CLOSE,
            pair,
            owner,
            position,
            delegation,
            true,
            recipient,
            mint,
            42,
        );
        let mut data = Vec::new();
        approval.serialize(&mut data).unwrap();

        assert!(validate_delegation_approval(
            Pubkey::new_unique(),
            &data,
            program,
            LEVERAGE_DELEGATE_CLOSE,
            pair,
            owner,
            position,
            delegation,
            true,
            recipient,
            mint,
            42,
        )
        .is_err());
        assert!(validate_delegation_approval(
            program,
            &data,
            program,
            LEVERAGE_DELEGATE_INCREASE,
            pair,
            owner,
            position,
            delegation,
            true,
            recipient,
            mint,
            42,
        )
        .is_err());
        assert!(validate_delegation_approval(
            program,
            &data,
            program,
            LEVERAGE_DELEGATE_CLOSE,
            pair,
            Pubkey::new_unique(),
            position,
            delegation,
            true,
            recipient,
            mint,
            42,
        )
        .is_err());
        assert!(validate_delegation_approval(
            program,
            &data,
            program,
            LEVERAGE_DELEGATE_CLOSE,
            pair,
            owner,
            position,
            Pubkey::new_unique(),
            true,
            recipient,
            mint,
            42,
        )
        .is_err());
        assert!(validate_delegation_approval(
            program,
            &data,
            program,
            LEVERAGE_DELEGATE_CLOSE,
            pair,
            owner,
            position,
            delegation,
            false,
            recipient,
            mint,
            42,
        )
        .is_err());
        assert!(validate_delegation_approval(
            program,
            &data,
            program,
            LEVERAGE_DELEGATE_CLOSE,
            pair,
            owner,
            position,
            delegation,
            true,
            Pubkey::new_unique(),
            mint,
            42,
        )
        .is_err());
        assert!(validate_delegation_approval(
            program,
            &data,
            program,
            LEVERAGE_DELEGATE_CLOSE,
            pair,
            owner,
            position,
            delegation,
            true,
            recipient,
            Pubkey::new_unique(),
            42,
        )
        .is_err());
        assert!(validate_delegation_approval(
            program,
            &data,
            program,
            LEVERAGE_DELEGATE_CLOSE,
            pair,
            owner,
            position,
            delegation,
            true,
            recipient,
            mint,
            43,
        )
        .is_err());
    }

    #[test]
    fn initial_health_accepts_sufficient_margin_and_unwind_depth() {
        assert!(require_initial_leverage_health(100, 1_000_000, 1_000_000, 100, 90).is_ok());
    }

    #[test]
    fn initial_health_rejects_low_margin() {
        assert!(require_initial_leverage_health(100, 1_000_000, 1_000_000, 100, 91).is_err());
    }

    #[test]
    fn initial_health_rejects_high_unwind_impact() {
        assert!(require_initial_leverage_health(100, 1_000, 10_000, 900, 700).is_err());
    }

    #[test]
    fn non_liquidatable_check_uses_maintenance_buffer() {
        assert!(require_leverage_not_liquidatable(1_000, 900).is_ok());
        assert!(require_leverage_not_liquidatable(1_000, 940).is_err());
    }
}
