#[cfg(test)]
mod metronome_constants {
    use lib::metronome::{
        DEFAULT_BATCH_SIZE, DEFAULT_CHANNEL_CAPACITY, DEFAULT_DEV_PHASES_PER_CYCLE, DEFAULT_HASHES_PER_REV, DEFAULT_HASHES_PER_SECOND, DEFAULT_MS_PER_PHASE,
        DEFAULT_MS_PER_REV, DEFAULT_NUM_CONSECUTIVE_LEADER_PHASES, DEFAULT_PHASES_PER_CYCLE, DEFAULT_REVS_PER_DAY, DEFAULT_REVS_PER_PHASE, DEFAULT_REVS_PER_SECOND,
        DEFAULT_S_PER_PHASE, DEFAULT_SECONDS_PER_DAY, DEFAULT_SPINLOCK_THRESHOLD_US, DEFAULT_US_PER_REV, DEFAULT_US_TOLERANCE_PER_REV,
    };

    #[test]
    fn seconds_per_day() {
        assert_eq!(DEFAULT_SECONDS_PER_DAY, 24 * 60 * 60, "DEFAULT_SECONDS_PER_DAY should be 86400.");
    }

    #[test]
    fn revs_per_second() {
        assert_eq!(DEFAULT_REVS_PER_SECOND, 160, "DEFAULT_REVS_PER_SECOND should be 160.");
    }

    #[test]
    fn revs_per_day() {
        assert_eq!(
            DEFAULT_REVS_PER_DAY,
            DEFAULT_REVS_PER_SECOND * DEFAULT_SECONDS_PER_DAY,
            "DEFAULT_REVS_PER_DAY should be DEFAULT_REVS_PER_SECOND * DEFAULT_SECONDS_PER_DAY."
        );
        assert_eq!(DEFAULT_REVS_PER_DAY, 13_824_000, "DEFAULT_REVS_PER_DAY should be 13,824,000.");
    }

    #[test]
    fn ms_per_rev() {
        assert_eq!(
            DEFAULT_MS_PER_REV,
            1_000 / DEFAULT_REVS_PER_SECOND,
            "DEFAULT_MS_PER_REV should be 1000 / DEFAULT_REVS_PER_SECOND."
        );
        assert_eq!(DEFAULT_MS_PER_REV, 6, "DEFAULT_MS_PER_REV should be 6.");
    }

    #[test]
    fn us_tolerance_per_rev() {
        assert_eq!(DEFAULT_US_TOLERANCE_PER_REV, 250, "DEFAULT_US_TOLERANCE_PER_REV should be 250.");
    }

    #[test]
    fn us_per_rev() {
        assert_eq!(
            DEFAULT_US_PER_REV,
            (DEFAULT_MS_PER_REV * 1000) + DEFAULT_US_TOLERANCE_PER_REV,
            "DEFAULT_US_PER_REV should be (DEFAULT_MS_PER_REV * 1000) + DEFAULT_US_TOLERANCE_PER_REV."
        );
        assert_eq!(DEFAULT_US_PER_REV, 6250, "DEFAULT_US_PER_REV should be 6250.");
    }

    #[test]
    fn revs_per_phase() {
        assert_eq!(DEFAULT_REVS_PER_PHASE, 64, "DEFAULT_REVS_PER_PHASE should be 64.");
    }

    #[test]
    fn hashes_per_second() {
        assert_eq!(DEFAULT_HASHES_PER_SECOND, 2_000_000, "DEFAULT_HASHES_PER_SECOND should be 2,000,000.");
    }

    #[test]
    fn hashes_per_rev() {
        assert_eq!(
            DEFAULT_HASHES_PER_REV,
            DEFAULT_HASHES_PER_SECOND / DEFAULT_REVS_PER_SECOND,
            "DEFAULT_HASHES_PER_REV should be DEFAULT_HASHES_PER_SECOND / DEFAULT_REVS_PER_SECOND."
        );
        assert_eq!(DEFAULT_HASHES_PER_REV, 12_500, "DEFAULT_HASHES_PER_REV should be 12,500.");
    }

