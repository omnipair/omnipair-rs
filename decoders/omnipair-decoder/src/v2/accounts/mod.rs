// This V2 decoder code is generated from packages/program-interface/src/idl_v2.json.
use carbon_core::account::AccountDecoder;
use carbon_core::deserialize::CarbonDeserialize;

use super::OmnipairV2Decoder;

pub mod hedge_position;
pub mod margin_position;
pub mod market;
pub mod stake_position;

#[derive(Debug, serde::Serialize, serde::Deserialize, PartialEq, Eq, Clone, Hash)]
pub enum OmnipairV2Account {
    HedgePosition(hedge_position::HedgePosition),
    MarginPosition(margin_position::MarginPosition),
    Market(market::Market),
    StakePosition(stake_position::StakePosition),
}

impl<'a> AccountDecoder<'a> for OmnipairV2Decoder {
    type AccountType = OmnipairV2Account;

    fn decode_account(
        &self,
        account: &solana_account::Account,
    ) -> Option<carbon_core::account::DecodedAccount<Self::AccountType>> {
        if let Some(decoded_account) =
            hedge_position::HedgePosition::deserialize(account.data.as_slice())
        {
            return Some(carbon_core::account::DecodedAccount {
                lamports: account.lamports,
                data: OmnipairV2Account::HedgePosition(decoded_account),
                owner: account.owner,
                executable: account.executable,
                rent_epoch: account.rent_epoch,
            });
        }

        if let Some(decoded_account) =
            margin_position::MarginPosition::deserialize(account.data.as_slice())
        {
            return Some(carbon_core::account::DecodedAccount {
                lamports: account.lamports,
                data: OmnipairV2Account::MarginPosition(decoded_account),
                owner: account.owner,
                executable: account.executable,
                rent_epoch: account.rent_epoch,
            });
        }

        if let Some(decoded_account) = market::Market::deserialize(account.data.as_slice()) {
            return Some(carbon_core::account::DecodedAccount {
                lamports: account.lamports,
                data: OmnipairV2Account::Market(decoded_account),
                owner: account.owner,
                executable: account.executable,
                rent_epoch: account.rent_epoch,
            });
        }

        if let Some(decoded_account) =
            stake_position::StakePosition::deserialize(account.data.as_slice())
        {
            return Some(carbon_core::account::DecodedAccount {
                lamports: account.lamports,
                data: OmnipairV2Account::StakePosition(decoded_account),
                owner: account.owner,
                executable: account.executable,
                rent_epoch: account.rent_epoch,
            });
        }

        None
    }
}
