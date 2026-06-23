// This V2 decoder code is generated from packages/program-interface/src/idl_v2.json.
use super::*;

use carbon_core::{borsh, CarbonDeserialize};

#[derive(
    CarbonDeserialize, Debug, serde::Serialize, serde::Deserialize, PartialEq, Eq, Clone, Hash,
)]
pub struct UpdateProtocolRevenueArgs {
    pub swap_bps: Option<u16>,
    pub interest_bps: Option<u16>,
    pub revenue_distribution: Option<RevenueDistribution>,
}
