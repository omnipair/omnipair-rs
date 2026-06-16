// This V2 decoder code is generated from packages/program-interface/src/idl_v2.json.
use carbon_core::{borsh, CarbonDeserialize};

#[derive(
    CarbonDeserialize, Debug, serde::Serialize, serde::Deserialize, PartialEq, Eq, Clone, Hash,
)]
pub struct BufferLedger {
    pub buffer_share_supply: u64,
    pub staked_buffer_share_amount: u64,
    pub required_buffer: u64,
    pub buffer_ratio_bps: u16,
}
