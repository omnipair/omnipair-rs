

use carbon_core::{CarbonDeserialize, borsh};


#[derive(CarbonDeserialize, Debug, serde::Serialize, serde::Deserialize, PartialEq, Eq, Clone, Hash)]
pub struct EmitValueArgs {
    pub amount: Option<u64>,
    pub token_mint: Option<solana_pubkey::Pubkey>,
}
