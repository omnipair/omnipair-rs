use anchor_lang::prelude::*;

use crate::{errors::ErrorCode, math::decayed_daily_bucket};

#[derive(AnchorSerialize, AnchorDeserialize, Clone, Copy, Default, InitSpace)]
pub struct DailyLimits {
    pub borrowed_bucket: u64,
    pub withdrawn_bucket: u64,
    pub last_decay_slot: u64,
}

impl DailyLimits {
    pub fn decay_to_slot(&mut self, current_slot: u64) -> Result<()> {
        self.borrowed_bucket =
            decayed_daily_bucket(self.borrowed_bucket, self.last_decay_slot, current_slot)?;
        self.withdrawn_bucket =
            decayed_daily_bucket(self.withdrawn_bucket, self.last_decay_slot, current_slot)?;
        self.last_decay_slot = current_slot;
        Ok(())
    }

    pub fn record_borrow(&mut self, amount: u64, limit: u64, current_slot: u64) -> Result<()> {
        self.decay_to_slot(current_slot)?;
        let next_bucket = self
            .borrowed_bucket
            .checked_add(amount)
            .ok_or(ErrorCode::MarketMathOverflow)?;
        require_gte!(limit, next_bucket, ErrorCode::DailyLimitExceeded);
        self.borrowed_bucket = next_bucket;
        Ok(())
    }

    pub fn record_withdraw(&mut self, amount: u64, limit: u64, current_slot: u64) -> Result<()> {
        self.decay_to_slot(current_slot)?;
        let next_bucket = self
            .withdrawn_bucket
            .checked_add(amount)
            .ok_or(ErrorCode::MarketMathOverflow)?;
        require_gte!(limit, next_bucket, ErrorCode::DailyLimitExceeded);
        self.withdrawn_bucket = next_bucket;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::constants::{MS_PER_DAY, TARGET_MS_PER_SLOT};

    #[test]
    fn daily_limit_bucket_decays_over_one_day() {
        let mut limits = DailyLimits {
            borrowed_bucket: 100_000,
            withdrawn_bucket: 50_000,
            last_decay_slot: 0,
        };
        let half_day_slots = MS_PER_DAY / TARGET_MS_PER_SLOT / 2;

        limits.decay_to_slot(half_day_slots).unwrap();

        assert_eq!(limits.borrowed_bucket, 50_000);
        assert_eq!(limits.withdrawn_bucket, 25_000);
    }
}
