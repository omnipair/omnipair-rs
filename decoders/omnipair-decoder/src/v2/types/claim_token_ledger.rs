// This V2 decoder code is generated from packages/program-interface/src/idl_v2.json.
use carbon_core::{borsh, CarbonDeserialize};

#[derive(
    CarbonDeserialize, Debug, serde::Serialize, serde::Deserialize, PartialEq, Eq, Clone, Hash,
)]
pub struct ClaimTokenLedger {
    pub protected_claim_token_supply: u64,
    pub hedged_claim_token_supply: u64,
    pub staked_claim_token_supply: u64,
}
