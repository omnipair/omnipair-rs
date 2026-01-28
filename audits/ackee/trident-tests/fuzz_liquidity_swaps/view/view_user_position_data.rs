use trident_fuzz::trident::Trident;

use crate::{
    types::{
        omnipair::{
            self, ViewUserPositionDataInstruction, ViewUserPositionDataInstructionAccounts,
            ViewUserPositionDataInstructionData,
        },
        Pair, UserPosition, UserPositionViewKind,
    },
    utils::POSITION_SEED_PREFIX,
    FuzzTest,
};

impl FuzzTest {
    pub fn view_user_position_data(&mut self) {
        if let Some(accounts) = self.get_accounts_view_user_position_data() {
            let data = self.get_data_view_user_position_data();
            let ix = ViewUserPositionDataInstruction::data(data)
                .accounts(accounts.clone())
                .instruction();

            let res = self
                .trident
                .process_transaction(&[ix], Some("View User Position Data"));

            assert!(res.is_success());

            // INVARIANT 1: the user_position must belong to the passed pair
            let up_account = self
                .trident
                .get_account_with_type::<UserPosition>(&accounts.user_position, 8)
                .unwrap();
            assert_eq!(
                up_account.pair, accounts.pair,
                "ViewUserPositionData accepted a user_position that does not belong to the given pair"
            );
            // INVARIANT 2: the passed rate_model must match the pair's configured rate_model
            let pair_account = self
                .trident
                .get_account_with_type::<Pair>(&accounts.pair, 8)
                .unwrap();
            assert_eq!(
                pair_account.rate_model, accounts.rate_model,
                "ViewPairData accepted a mismatched rate_model for the given pair"
            );
        }
    }

    fn get_data_view_user_position_data(&mut self) -> ViewUserPositionDataInstructionData {
        let getter = UserPositionViewKind::random(&mut self.trident);
        ViewUserPositionDataInstructionData::new(getter)
    }

    fn get_accounts_view_user_position_data(
        &mut self,
    ) -> Option<ViewUserPositionDataInstructionAccounts> {
        let pair = self.fuzz_accounts.pair.get(&mut self.trident).expect("Pair should exist");
        let user = self.fuzz_accounts.user.get(&mut self.trident).expect("User should exist");
        let user_position = self
            .trident
            .find_program_address(
                &[POSITION_SEED_PREFIX, pair.as_ref(), user.as_ref()],
                &omnipair::program_id(),
            )
            .0;

        self.trident
            .get_account_with_type::<UserPosition>(&user_position, 8)?;

        let rate_model = self.fuzz_accounts.rate_model.get(&mut self.trident).expect("Rate model should exist");
        let futarchy_authority = self.fuzz_accounts.futarchy_authority.get(&mut self.trident).expect("Futarchy authority should exist");

        Some(ViewUserPositionDataInstructionAccounts::new(
            pair,
            user_position,
            rate_model,
            futarchy_authority,
        ))
    }
}

impl UserPositionViewKind {
    pub fn random(trident: &mut Trident) -> Self {
        match trident.random_from_range(0..=5) {
            0 => Self::UserBorrowingPower,
            1 => Self::UserAppliedCollateralFactorBps,
            2 => Self::UserLiquidationCollateralFactorBps,
            3 => Self::UserDebtUtilizationBps,
            4 => Self::UserLiquidationPrice,
            5 => Self::UserDebtWithInterest,
            _ => unreachable!(),
        }
    }
}