    #[test]
    fn s_per_phase() {
        let expected: f64 = DEFAULT_REVS_PER_PHASE as f64 / DEFAULT_REVS_PER_SECOND as f64;
        assert!(
            (DEFAULT_S_PER_PHASE - expected).abs() < f64::EPSILON,
            "DEFAULT_S_PER_PHASE should be DEFAULT_REVS_PER_PHASE / DEFAULT_REVS_PER_SECOND = {}.",
            expected
        );
        assert!((DEFAULT_S_PER_PHASE - 0.4).abs() < f64::EPSILON, "DEFAULT_S_PER_PHASE should be 0.4.");
    }

    #[test]
    fn ms_per_phase() {
        assert_eq!(
            DEFAULT_MS_PER_PHASE,
            1_000 * DEFAULT_REVS_PER_PHASE / DEFAULT_REVS_PER_SECOND,
            "DEFAULT_MS_PER_PHASE should be 1000 * DEFAULT_REVS_PER_PHASE / DEFAULT_REVS_PER_SECOND."
        );
        assert_eq!(DEFAULT_MS_PER_PHASE, 400, "DEFAULT_MS_PER_PHASE should be 400.");
    }

    #[test]
    fn phases_per_cycle() {
        assert_eq!(
            DEFAULT_PHASES_PER_CYCLE,
            2 * DEFAULT_REVS_PER_DAY / DEFAULT_REVS_PER_PHASE,
            "DEFAULT_PHASES_PER_CYCLE should be 2 * DEFAULT_REVS_PER_DAY / DEFAULT_REVS_PER_PHASE."
        );
        assert_eq!(DEFAULT_PHASES_PER_CYCLE, 432_000, "DEFAULT_PHASES_PER_CYCLE should be 432,000.");
    }

    #[test]
    fn dev_phases_per_cycle() {
        assert_eq!(DEFAULT_DEV_PHASES_PER_CYCLE, 8_192, "DEFAULT_DEV_PHASES_PER_CYCLE should be 8,192.");
    }

    #[test]
    fn num_consecutive_leader_phases() {
        assert_eq!(DEFAULT_NUM_CONSECUTIVE_LEADER_PHASES, 4, "DEFAULT_NUM_CONSECUTIVE_LEADER_PHASES should be 4.");
    }

    #[test]
    fn channel_capacity() {
        assert_eq!(DEFAULT_CHANNEL_CAPACITY, 1_000, "DEFAULT_CHANNEL_CAPACITY should be 1,000.");
    }

    #[test]
    fn batch_size() {
        assert_eq!(DEFAULT_BATCH_SIZE, 64, "DEFAULT_BATCH_SIZE should be 64.");
    }

    #[test]
    fn spinlock_threshold_us() {
        assert_eq!(DEFAULT_SPINLOCK_THRESHOLD_US, 250, "DEFAULT_SPINLOCK_THRESHOLD_US should be 250.");
    }

    #[test]
    fn derived_relationships() {
        // Test relationships between constants.
        assert_eq!(
            DEFAULT_REVS_PER_DAY,
            DEFAULT_REVS_PER_SECOND * DEFAULT_SECONDS_PER_DAY,
            "DEFAULT_REVS_PER_DAY relationship is incorrect."
        );

        assert_eq!(
            DEFAULT_US_PER_REV,
            (DEFAULT_MS_PER_REV * 1000) + DEFAULT_US_TOLERANCE_PER_REV,
            "DEFAULT_US_PER_REV relationship is incorrect."
        );

        assert_eq!(
            DEFAULT_HASHES_PER_REV,
            DEFAULT_HASHES_PER_SECOND / DEFAULT_REVS_PER_SECOND,
            "DEFAULT_HASHES_PER_REV relationship is incorrect."
        );

        assert_eq!(
            DEFAULT_MS_PER_PHASE,
            1_000 * DEFAULT_REVS_PER_PHASE / DEFAULT_REVS_PER_SECOND,
            "DEFAULT_MS_PER_PHASE relationship is incorrect."
        );

        assert_eq!(
            DEFAULT_PHASES_PER_CYCLE,
            2 * DEFAULT_REVS_PER_DAY / DEFAULT_REVS_PER_PHASE,
            "DEFAULT_PHASES_PER_CYCLE relationship is incorrect."
        );
    }
}
