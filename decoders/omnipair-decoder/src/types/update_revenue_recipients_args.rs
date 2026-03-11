

use carbon_core::{CarbonDeserialize, borsh};


#[derive(CarbonDeserialize, Debug, serde::Serialize, serde::Deserialize, PartialEq, Eq, Clone, Hash)]
pub struct UpdateRevenueRecipientsArgs {
    pub futarchy_treasury: Option<solana_pubkey::Pubkey>,
    pub buybacks_vault: Option<solana_pubkey::Pubkey>,
    pub team_treasury: Option<solana_pubkey::Pubkey>,
}
