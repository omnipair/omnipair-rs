use anchor_lang::prelude::*;

use crate::{constants::*, errors::ErrorCode};

#[derive(Clone, Debug, Default, PartialEq, Eq, AnchorSerialize, AnchorDeserialize, InitSpace)]
pub struct RevenueShare {
    pub swap_bps: u16,
    pub interest_bps: u16,
}

/// Revenue recipient wallet addresses. Recipient token accounts are derived or
/// validated against these owners when protocol fees are claimed.
#[derive(Clone, Debug, Default, PartialEq, Eq, AnchorSerialize, AnchorDeserialize, InitSpace)]
pub struct RevenueRecipients {
    pub futarchy_treasury: Pubkey,
    pub buybacks_vault: Pubkey,
    pub team_treasury: Pubkey,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, AnchorSerialize, AnchorDeserialize, InitSpace)]
pub struct RevenueDistribution {
    pub futarchy_treasury_bps: u16,
    pub buybacks_vault_bps: u16,
    pub team_treasury_bps: u16,
}

impl RevenueDistribution {
    pub fn is_valid(&self) -> bool {
        self.futarchy_treasury_bps
            .saturating_add(self.buybacks_vault_bps)
            .saturating_add(self.team_treasury_bps)
            == BPS_DENOMINATOR
    }
}

#[account]
#[derive(Debug, InitSpace)]
pub struct FutarchyAuthority {
    pub version: u8,
    pub authority: Pubkey,
    pub recipients: RevenueRecipients,
    pub revenue_share: RevenueShare,
    pub revenue_distribution: RevenueDistribution,
    pub global_reduce_only: bool,
    pub bump: u8,
}

impl FutarchyAuthority {
    pub const CURRENT_VERSION: u8 = 1;

    pub fn validate(&self) -> Result<()> {
        require!(
            self.revenue_distribution.is_valid(),
            ErrorCode::InvalidDistribution
        );
        Ok(())
    }

    pub fn is_reduce_only(&self, market_reduce_only: bool) -> bool {
        self.global_reduce_only || market_reduce_only
    }

    #[allow(clippy::too_many_arguments)]
    pub fn initialize(
        authority: Pubkey,
        swap_bps: u16,
        interest_bps: u16,
        futarchy_treasury: Pubkey,
        buybacks_vault: Pubkey,
        team_treasury: Pubkey,
        futarchy_treasury_bps: u16,
        buybacks_vault_bps: u16,
        team_treasury_bps: u16,
        bump: u8,
    ) -> Result<Self> {
        let revenue_distribution = RevenueDistribution {
            futarchy_treasury_bps,
            buybacks_vault_bps,
            team_treasury_bps,
        };
        require!(
            revenue_distribution.is_valid(),
            ErrorCode::InvalidDistribution
        );

        Ok(Self {
            version: Self::CURRENT_VERSION,
            authority,
            recipients: RevenueRecipients {
                futarchy_treasury,
                buybacks_vault,
                team_treasury,
            },
            revenue_share: RevenueShare {
                swap_bps,
                interest_bps,
            },
            revenue_distribution,
            global_reduce_only: false,
            bump,
        })
    }
}

#[macro_export]
macro_rules! generate_futarchy_authority_seeds {
    ($futarchy_authority:expr) => {
        [FUTARCHY_AUTHORITY_SEED_PREFIX, &[$futarchy_authority.bump]]
    };
}
