// This V2 decoder code is generated from packages/program-interface/src/idl_v2.json.
use carbon_core::{borsh, CarbonDeserialize};

#[derive(
    CarbonDeserialize, Debug, serde::Serialize, serde::Deserialize, PartialEq, Eq, Clone, Hash,
)]
pub struct UpdateFutarchyAuthorityArgs {
    pub new_authority: solana_pubkey::Pubkey,
}
