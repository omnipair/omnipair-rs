
 
use carbon_core::{borsh, CarbonDeserialize};

#[derive(
    CarbonDeserialize, Debug, serde::Deserialize, serde::Serialize, PartialEq, Eq, Clone, Hash,
)] 
 

#[carbon(discriminator = "0x5e03cbdb6b8904a2")] 
pub struct RateModel {
        pub exp_rate: u64,
        pub target_util_start: u64,
        pub target_util_end: u64, 
}