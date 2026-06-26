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
