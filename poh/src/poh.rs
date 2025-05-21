use std::{
    thread as std_thread,
    time::{Duration, Instant},
};

use crate::types::{PoH, Record};

use lib::{
    hash::Hasher,
    metronome::{DEFAULT_HASHES_PER_REV, DEFAULT_PHASES_PER_CYCLE, DEFAULT_REVS_PER_PHASE, DEFAULT_SPINLOCK_THRESHOLD_US, DEFAULT_US_PER_REV},
};

impl PoH {
    pub fn new(seed: &[u8]) -> Self {
        let hasher: Hasher = Hasher::default();
        let current_hash: [u8; 32] = hasher.hash(seed);
        return Self {
            current_hash,
            rev_count: 0,
            phase_count: 0,
            cycle_count: 0,
            start_time: Instant::now(),
            next_rev_target_us: DEFAULT_US_PER_REV,
        };
    }

    pub fn next_rev(&mut self) -> Record {
        return self.core(None);
    }

    pub fn insert_event(&mut self, event_data: &[u8]) -> Record {
        return self.core(Some(event_data));
    }

    pub fn verify_records(records: &[Record]) -> bool {
        if records.is_empty() {
            return false;
        }

        let hasher: Hasher = Hasher::default();

        for window in records.windows(2) {
            let prev: &Record = &window[0];
            let curr: &Record = &window[1];
            let event_data: Option<&[u8]> = curr.event.as_deref();

            if !hasher.verify_hash_chain(&prev.hash, &curr.hash, DEFAULT_HASHES_PER_REV, event_data) {
                return false;
            }

            // Verify sequence numbers.
            let rev_index_valid: bool = curr.rev_index == prev.rev_index.saturating_add(1);
            let phase_index_valid: bool = curr.phase_index == curr.rev_index / DEFAULT_REVS_PER_PHASE;
            let cycle_valid: bool = curr.cycle_index == curr.rev_index / (DEFAULT_REVS_PER_PHASE * DEFAULT_PHASES_PER_CYCLE);

            if !(rev_index_valid && phase_index_valid && cycle_valid) {
                return false;
            }
        }
        return true;
    }

    pub fn verify_timestamps(records: &[Record], log_failures: bool) -> bool {
        if records.is_empty() {
            return false;
        }

        let first_timestamp: u64 = records[0].timestamp_ms;

        for (i, record) in records.iter().enumerate() {
            let timestamp: u64 = record.timestamp_ms;
            let expected_timestamp: u64 = first_timestamp.saturating_add((i as u64).checked_mul(DEFAULT_US_PER_REV).unwrap_or(0) / 1000);
            // Adjust tolerance based on whether this is an event rev.
            let allowed_drift: u64 = 8; // ~8ms tolerance, relaxed.
            // Ensure we don't underflow.
            let lower_bound: u64 = expected_timestamp.saturating_sub(allowed_drift);
            let upper_bound: u64 = expected_timestamp.saturating_add(allowed_drift);

            let too_early: bool = timestamp < lower_bound;
            let too_late: bool = timestamp > upper_bound;

            if too_early || too_late {
                if log_failures {
                    println!(
                        "Timestamp mismatch at record {}: actual={}, expected={}, drift={}, allowed=~{}",
                        i,
                        timestamp,
                        expected_timestamp,
                        if too_early {
                            lower_bound.saturating_sub(timestamp)
                        } else {
                            timestamp.saturating_sub(upper_bound)
                        },
                        allowed_drift
                    );
                }
                return false;
            }
        }
        return true;
    }

    fn core(&mut self, event_data: Option<&[u8]>) -> Record {
        // Control timing.
        self.enforce_timing();

        let hasher: Hasher = Hasher::default();

        if let Some(event) = event_data {
            self.current_hash = hasher.embed_data(&self.current_hash, event);
        }

        self.current_hash = hasher.extend_hash_chain(&self.current_hash, DEFAULT_HASHES_PER_REV);

        let rev_index: u64 = self.rev_count;
        let phase_index: u64 = rev_index / DEFAULT_REVS_PER_PHASE;
        let cycle_index: u64 = phase_index / DEFAULT_PHASES_PER_CYCLE;
        let record: Record = Record {
            hash: self.current_hash,
            rev_index,
            phase_index,
            cycle_index,
            timestamp_ms: self.start_time.elapsed().as_millis() as u64,
            event: event_data.map(|d| d.to_vec()),
        };

        self.rev_count = self.rev_count.checked_add(1).expect("rev_count overflow");

        if self.rev_count % DEFAULT_REVS_PER_PHASE == 0 {
            self.phase_count = self.phase_count.checked_add(1).expect("phase_count overflow");
        }

        if phase_index % DEFAULT_PHASES_PER_CYCLE == 0 && self.rev_count % DEFAULT_REVS_PER_PHASE == 0 {
            self.cycle_count = cycle_index;
            self.phase_count = 0;
        }

        // Calculate next rev target time.
        self.next_rev_target_us = self.next_rev_target_us.saturating_add(DEFAULT_US_PER_REV);

        return record;
    }

    fn enforce_timing(&self) {
        let elapsed_us: u64 = self.start_time.elapsed().as_micros() as u64;
        let target_us: u64 = self.next_rev_target_us;

        if elapsed_us < target_us {
            let sleep_us: u64 = target_us.saturating_sub(elapsed_us);
            // Use spin waiting for very short sleeps to improve precision.
            if sleep_us < DEFAULT_SPINLOCK_THRESHOLD_US {
                // Spin wait for greater timing precision.
                let spin_until: u128 = self.start_time.elapsed().as_micros().saturating_add(sleep_us as u128);
                while self.start_time.elapsed().as_micros() < spin_until {
                    // Insert a pause instruction to reduce CPU usage during spin-waiting.
                    #[cfg(target_arch = "x86_64")]
                    unsafe {
                        std::arch::x86_64::_mm_pause();
                    }
                }
            } else {
                // Use normal sleep for longer durations.
                std_thread::sleep(Duration::from_micros(sleep_us));
            }
        }
    }
}
