// This V2 decoder code is generated from packages/program-interface/src/idl_v2.json.
use carbon_core::deserialize::CarbonDeserialize;

use super::{OmnipairV2Decoder, PROGRAM_ID};

pub mod add_liquidity;
pub mod borrow;
pub mod claim_protocol_fees;
pub mod claim_yield;
pub mod close_hedge;
pub mod deposit_collateral;
pub mod hlp_closed;
pub mod hlp_opened;
pub mod hlp_rebalanced;
pub mod init_futarchy_authority;
pub mod initialize;
pub mod liquidate;
pub mod liquidity_added;
pub mod liquidity_removed;
pub mod market_collateral_deposited;
pub mod market_collateral_withdrawn;
pub mod market_created;
pub mod market_debt_updated;
pub mod market_fee_liability_claimed;
pub mod market_health_updated;
pub mod market_updated;
pub mod open_hedge;
pub mod position_liquidated;
pub mod protocol_fees_claimed;
pub mod remove_liquidity;
pub mod repay;
pub mod set_global_reduce_only;
pub mod set_reduce_only;
pub mod set_yield_recipient;
pub mod swap;
pub mod swap_executed;
pub mod swap_settled;
pub mod update_config;
pub mod update_futarchy_authority;
pub mod update_protocol_revenue;
pub mod update_revenue_recipients;
pub mod withdraw_collateral;
pub mod yield_claimed;
pub mod yield_recipient_updated;

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
    ClaimProtocolFees(claim_protocol_fees::ClaimProtocolFees),
    ClaimYield(claim_yield::ClaimYield),
    CloseHedge(close_hedge::CloseHedge),
    DepositCollateral(deposit_collateral::DepositCollateral),
    InitFutarchyAuthority(init_futarchy_authority::InitFutarchyAuthority),
    Initialize(initialize::Initialize),
    Liquidate(liquidate::Liquidate),
    OpenHedge(open_hedge::OpenHedge),
    RemoveLiquidity(remove_liquidity::RemoveLiquidity),
    Repay(repay::Repay),
    SetGlobalReduceOnly(set_global_reduce_only::SetGlobalReduceOnly),
    SetReduceOnly(set_reduce_only::SetReduceOnly),
    SetYieldRecipient(set_yield_recipient::SetYieldRecipient),
    Swap(swap::Swap),
    UpdateConfig(update_config::UpdateConfig),
    UpdateFutarchyAuthority(update_futarchy_authority::UpdateFutarchyAuthority),
    UpdateProtocolRevenue(update_protocol_revenue::UpdateProtocolRevenue),
    UpdateRevenueRecipients(update_revenue_recipients::UpdateRevenueRecipients),
    WithdrawCollateral(withdraw_collateral::WithdrawCollateral),
    HlpClosed(hlp_closed::HlpClosed),
    HlpOpened(hlp_opened::HlpOpened),
    HlpRebalanced(hlp_rebalanced::HlpRebalanced),
    LiquidityAdded(liquidity_added::LiquidityAdded),
    LiquidityRemoved(liquidity_removed::LiquidityRemoved),
    MarketCollateralDeposited(market_collateral_deposited::MarketCollateralDeposited),
    MarketCollateralWithdrawn(market_collateral_withdrawn::MarketCollateralWithdrawn),
    MarketCreated(market_created::MarketCreated),
    MarketDebtUpdated(market_debt_updated::MarketDebtUpdated),
    MarketFeeLiabilityClaimed(market_fee_liability_claimed::MarketFeeLiabilityClaimed),
    MarketHealthUpdated(market_health_updated::MarketHealthUpdated),
    MarketUpdated(market_updated::MarketUpdated),
    PositionLiquidated(position_liquidated::PositionLiquidated),
    ProtocolFeesClaimed(protocol_fees_claimed::ProtocolFeesClaimed),
    SwapExecuted(swap_executed::SwapExecuted),
    SwapSettled(swap_settled::SwapSettled),
    YieldClaimed(yield_claimed::YieldClaimed),
    YieldRecipientUpdated(yield_recipient_updated::YieldRecipientUpdated),
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

        if let Some(decoded) =
            claim_protocol_fees::ClaimProtocolFees::deserialize(instruction.data.as_slice())
        {
            return Some(carbon_core::instruction::DecodedInstruction {
                program_id: instruction.program_id,
                accounts: instruction.accounts.clone(),
                data: OmnipairV2Instruction::ClaimProtocolFees(decoded),
            });
        }

        if let Some(decoded) = claim_yield::ClaimYield::deserialize(instruction.data.as_slice()) {
            return Some(carbon_core::instruction::DecodedInstruction {
                program_id: instruction.program_id,
                accounts: instruction.accounts.clone(),
                data: OmnipairV2Instruction::ClaimYield(decoded),
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
            init_futarchy_authority::InitFutarchyAuthority::deserialize(instruction.data.as_slice())
        {
            return Some(carbon_core::instruction::DecodedInstruction {
                program_id: instruction.program_id,
                accounts: instruction.accounts.clone(),
                data: OmnipairV2Instruction::InitFutarchyAuthority(decoded),
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
            set_global_reduce_only::SetGlobalReduceOnly::deserialize(instruction.data.as_slice())
        {
            return Some(carbon_core::instruction::DecodedInstruction {
                program_id: instruction.program_id,
                accounts: instruction.accounts.clone(),
                data: OmnipairV2Instruction::SetGlobalReduceOnly(decoded),
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

        if let Some(decoded) =
            set_yield_recipient::SetYieldRecipient::deserialize(instruction.data.as_slice())
        {
            return Some(carbon_core::instruction::DecodedInstruction {
                program_id: instruction.program_id,
                accounts: instruction.accounts.clone(),
                data: OmnipairV2Instruction::SetYieldRecipient(decoded),
            });
        }

        if let Some(decoded) = swap::Swap::deserialize(instruction.data.as_slice()) {
            return Some(carbon_core::instruction::DecodedInstruction {
                program_id: instruction.program_id,
                accounts: instruction.accounts.clone(),
                data: OmnipairV2Instruction::Swap(decoded),
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

        if let Some(decoded) = update_futarchy_authority::UpdateFutarchyAuthority::deserialize(
            instruction.data.as_slice(),
        ) {
            return Some(carbon_core::instruction::DecodedInstruction {
                program_id: instruction.program_id,
                accounts: instruction.accounts.clone(),
                data: OmnipairV2Instruction::UpdateFutarchyAuthority(decoded),
            });
        }

        if let Some(decoded) =
            update_protocol_revenue::UpdateProtocolRevenue::deserialize(instruction.data.as_slice())
        {
            return Some(carbon_core::instruction::DecodedInstruction {
                program_id: instruction.program_id,
                accounts: instruction.accounts.clone(),
                data: OmnipairV2Instruction::UpdateProtocolRevenue(decoded),
            });
        }

        if let Some(decoded) = update_revenue_recipients::UpdateRevenueRecipients::deserialize(
            instruction.data.as_slice(),
        ) {
            return Some(carbon_core::instruction::DecodedInstruction {
                program_id: instruction.program_id,
                accounts: instruction.accounts.clone(),
                data: OmnipairV2Instruction::UpdateRevenueRecipients(decoded),
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

        if let Some(decoded) = hlp_closed::HlpClosed::deserialize(instruction.data.as_slice()) {
            return Some(carbon_core::instruction::DecodedInstruction {
                program_id: instruction.program_id,
                accounts: instruction.accounts.clone(),
                data: OmnipairV2Instruction::HlpClosed(decoded),
            });
        }

        if let Some(decoded) = hlp_opened::HlpOpened::deserialize(instruction.data.as_slice()) {
            return Some(carbon_core::instruction::DecodedInstruction {
                program_id: instruction.program_id,
                accounts: instruction.accounts.clone(),
                data: OmnipairV2Instruction::HlpOpened(decoded),
            });
        }

        if let Some(decoded) =
            hlp_rebalanced::HlpRebalanced::deserialize(instruction.data.as_slice())
        {
            return Some(carbon_core::instruction::DecodedInstruction {
                program_id: instruction.program_id,
                accounts: instruction.accounts.clone(),
                data: OmnipairV2Instruction::HlpRebalanced(decoded),
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
            market_health_updated::MarketHealthUpdated::deserialize(instruction.data.as_slice())
        {
            return Some(carbon_core::instruction::DecodedInstruction {
                program_id: instruction.program_id,
                accounts: instruction.accounts.clone(),
                data: OmnipairV2Instruction::MarketHealthUpdated(decoded),
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

        if let Some(decoded) =
            protocol_fees_claimed::ProtocolFeesClaimed::deserialize(instruction.data.as_slice())
        {
            return Some(carbon_core::instruction::DecodedInstruction {
                program_id: instruction.program_id,
                accounts: instruction.accounts.clone(),
                data: OmnipairV2Instruction::ProtocolFeesClaimed(decoded),
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

        if let Some(decoded) = swap_settled::SwapSettled::deserialize(instruction.data.as_slice()) {
            return Some(carbon_core::instruction::DecodedInstruction {
                program_id: instruction.program_id,
                accounts: instruction.accounts.clone(),
                data: OmnipairV2Instruction::SwapSettled(decoded),
            });
        }

        if let Some(decoded) = yield_claimed::YieldClaimed::deserialize(instruction.data.as_slice())
        {
            return Some(carbon_core::instruction::DecodedInstruction {
                program_id: instruction.program_id,
                accounts: instruction.accounts.clone(),
                data: OmnipairV2Instruction::YieldClaimed(decoded),
            });
        }

        if let Some(decoded) =
            yield_recipient_updated::YieldRecipientUpdated::deserialize(instruction.data.as_slice())
        {
            return Some(carbon_core::instruction::DecodedInstruction {
                program_id: instruction.program_id,
                accounts: instruction.accounts.clone(),
                data: OmnipairV2Instruction::YieldRecipientUpdated(decoded),
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
        let accounts = (0..14)
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
        assert_eq!(arranged.program, accounts[13].pubkey);
    }
}
