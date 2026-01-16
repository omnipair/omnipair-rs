use trident_fuzz::fuzzing::*;

use crate::{
    types::{
        omnipair::{
            ViewPairDataInstruction, ViewPairDataInstructionAccounts, ViewPairDataInstructionData,
        },
        EmitValueArgs, Pair, PairViewKind,
    },
    FuzzTest,
};

impl FuzzTest {
    pub fn view_pair_data(&mut self) {
        if self.fuzz_accounts.pair.is_empty() {
            return;
        }
        let data = self.get_data_view_pair();
        let accounts = self.get_accounts_view_pair();

        let ix = ViewPairDataInstruction::data(data)
            .accounts(accounts.clone())
            .instruction();

        let res = self
            .trident
            .process_transaction(&[ix], Some("View Pair Data"));

        assert!(res.is_success());

        // INVARIANT 1: the passed rate_model must match the pair's configured rate_model
        let pair_account = self
            .trident
            .get_account_with_type::<Pair>(&accounts.pair, 8)
            .unwrap();
        assert_eq!(
            pair_account.rate_model, accounts.rate_model,
            "ViewPairData accepted a mismatched rate_model for the given pair"
        );
    }

    fn get_data_view_pair(&mut self) -> ViewPairDataInstructionData {
        let getter = PairViewKind::random(&mut self.trident);
        let collateral_amount = if getter == PairViewKind::GetBorrowLimitAndCfBpsForCollateral {
            Some(self.trident.random_from_range(100..=100_000_000))
        } else {
            None
        };
        // Any token mint for collateral token
        let collateral_token = if getter == PairViewKind::GetBorrowLimitAndCfBpsForCollateral {
            Some(self.fuzz_accounts.token_mint.get(&mut self.trident).expect("Token mint should exist"))
        } else {
            None
        };

        ViewPairDataInstructionData::new(
            getter,
            EmitValueArgs::new(
                None, // unused
                collateral_amount,
                collateral_token,
            ),
        )
    }

    fn get_accounts_view_pair(&mut self) -> ViewPairDataInstructionAccounts {
        let pair = self.fuzz_accounts.pair.get(&mut self.trident).expect("Pair should exist");
        let rate_model = self.fuzz_accounts.rate_model.get(&mut self.trident).expect("Rate model should exist");
        let futarchy_authority = self.fuzz_accounts.futarchy_authority.get(&mut self.trident).expect("Futarchy authority should exist");

        ViewPairDataInstructionAccounts::new(pair, rate_model, futarchy_authority)
    }
}

impl PairViewKind {
    pub fn random(trident: &mut Trident) -> Self {
        match trident.random_from_range(0..=6) {
            0 => Self::EmaPrice0Nad,
            1 => Self::EmaPrice1Nad,
            2 => Self::SpotPrice0Nad,
            3 => Self::SpotPrice1Nad,
            4 => Self::K,
            5 => Self::GetRates,
            6 => Self::GetBorrowLimitAndCfBpsForCollateral,
            _ => unreachable!(),
        }
    }
}
