 
use carbon_core::account::AccountDecoder; 
use carbon_core::deserialize::CarbonDeserialize;
 

use super::OmnipairDecoder; 
pub mod futarchy_authority; 
pub mod pair; 
pub mod rate_model; 
pub mod user_position; 

pub enum OmnipairAccount { 
        FutarchyAuthority(futarchy_authority::FutarchyAuthority), 
        Pair(pair::Pair), 
        RateModel(rate_model::RateModel), 
        UserPosition(user_position::UserPosition), 
}


impl<'a> AccountDecoder<'a> for OmnipairDecoder { 
    type AccountType = OmnipairAccount;
     fn decode_account( &self, account: &solana_account::Account, ) -> Option<carbon_core::account::DecodedAccount<Self::AccountType>> { 
         
            if let Some(decoded_account) = futarchy_authority::FutarchyAuthority::deserialize(account.data.as_slice()) { 
            return Some(carbon_core::account::DecodedAccount { 
                lamports: account.lamports, 
                data: OmnipairAccount::FutarchyAuthority(decoded_account), 
                owner: account.owner, 
                executable: account.executable, 
                rent_epoch: account.rent_epoch, 
            }); 
        } 
         
            if let Some(decoded_account) = pair::Pair::deserialize(account.data.as_slice()) { 
            return Some(carbon_core::account::DecodedAccount { 
                lamports: account.lamports, 
                data: OmnipairAccount::Pair(decoded_account), 
                owner: account.owner, 
                executable: account.executable, 
                rent_epoch: account.rent_epoch, 
            }); 
        } 
         
            if let Some(decoded_account) = rate_model::RateModel::deserialize(account.data.as_slice()) { 
            return Some(carbon_core::account::DecodedAccount { 
                lamports: account.lamports, 
                data: OmnipairAccount::RateModel(decoded_account), 
                owner: account.owner, 
                executable: account.executable, 
                rent_epoch: account.rent_epoch, 
            }); 
        } 
         
            if let Some(decoded_account) = user_position::UserPosition::deserialize(account.data.as_slice()) { 
            return Some(carbon_core::account::DecodedAccount { 
                lamports: account.lamports, 
                data: OmnipairAccount::UserPosition(decoded_account), 
                owner: account.owner, 
                executable: account.executable, 
                rent_epoch: account.rent_epoch, 
            }); 
        } 
         
    None 
    } 
}