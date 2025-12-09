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

    pub collateral_vault: AddressStorage,

    pub user_collateral_token_account: AddressStorage,

    pub collateral_token_mint: AddressStorage,

    pub token0_vault: AddressStorage,

    pub token1_vault: AddressStorage,

    pub token0_vault_mint: AddressStorage,

    pub token1_vault_mint: AddressStorage,

    pub lp_mint: AddressStorage,

    pub user_lp_token_account: AddressStorage,

    pub token_vault: AddressStorage,

    pub user_token_account: AddressStorage,

    pub vault_token_mint: AddressStorage,

    pub caller: AddressStorage,

    pub authority_token0_account: AddressStorage,

    pub authority_token1_account: AddressStorage,

    pub source_mint: AddressStorage,

    pub source_token_account: AddressStorage,

    pub futarchy_treasury_token_account: AddressStorage,

    pub buybacks_vault_token_account: AddressStorage,

    pub team_treasury_token_account: AddressStorage,

    pub receiver_token0_account: AddressStorage,

    pub receiver_token1_account: AddressStorage,

    pub deployer_token0_account: AddressStorage,

    pub deployer_token1_account: AddressStorage,

    pub authority_wsol_account: AddressStorage,

    pub caller_token_account: AddressStorage,

    pub position_owner: AddressStorage,

    pub token_in_vault: AddressStorage,

    pub token_out_vault: AddressStorage,

    pub user_token_in_account: AddressStorage,

    pub user_token_out_account: AddressStorage,

    pub token_in_mint: AddressStorage,

    pub token_out_mint: AddressStorage,

    pub authority_token_in_account: AddressStorage,
}
