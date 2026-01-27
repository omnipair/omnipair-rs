

use carbon_core::{CarbonDeserialize, borsh};


#[derive(CarbonDeserialize, Debug, serde::Serialize, serde::Deserialize, PartialEq, Eq, Clone, Hash)]
pub struct UpdateFutarchyAuthorityArgs {
    pub new_authority: solana_pubkey::Pubkey,
}
