// This V2 decoder code is generated from packages/program-interface/src/idl_v2.json.
use carbon_core::deserialize::CarbonDeserialize;

use super::{OmnipairV2Decoder, PROGRAM_ID};

pub mod add_liquidity;
pub mod borrow;
pub mod claim_fees;
pub mod claim_hedge_fees;
pub mod claim_market_fees;
pub mod close_hedge;
pub mod deposit_collateral;
pub mod deposit_insurance;
pub mod initialize;
pub mod liquidate;
pub mod liquidity_added;
pub mod liquidity_removed;
pub mod market_collateral_deposited;
pub mod market_collateral_withdrawn;
pub mod market_created;
pub mod market_debt_updated;
pub mod market_fee_liability_claimed;
pub mod market_fees_claimed;
pub mod market_health_updated;
pub mod market_hedge_closed;
pub mod market_hedge_fees_claimed;
pub mod market_hedge_opened;
pub mod market_insurance_funded;
pub mod market_stake_updated;
pub mod market_updated;
pub mod open_hedge;
pub mod position_liquidated;
pub mod remove_liquidity;
pub mod repay;
pub mod set_reduce_only;
pub mod stake;
pub mod swap;
pub mod swap_executed;
pub mod unstake;
pub mod update_config;
pub mod withdraw_collateral;

#[derive(
    carbon_core::InstructionType,
    serde::Serialize,
    serde::Deserialize,
    PartialEq,
    Eq,
    Debug,
    Clone,
    Hash,
)]
pub enum OmnipairV2Instruction {
    AddLiquidity(add_liquidity::AddLiquidity),
    Borrow(borrow::Borrow),
    ClaimFees(claim_fees::ClaimFees),
    ClaimHedgeFees(claim_hedge_fees::ClaimHedgeFees),
    ClaimMarketFees(claim_market_fees::ClaimMarketFees),
    CloseHedge(close_hedge::CloseHedge),
    DepositCollateral(deposit_collateral::DepositCollateral),
    DepositInsurance(deposit_insurance::DepositInsurance),
    Initialize(initialize::Initialize),
    Liquidate(liquidate::Liquidate),
    OpenHedge(open_hedge::OpenHedge),
    RemoveLiquidity(remove_liquidity::RemoveLiquidity),
    Repay(repay::Repay),
    SetReduceOnly(set_reduce_only::SetReduceOnly),
    Stake(stake::Stake),
    Swap(swap::Swap),
    Unstake(unstake::Unstake),
    UpdateConfig(update_config::UpdateConfig),
    WithdrawCollateral(withdraw_collateral::WithdrawCollateral),
    LiquidityAdded(liquidity_added::LiquidityAdded),
    LiquidityRemoved(liquidity_removed::LiquidityRemoved),
    MarketCollateralDeposited(market_collateral_deposited::MarketCollateralDeposited),
    MarketCollateralWithdrawn(market_collateral_withdrawn::MarketCollateralWithdrawn),
    MarketCreated(market_created::MarketCreated),
    MarketDebtUpdated(market_debt_updated::MarketDebtUpdated),
    MarketFeeLiabilityClaimed(market_fee_liability_claimed::MarketFeeLiabilityClaimed),
    MarketFeesClaimed(market_fees_claimed::MarketFeesClaimed),
    MarketHealthUpdated(market_health_updated::MarketHealthUpdated),
    MarketHedgeClosed(market_hedge_closed::MarketHedgeClosed),
    MarketHedgeFeesClaimed(market_hedge_fees_claimed::MarketHedgeFeesClaimed),
    MarketHedgeOpened(market_hedge_opened::MarketHedgeOpened),
    MarketInsuranceFunded(market_insurance_funded::MarketInsuranceFunded),
    MarketStakeUpdated(market_stake_updated::MarketStakeUpdated),
    MarketUpdated(market_updated::MarketUpdated),
    PositionLiquidated(position_liquidated::PositionLiquidated),
    SwapExecuted(swap_executed::SwapExecuted),
}

impl<'a> carbon_core::instruction::InstructionDecoder<'a> for OmnipairV2Decoder {
    type InstructionType = OmnipairV2Instruction;

