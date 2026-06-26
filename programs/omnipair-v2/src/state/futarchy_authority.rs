use anchor_lang::prelude::*;

use crate::{constants::*, errors::ErrorCode};

#[derive(Clone, Debug, Default, PartialEq, Eq, AnchorSerialize, AnchorDeserialize, InitSpace)]
pub struct RevenueShare {
    pub swap_bps: u16,
    pub interest_bps: u16,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, AnchorSerialize, AnchorDeserialize, InitSpace)]
pub enum ProtocolAuctionLane {
    Fee,
    Buyback,
}

impl Default for ProtocolAuctionLane {
    fn default() -> Self {
        Self::Fee
    }
}

impl ProtocolAuctionLane {
    pub fn code(self) -> u8 {
        match self {
            Self::Fee => 0,
            Self::Buyback => 1,
        }
    }
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

#[derive(Clone, Copy, Debug, PartialEq, Eq, AnchorSerialize, AnchorDeserialize, InitSpace)]
pub struct ProtocolAuctionSplit {
    pub fee_auction_bps: u16,
    pub buyback_auction_bps: u16,
}

impl Default for ProtocolAuctionSplit {
    fn default() -> Self {
        Self {
            fee_auction_bps: BPS_DENOMINATOR,
            buyback_auction_bps: 0,
        }
    }
}

impl ProtocolAuctionSplit {
    pub fn is_valid(&self) -> bool {
        self.fee_auction_bps
            .saturating_add(self.buyback_auction_bps)
            == BPS_DENOMINATOR
    }
}

#[derive(
    Clone, Copy, Debug, Default, PartialEq, Eq, AnchorSerialize, AnchorDeserialize, InitSpace,
)]
pub struct ProtocolAuctionParams {
    pub start_multiplier_bps: u16,
    pub floor_multiplier_bps: u16,
    pub duration_slots: u64,
    pub max_reference_age_slots: u64,
}

impl ProtocolAuctionParams {
    pub fn default_epoch() -> Self {
        Self {
            start_multiplier_bps: 12_000,
            floor_multiplier_bps: 8_000,
            duration_slots: 216_000,
            max_reference_age_slots: 21_600,
        }
    }

    pub fn validate(&self) -> Result<()> {
        require!(
            self.start_multiplier_bps > 0,
            ErrorCode::InvalidAuctionConfig
        );
        require!(
            self.floor_multiplier_bps > 0,
            ErrorCode::InvalidAuctionConfig
        );
        require_gte!(
            self.start_multiplier_bps,
            self.floor_multiplier_bps,
            ErrorCode::InvalidAuctionConfig
        );
        require!(self.duration_slots > 0, ErrorCode::InvalidAuctionConfig);
        require!(
            self.max_reference_age_slots > 0,
            ErrorCode::InvalidAuctionConfig
        );
        Ok(())
    }
}

#[derive(
    Clone, Copy, Debug, Default, PartialEq, Eq, AnchorSerialize, AnchorDeserialize, InitSpace,
)]
pub struct ProtocolAuctionRecipients {
    pub treasury: Pubkey,
    pub staking_vault: Pubkey,
    pub treasury_bps: u16,
    pub staking_vault_bps: u16,
}

impl ProtocolAuctionRecipients {
    pub fn treasury_only(treasury: Pubkey, staking_vault: Pubkey) -> Self {
        Self {
            treasury,
            staking_vault,
            treasury_bps: BPS_DENOMINATOR,
            staking_vault_bps: 0,
        }
    }

    pub fn is_valid(&self) -> bool {
        self.treasury_bps.saturating_add(self.staking_vault_bps) == BPS_DENOMINATOR
    }
}

#[derive(
    Clone, Copy, Debug, Default, PartialEq, Eq, AnchorSerialize, AnchorDeserialize, InitSpace,
)]
pub struct ProtocolAuctionConfig {
    pub accepted_mint: Pubkey,
    pub recipients: ProtocolAuctionRecipients,
    pub params: ProtocolAuctionParams,
    pub last_settlement_slot: u64,
    pub last_settlement_price_nad: u64,
}

impl ProtocolAuctionConfig {
    pub fn initialize(
        accepted_mint: Pubkey,
        treasury: Pubkey,
        staking_vault: Pubkey,
        current_slot: u64,
    ) -> Result<Self> {
        let params = ProtocolAuctionParams::default_epoch();
        params.validate()?;
        Ok(Self {
            accepted_mint,
            recipients: ProtocolAuctionRecipients::treasury_only(treasury, staking_vault),
            params,
            last_settlement_slot: current_slot,
            last_settlement_price_nad: 0,
        })
    }

    pub fn validate(&self) -> Result<()> {
        require_keys_neq!(
            self.accepted_mint,
            Pubkey::default(),
            ErrorCode::InvalidMint
        );
        require!(self.recipients.is_valid(), ErrorCode::InvalidDistribution);
        self.params.validate()
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
    pub protocol_auction_split: ProtocolAuctionSplit,
    pub fee_auction: ProtocolAuctionConfig,
    pub buyback_auction: ProtocolAuctionConfig,
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
        require!(
            self.protocol_auction_split.is_valid(),
            ErrorCode::InvalidDistribution
        );
        self.fee_auction.validate()?;
        self.buyback_auction.validate()?;
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
        staking_vault: Pubkey,
        fee_auction_accepted_mint: Pubkey,
        buyback_auction_accepted_mint: Pubkey,
        futarchy_treasury_bps: u16,
        buybacks_vault_bps: u16,
        team_treasury_bps: u16,
        current_slot: u64,
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
            protocol_auction_split: ProtocolAuctionSplit::default(),
            fee_auction: ProtocolAuctionConfig::initialize(
                fee_auction_accepted_mint,
                futarchy_treasury,
                staking_vault,
                current_slot,
            )?,
            buyback_auction: ProtocolAuctionConfig::initialize(
                buyback_auction_accepted_mint,
                futarchy_treasury,
                staking_vault,
                current_slot,
            )?,
            global_reduce_only: false,
            bump,
        })
    }

    pub fn auction_config(&self, lane: ProtocolAuctionLane) -> &ProtocolAuctionConfig {
        match lane {
            ProtocolAuctionLane::Fee => &self.fee_auction,
            ProtocolAuctionLane::Buyback => &self.buyback_auction,
        }
    }

    pub fn auction_config_mut(&mut self, lane: ProtocolAuctionLane) -> &mut ProtocolAuctionConfig {
        match lane {
            ProtocolAuctionLane::Fee => &mut self.fee_auction,
            ProtocolAuctionLane::Buyback => &mut self.buyback_auction,
        }
    }
}

#[macro_export]
macro_rules! generate_futarchy_authority_seeds {
    ($futarchy_authority:expr) => {
        [FUTARCHY_AUTHORITY_SEED_PREFIX, &[$futarchy_authority.bump]]
    };
}

#[cfg(test)]
mod tests {
    include!("../tests/state/futarchy_authority.rs");
}
