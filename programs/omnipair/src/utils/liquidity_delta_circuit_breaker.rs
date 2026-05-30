use anchor_lang::prelude::*;
use anchor_lang::solana_program::{
    instruction::Instruction,
    sysvar,
    sysvar::instructions::{load_current_index_checked, load_instruction_at_checked},
};

use crate::errors::ErrorCode;

const ADD_LIQUIDITY_DISCRIMINATOR: [u8; 8] = [0xb5, 0x9d, 0x59, 0x43, 0x8f, 0xb6, 0x34, 0x48];
const REMOVE_LIQUIDITY_DISCRIMINATOR: [u8; 8] = [0x50, 0x55, 0xd1, 0x48, 0x18, 0xce, 0xb1, 0x6c];

#[derive(Clone, Copy)]
pub enum LiquidityDeltaInstruction {
    AddLiquidity,
    RemoveLiquidity,
}

pub fn require_top_level_liquidity_delta_ix(
    pair: &Pubkey,
    instructions_sysvar: &AccountInfo,
    expected_instruction: LiquidityDeltaInstruction,
) -> Result<()> {
    require_keys_eq!(
        *instructions_sysvar.key,
        sysvar::instructions::ID,
        ErrorCode::InvalidInstructionsSysvar
    );

    let current_index = load_current_index_checked(instructions_sysvar)
        .map_err(|_| error!(ErrorCode::InvalidInstructionsSysvar))?
        as usize;
    let current_instruction = load_instruction_at_checked(current_index, instructions_sysvar)
        .map_err(|_| error!(ErrorCode::InvalidInstructionsSysvar))?;

    require!(
        is_expected_same_pair_liquidity_delta(&current_instruction, pair, expected_instruction),
        ErrorCode::LiquidityDeltaCircuitBreakerCpi
    );
    Ok(())
}

pub fn require_no_same_tx_liquidity_delta(
    pair: &Pubkey,
    instructions_sysvar: &AccountInfo,
) -> Result<()> {
    require_no_matching_liquidity_delta(pair, instructions_sysvar, true)
}

pub fn require_no_same_tx_add_liquidity(
    pair: &Pubkey,
    instructions_sysvar: &AccountInfo,
) -> Result<()> {
    require_no_matching_liquidity_delta(pair, instructions_sysvar, false)
}

fn require_no_matching_liquidity_delta(
    pair: &Pubkey,
    instructions_sysvar: &AccountInfo,
    include_remove_liquidity: bool,
) -> Result<()> {
    require_keys_eq!(
        *instructions_sysvar.key,
        sysvar::instructions::ID,
        ErrorCode::InvalidInstructionsSysvar
    );

    let instruction_count = load_instruction_count(instructions_sysvar)?;

    for index in 0..instruction_count {
        let instruction = load_instruction_at_checked(index, instructions_sysvar)
            .map_err(|_| error!(ErrorCode::InvalidInstructionsSysvar))?;

        if is_same_pair_liquidity_delta(&instruction, pair, include_remove_liquidity) {
            return err!(ErrorCode::LiquidityDeltaCircuitBreaker);
        }
    }

    Ok(())
}

fn load_instruction_count(instructions_sysvar: &AccountInfo) -> Result<usize> {
    let data = instructions_sysvar
        .try_borrow_data()
        .map_err(|_| error!(ErrorCode::InvalidInstructionsSysvar))?;
    require!(data.len() >= 2, ErrorCode::InvalidInstructionsSysvar);
    Ok(u16::from_le_bytes([data[0], data[1]]) as usize)
}

fn is_same_pair_liquidity_delta(
    instruction: &Instruction,
    pair: &Pubkey,
    include_remove_liquidity: bool,
) -> bool {
    if instruction.program_id != crate::ID {
        return false;
    }

    let Some(discriminator) = instruction.data.get(..8) else {
        return false;
    };

    let is_liquidity_delta =
        discriminator_matches(discriminator, LiquidityDeltaInstruction::AddLiquidity)
            || (include_remove_liquidity && discriminator == REMOVE_LIQUIDITY_DISCRIMINATOR);
    if !is_liquidity_delta {
        return false;
    }

    instruction
        .accounts
        .first()
        .map(|account| account.pubkey == *pair)
        .unwrap_or(false)
}

fn is_expected_same_pair_liquidity_delta(
    instruction: &Instruction,
    pair: &Pubkey,
    expected_instruction: LiquidityDeltaInstruction,
) -> bool {
    if instruction.program_id != crate::ID {
        return false;
    }

    let Some(discriminator) = instruction.data.get(..8) else {
        return false;
    };

    if !discriminator_matches(discriminator, expected_instruction) {
        return false;
    }

    instruction
        .accounts
        .first()
        .map(|account| account.pubkey == *pair)
        .unwrap_or(false)
}

