// This V2 decoder code is generated from packages/program-interface/src/idl_v2.json.
use carbon_core::{borsh, CarbonDeserialize};

#[derive(
    CarbonDeserialize, Debug, serde::Serialize, serde::Deserialize, PartialEq, Eq, Clone, Hash,
)]
pub struct RevenueRecipients {
    pub futarchy_treasury: solana_pubkey::Pubkey,
    pub buybacks_vault: solana_pubkey::Pubkey,
    pub team_treasury: solana_pubkey::Pubkey,
}