    fn decode_instruction(
        &self,
        instruction: &solana_instruction::Instruction,
    ) -> Option<carbon_core::instruction::DecodedInstruction<Self::InstructionType>> {
        if instruction.program_id != PROGRAM_ID {
            return None;
        }

        if let Some(decoded) = add_liquidity::AddLiquidity::deserialize(instruction.data.as_slice())
        {
            return Some(carbon_core::instruction::DecodedInstruction {
                program_id: instruction.program_id,
                accounts: instruction.accounts.clone(),
                data: OmnipairV2Instruction::AddLiquidity(decoded),
            });
        }

        if let Some(decoded) = borrow::Borrow::deserialize(instruction.data.as_slice()) {
            return Some(carbon_core::instruction::DecodedInstruction {
                program_id: instruction.program_id,
                accounts: instruction.accounts.clone(),
                data: OmnipairV2Instruction::Borrow(decoded),
            });
        }

        if let Some(decoded) = claim_fees::ClaimFees::deserialize(instruction.data.as_slice()) {
            return Some(carbon_core::instruction::DecodedInstruction {
                program_id: instruction.program_id,
                accounts: instruction.accounts.clone(),
                data: OmnipairV2Instruction::ClaimFees(decoded),
            });
        }

        if let Some(decoded) =
            claim_hedge_fees::ClaimHedgeFees::deserialize(instruction.data.as_slice())
        {
            return Some(carbon_core::instruction::DecodedInstruction {
                program_id: instruction.program_id,
                accounts: instruction.accounts.clone(),
                data: OmnipairV2Instruction::ClaimHedgeFees(decoded),
            });
        }

        if let Some(decoded) =
            claim_market_fees::ClaimMarketFees::deserialize(instruction.data.as_slice())
        {
            return Some(carbon_core::instruction::DecodedInstruction {
                program_id: instruction.program_id,
                accounts: instruction.accounts.clone(),
                data: OmnipairV2Instruction::ClaimMarketFees(decoded),
            });
        }

        if let Some(decoded) = close_hedge::CloseHedge::deserialize(instruction.data.as_slice()) {
            return Some(carbon_core::instruction::DecodedInstruction {
                program_id: instruction.program_id,
                accounts: instruction.accounts.clone(),
                data: OmnipairV2Instruction::CloseHedge(decoded),
            });
        }

        if let Some(decoded) =
            deposit_collateral::DepositCollateral::deserialize(instruction.data.as_slice())
        {
            return Some(carbon_core::instruction::DecodedInstruction {
                program_id: instruction.program_id,
                accounts: instruction.accounts.clone(),
                data: OmnipairV2Instruction::DepositCollateral(decoded),
            });
        }

        if let Some(decoded) =
            deposit_insurance::DepositInsurance::deserialize(instruction.data.as_slice())
        {
            return Some(carbon_core::instruction::DecodedInstruction {
                program_id: instruction.program_id,
                accounts: instruction.accounts.clone(),
                data: OmnipairV2Instruction::DepositInsurance(decoded),
            });
        }

        if let Some(decoded) = initialize::Initialize::deserialize(instruction.data.as_slice()) {
            return Some(carbon_core::instruction::DecodedInstruction {
                program_id: instruction.program_id,
                accounts: instruction.accounts.clone(),
                data: OmnipairV2Instruction::Initialize(decoded),
            });
        }

        if let Some(decoded) = liquidate::Liquidate::deserialize(instruction.data.as_slice()) {
            return Some(carbon_core::instruction::DecodedInstruction {
                program_id: instruction.program_id,
                accounts: instruction.accounts.clone(),
                data: OmnipairV2Instruction::Liquidate(decoded),
            });
        }

        if let Some(decoded) = open_hedge::OpenHedge::deserialize(instruction.data.as_slice()) {
            return Some(carbon_core::instruction::DecodedInstruction {
                program_id: instruction.program_id,
                accounts: instruction.accounts.clone(),
                data: OmnipairV2Instruction::OpenHedge(decoded),
            });
        }

        if let Some(decoded) =
            remove_liquidity::RemoveLiquidity::deserialize(instruction.data.as_slice())
        {
            return Some(carbon_core::instruction::DecodedInstruction {
                program_id: instruction.program_id,
                accounts: instruction.accounts.clone(),
                data: OmnipairV2Instruction::RemoveLiquidity(decoded),
            });
        }

        if let Some(decoded) = repay::Repay::deserialize(instruction.data.as_slice()) {
            return Some(carbon_core::instruction::DecodedInstruction {
                program_id: instruction.program_id,
                accounts: instruction.accounts.clone(),
                data: OmnipairV2Instruction::Repay(decoded),
            });
        }

        if let Some(decoded) =
            set_reduce_only::SetReduceOnly::deserialize(instruction.data.as_slice())
        {
            return Some(carbon_core::instruction::DecodedInstruction {
                program_id: instruction.program_id,
                accounts: instruction.accounts.clone(),
                data: OmnipairV2Instruction::SetReduceOnly(decoded),
            });
        }

        if let Some(decoded) = stake::Stake::deserialize(instruction.data.as_slice()) {
            return Some(carbon_core::instruction::DecodedInstruction {
                program_id: instruction.program_id,
                accounts: instruction.accounts.clone(),
                data: OmnipairV2Instruction::Stake(decoded),
            });
        }

        if let Some(decoded) = swap::Swap::deserialize(instruction.data.as_slice()) {
            return Some(carbon_core::instruction::DecodedInstruction {
                program_id: instruction.program_id,
                accounts: instruction.accounts.clone(),
                data: OmnipairV2Instruction::Swap(decoded),
            });
        }

        if let Some(decoded) = unstake::Unstake::deserialize(instruction.data.as_slice()) {
            return Some(carbon_core::instruction::DecodedInstruction {
                program_id: instruction.program_id,
                accounts: instruction.accounts.clone(),
                data: OmnipairV2Instruction::Unstake(decoded),
            });
        }

        if let Some(decoded) = update_config::UpdateConfig::deserialize(instruction.data.as_slice())
        {
            return Some(carbon_core::instruction::DecodedInstruction {
                program_id: instruction.program_id,
                accounts: instruction.accounts.clone(),
                data: OmnipairV2Instruction::UpdateConfig(decoded),
            });
        }

        if let Some(decoded) =
            withdraw_collateral::WithdrawCollateral::deserialize(instruction.data.as_slice())
        {
            return Some(carbon_core::instruction::DecodedInstruction {
                program_id: instruction.program_id,
                accounts: instruction.accounts.clone(),
                data: OmnipairV2Instruction::WithdrawCollateral(decoded),
            });
        }

        if let Some(decoded) =
            liquidity_added::LiquidityAdded::deserialize(instruction.data.as_slice())
        {
            return Some(carbon_core::instruction::DecodedInstruction {
                program_id: instruction.program_id,
                accounts: instruction.accounts.clone(),
                data: OmnipairV2Instruction::LiquidityAdded(decoded),
            });
        }

        if let Some(decoded) =
            liquidity_removed::LiquidityRemoved::deserialize(instruction.data.as_slice())
        {
            return Some(carbon_core::instruction::DecodedInstruction {
                program_id: instruction.program_id,
                accounts: instruction.accounts.clone(),
                data: OmnipairV2Instruction::LiquidityRemoved(decoded),
            });
        }

        if let Some(decoded) = market_collateral_deposited::MarketCollateralDeposited::deserialize(
            instruction.data.as_slice(),
        ) {
            return Some(carbon_core::instruction::DecodedInstruction {
                program_id: instruction.program_id,
                accounts: instruction.accounts.clone(),
                data: OmnipairV2Instruction::MarketCollateralDeposited(decoded),
            });
        }

        if let Some(decoded) = market_collateral_withdrawn::MarketCollateralWithdrawn::deserialize(
            instruction.data.as_slice(),
        ) {
            return Some(carbon_core::instruction::DecodedInstruction {
                program_id: instruction.program_id,
                accounts: instruction.accounts.clone(),
                data: OmnipairV2Instruction::MarketCollateralWithdrawn(decoded),
            });
        }

        if let Some(decoded) =
            market_created::MarketCreated::deserialize(instruction.data.as_slice())
        {
            return Some(carbon_core::instruction::DecodedInstruction {
                program_id: instruction.program_id,
                accounts: instruction.accounts.clone(),
                data: OmnipairV2Instruction::MarketCreated(decoded),
            });
        }

        if let Some(decoded) =
            market_debt_updated::MarketDebtUpdated::deserialize(instruction.data.as_slice())
        {
            return Some(carbon_core::instruction::DecodedInstruction {
                program_id: instruction.program_id,
                accounts: instruction.accounts.clone(),
                data: OmnipairV2Instruction::MarketDebtUpdated(decoded),
            });
        }

        if let Some(decoded) = market_fee_liability_claimed::MarketFeeLiabilityClaimed::deserialize(
            instruction.data.as_slice(),
        ) {
            return Some(carbon_core::instruction::DecodedInstruction {
                program_id: instruction.program_id,
                accounts: instruction.accounts.clone(),
                data: OmnipairV2Instruction::MarketFeeLiabilityClaimed(decoded),
            });
        }

        if let Some(decoded) =
            market_fees_claimed::MarketFeesClaimed::deserialize(instruction.data.as_slice())
        {
            return Some(carbon_core::instruction::DecodedInstruction {
                program_id: instruction.program_id,
                accounts: instruction.accounts.clone(),
                data: OmnipairV2Instruction::MarketFeesClaimed(decoded),
            });
        }

        if let Some(decoded) =
            market_health_updated::MarketHealthUpdated::deserialize(instruction.data.as_slice())
        {
            return Some(carbon_core::instruction::DecodedInstruction {
                program_id: instruction.program_id,
                accounts: instruction.accounts.clone(),
                data: OmnipairV2Instruction::MarketHealthUpdated(decoded),
            });
        }

        if let Some(decoded) =
            market_hedge_closed::MarketHedgeClosed::deserialize(instruction.data.as_slice())
        {
            return Some(carbon_core::instruction::DecodedInstruction {
                program_id: instruction.program_id,
                accounts: instruction.accounts.clone(),
                data: OmnipairV2Instruction::MarketHedgeClosed(decoded),
            });
        }

        if let Some(decoded) = market_hedge_fees_claimed::MarketHedgeFeesClaimed::deserialize(
            instruction.data.as_slice(),
        ) {
            return Some(carbon_core::instruction::DecodedInstruction {
                program_id: instruction.program_id,
                accounts: instruction.accounts.clone(),
                data: OmnipairV2Instruction::MarketHedgeFeesClaimed(decoded),
            });
        }

        if let Some(decoded) =
            market_hedge_opened::MarketHedgeOpened::deserialize(instruction.data.as_slice())
        {
            return Some(carbon_core::instruction::DecodedInstruction {
                program_id: instruction.program_id,
                accounts: instruction.accounts.clone(),
                data: OmnipairV2Instruction::MarketHedgeOpened(decoded),
            });
        }

        if let Some(decoded) =
            market_insurance_funded::MarketInsuranceFunded::deserialize(instruction.data.as_slice())
        {
            return Some(carbon_core::instruction::DecodedInstruction {
                program_id: instruction.program_id,
                accounts: instruction.accounts.clone(),
                data: OmnipairV2Instruction::MarketInsuranceFunded(decoded),
            });
        }

        if let Some(decoded) =
            market_stake_updated::MarketStakeUpdated::deserialize(instruction.data.as_slice())
        {
            return Some(carbon_core::instruction::DecodedInstruction {
                program_id: instruction.program_id,
                accounts: instruction.accounts.clone(),
                data: OmnipairV2Instruction::MarketStakeUpdated(decoded),
            });
        }

        if let Some(decoded) =
            market_updated::MarketUpdated::deserialize(instruction.data.as_slice())
        {
            return Some(carbon_core::instruction::DecodedInstruction {
                program_id: instruction.program_id,
                accounts: instruction.accounts.clone(),
                data: OmnipairV2Instruction::MarketUpdated(decoded),
            });
        }

        if let Some(decoded) =
            position_liquidated::PositionLiquidated::deserialize(instruction.data.as_slice())
        {
            return Some(carbon_core::instruction::DecodedInstruction {
                program_id: instruction.program_id,
                accounts: instruction.accounts.clone(),
                data: OmnipairV2Instruction::PositionLiquidated(decoded),
            });
        }

        if let Some(decoded) = swap_executed::SwapExecuted::deserialize(instruction.data.as_slice())
        {
            return Some(carbon_core::instruction::DecodedInstruction {
                program_id: instruction.program_id,
                accounts: instruction.accounts.clone(),
                data: OmnipairV2Instruction::SwapExecuted(decoded),
            });
        }

        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use carbon_core::deserialize::ArrangeAccounts;
    use carbon_core::instruction::InstructionDecoder;
    use solana_instruction::{AccountMeta, Instruction};
    use solana_pubkey::Pubkey;

    #[test]
    fn decodes_and_arranges_v2_swap_instruction() {
        let mut data = vec![0xf8, 0xc6, 0x9e, 0x91, 0xe1, 0x75, 0x87, 0xc8];
        data.push(0);
        data.extend_from_slice(&123_u64.to_le_bytes());
        data.extend_from_slice(&45_u64.to_le_bytes());
        let accounts = (0..13)
            .map(|_| AccountMeta::new(Pubkey::new_unique(), false))
            .collect::<Vec<_>>();
        let instruction = Instruction {
            program_id: PROGRAM_ID,
            accounts: accounts.clone(),
            data,
        };

        let decoded = OmnipairV2Decoder
            .decode_instruction(&instruction)
            .expect("swap should decode");

        match decoded.data {
            OmnipairV2Instruction::Swap(swap) => {
                assert!(matches!(
                    swap.args.asset_in,
                    crate::v2::types::MarketAsset::Base
                ));
                assert_eq!(swap.args.exact_asset_in, 123);
                assert_eq!(swap.args.min_asset_out, 45);
            }
            other => panic!("unexpected instruction: {other:?}"),
        }

        let arranged = swap::Swap::arrange_accounts(&accounts).expect("swap accounts arrange");
        assert_eq!(arranged.market, accounts[0].pubkey);
        assert_eq!(arranged.program, accounts[12].pubkey);
    }
}
