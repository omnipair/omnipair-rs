use trident_fuzz::fuzzing::*;

/// Storage for all account addresses used in fuzz testing.
///
/// This struct serves as a centralized repository for account addresses,
/// enabling their reuse across different instruction flows and test scenarios.
///
/// Docs: https://ackee.xyz/trident/docs/latest/trident-api-macro/trident-types/fuzz-accounts/
#[derive(Default)]
pub struct AccountAddresses {
    pub user: AddressStorage,

    pub token_mint: AddressStorage,

    pub pair: AddressStorage,

    pub rate_model: AddressStorage,

    pub futarchy_authority: AddressStorage,

    pub user_position: AddressStorage,

    pub lp_mint: AddressStorage,

    pub caller: AddressStorage,

    pub futarchy_treasury_token_account: AddressStorage,

    pub buybacks_vault_token_account: AddressStorage,

    pub team_treasury_token_account: AddressStorage,

    pub authority_wsol_account: AddressStorage,
}