fn discriminator_matches(
    discriminator: &[u8],
    expected_instruction: LiquidityDeltaInstruction,
) -> bool {
    match expected_instruction {
        LiquidityDeltaInstruction::AddLiquidity => discriminator == ADD_LIQUIDITY_DISCRIMINATOR,
        LiquidityDeltaInstruction::RemoveLiquidity => {
            discriminator == REMOVE_LIQUIDITY_DISCRIMINATOR
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use anchor_lang::solana_program::{
        account_info::AccountInfo,
        hash::hash,
        instruction::AccountMeta,
        sysvar::instructions::{
            construct_instructions_data, store_current_index, BorrowedAccountMeta,
            BorrowedInstruction,
        },
    };

    fn borrowed_instruction<'a>(
        program_id: &'a Pubkey,
        pair: &'a Pubkey,
        data: &'a [u8],
    ) -> BorrowedInstruction<'a> {
        BorrowedInstruction {
            program_id,
            accounts: vec![BorrowedAccountMeta {
                pubkey: pair,
                is_signer: false,
                is_writable: true,
            }],
            data,
        }
    }

    fn unrelated_instruction(program_id: Pubkey) -> Instruction {
        Instruction {
            program_id,
            accounts: vec![AccountMeta::new(Pubkey::new_unique(), false)],
            data: vec![1, 2, 3],
        }
    }

    fn run_guard(
        pair: &Pubkey,
        instructions: &[BorrowedInstruction],
        current_index: u16,
        include_remove_liquidity: bool,
    ) -> Result<()> {
        let key = sysvar::instructions::ID;
        let owner = sysvar::ID;
        let mut lamports = 0;
        let mut data = construct_instructions_data(instructions);
        store_current_index(&mut data, current_index);
        let account_info = AccountInfo::new(
            &key,
            false,
            false,
            &mut lamports,
            &mut data,
            &owner,
            false,
            0,
        );

        require_no_matching_liquidity_delta(pair, &account_info, include_remove_liquidity)
    }

    fn anchor_global_discriminator(name: &str) -> [u8; 8] {
        let digest = hash(format!("global:{name}").as_bytes()).to_bytes();
        digest[..8].try_into().unwrap()
    }

    #[test]
    fn liquidity_delta_discriminators_match_anchor_instruction_names() {
        assert_eq!(
            ADD_LIQUIDITY_DISCRIMINATOR,
            anchor_global_discriminator("add_liquidity")
        );
        assert_eq!(
            REMOVE_LIQUIDITY_DISCRIMINATOR,
            anchor_global_discriminator("remove_liquidity")
        );
    }

    #[test]
    fn liquidity_delta_blocks_same_pair_add_liquidity() {
        let pair = Pubkey::new_unique();
        let current_pair = Pubkey::new_unique();
        let add_ix = borrowed_instruction(&crate::ID, &pair, &ADD_LIQUIDITY_DISCRIMINATOR);
        let current_ix = borrowed_instruction(&crate::ID, &current_pair, &[9; 8]);

        let err = run_guard(&pair, &[add_ix, current_ix], 1, true).unwrap_err();
        assert_eq!(err, error!(ErrorCode::LiquidityDeltaCircuitBreaker));
    }

    #[test]
    fn liquidity_delta_blocks_same_pair_remove_liquidity() {
        let pair = Pubkey::new_unique();
        let current_pair = Pubkey::new_unique();
        let remove_ix = borrowed_instruction(&crate::ID, &pair, &REMOVE_LIQUIDITY_DISCRIMINATOR);
        let current_ix = borrowed_instruction(&crate::ID, &current_pair, &[9; 8]);

        let err = run_guard(&pair, &[remove_ix, current_ix], 1, true).unwrap_err();
        assert_eq!(err, error!(ErrorCode::LiquidityDeltaCircuitBreaker));
    }

    #[test]
    fn liquidity_delta_blocks_current_top_level_add_liquidity() {
        let pair = Pubkey::new_unique();
        let add_ix = borrowed_instruction(&crate::ID, &pair, &ADD_LIQUIDITY_DISCRIMINATOR);

        let err = run_guard(&pair, &[add_ix], 0, true).unwrap_err();
        assert_eq!(err, error!(ErrorCode::LiquidityDeltaCircuitBreaker));
    }

    #[test]
    fn liquidity_delta_blocks_current_top_level_remove_liquidity() {
        let pair = Pubkey::new_unique();
        let remove_ix = borrowed_instruction(&crate::ID, &pair, &REMOVE_LIQUIDITY_DISCRIMINATOR);

        let err = run_guard(&pair, &[remove_ix], 0, true).unwrap_err();
        assert_eq!(err, error!(ErrorCode::LiquidityDeltaCircuitBreaker));
    }

    #[test]
    fn add_only_guard_allows_remove_liquidity() {
        let pair = Pubkey::new_unique();
        let current_pair = Pubkey::new_unique();
        let remove_ix = borrowed_instruction(&crate::ID, &pair, &REMOVE_LIQUIDITY_DISCRIMINATOR);
        let current_ix = borrowed_instruction(&crate::ID, &current_pair, &[9; 8]);

        run_guard(&pair, &[remove_ix, current_ix], 1, false).unwrap();
    }

    #[test]
    fn liquidity_delta_allows_different_pair() {
        let pair = Pubkey::new_unique();
        let other_pair = Pubkey::new_unique();
        let current_pair = Pubkey::new_unique();
        let add_ix = borrowed_instruction(&crate::ID, &other_pair, &ADD_LIQUIDITY_DISCRIMINATOR);
        let current_ix = borrowed_instruction(&crate::ID, &current_pair, &[9; 8]);

        run_guard(&pair, &[add_ix, current_ix], 1, true).unwrap();
    }

    #[test]
    fn liquidity_delta_allows_unrelated_instruction() {
        let pair = Pubkey::new_unique();
        let current_pair = Pubkey::new_unique();
        let unrelated = unrelated_instruction(Pubkey::new_unique());
        let current_ix = borrowed_instruction(&crate::ID, &current_pair, &[9; 8]);
        let borrowed_unrelated = BorrowedInstruction {
            program_id: &unrelated.program_id,
            accounts: unrelated
                .accounts
                .iter()
                .map(|account| BorrowedAccountMeta {
                    pubkey: &account.pubkey,
                    is_signer: account.is_signer,
                    is_writable: account.is_writable,
                })
                .collect(),
            data: &unrelated.data,
        };

        run_guard(&pair, &[borrowed_unrelated, current_ix], 1, true).unwrap();
    }

    #[test]
    fn top_level_liquidity_delta_requires_current_ix_match() {
        let pair = Pubkey::new_unique();
        let other_program = Pubkey::new_unique();
        let add_ix = borrowed_instruction(&crate::ID, &pair, &ADD_LIQUIDITY_DISCRIMINATOR);
        let other_ix = borrowed_instruction(&other_program, &pair, &[9; 8]);
        let key = sysvar::instructions::ID;
        let owner = sysvar::ID;
        let mut lamports = 0;
        let mut data = construct_instructions_data(&[other_ix, add_ix]);
        store_current_index(&mut data, 1);
        let account_info = AccountInfo::new(
            &key,
            false,
            false,
            &mut lamports,
            &mut data,
            &owner,
            false,
            0,
        );

        require_top_level_liquidity_delta_ix(
            &pair,
            &account_info,
            LiquidityDeltaInstruction::AddLiquidity,
        )
        .unwrap();
    }

    #[test]
    fn top_level_liquidity_delta_rejects_wrapper_current_ix() {
        let pair = Pubkey::new_unique();
        let wrapper_program = Pubkey::new_unique();
        let wrapper_ix = borrowed_instruction(&wrapper_program, &pair, &[9; 8]);
        let key = sysvar::instructions::ID;
        let owner = sysvar::ID;
        let mut lamports = 0;
        let mut data = construct_instructions_data(&[wrapper_ix]);
        store_current_index(&mut data, 0);
        let account_info = AccountInfo::new(
            &key,
            false,
            false,
            &mut lamports,
            &mut data,
            &owner,
            false,
            0,
        );

        let err = require_top_level_liquidity_delta_ix(
            &pair,
            &account_info,
            LiquidityDeltaInstruction::AddLiquidity,
        )
        .unwrap_err();
        assert_eq!(err, error!(ErrorCode::LiquidityDeltaCircuitBreakerCpi));
    }

    #[test]
    fn top_level_liquidity_delta_allows_following_sibling_ix() {
        let pair = Pubkey::new_unique();
        let remove_ix = borrowed_instruction(&crate::ID, &pair, &REMOVE_LIQUIDITY_DISCRIMINATOR);
        let following_program = Pubkey::new_unique();
        let following_ix = borrowed_instruction(&following_program, &pair, &[9; 8]);
        let key = sysvar::instructions::ID;
        let owner = sysvar::ID;
        let mut lamports = 0;
        let mut data = construct_instructions_data(&[remove_ix, following_ix]);
        store_current_index(&mut data, 0);
        let account_info = AccountInfo::new(
            &key,
            false,
            false,
            &mut lamports,
            &mut data,
            &owner,
            false,
            0,
        );

        require_top_level_liquidity_delta_ix(
            &pair,
            &account_info,
            LiquidityDeltaInstruction::RemoveLiquidity,
        )
        .unwrap();
    }
}
