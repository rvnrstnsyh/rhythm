#[cfg(test)]
mod poh_operations {
    use std::{
        sync::mpsc::sync_channel,
        time::{Duration, Instant},
    };

    use poh::types::{PoH, Record};

    use thread::native_runtime::types::{Config, JoinHandle, Native};

    use lib::{
        digest,
        metronome::{DEFAULT_BATCH_SIZE, DEFAULT_CHANNEL_CAPACITY, DEFAULT_HASHES_PER_REV, DEFAULT_PHASES_PER_CYCLE, DEFAULT_REVS_PER_PHASE, DEFAULT_US_PER_REV},
    };

    #[test]
    fn poh_record_construction() {
        let seed: [u8; 64] = [b'0'; 64];
        let mut poh: PoH = PoH::new(&seed);

        let record1: Record = poh.next_rev();
        let record2: Record = poh.next_rev();

        // Ensure records have consecutive rev indices.
        assert_eq!(record1.rev_index + 1, record2.rev_index);
        // Ensure phase calculation is correct.
        assert_eq!(record1.phase_index, record1.rev_index / DEFAULT_REVS_PER_PHASE);
        assert_eq!(record2.phase_index, record2.rev_index / DEFAULT_REVS_PER_PHASE);
        // Ensure cycle calculation is correct.
        assert_eq!(record1.cycle_index, record1.phase_index / DEFAULT_PHASES_PER_CYCLE);
        assert_eq!(record2.cycle_index, record2.phase_index / DEFAULT_PHASES_PER_CYCLE);
    }

    #[test]
    fn hash_chain_extension() {
        let seed: [u8; 32] = [1u8; 32]; // Some seed data.
        let iterations: u64 = 10;
        // Test hash chain extension.
        let result: [u8; 32] = digest::extend_hash_chain(&seed, iterations);
        // Verify by manually applying hash iterations.
        let mut expected: [u8; 32] = seed;

        for _ in 0..iterations {
            expected = digest::hash(&expected);
        }

        assert_eq!(result, expected, "Hash chain extension produced incorrect result.");
    }

    #[test]
    fn hash_chain_verification() {
        let seed: [u8; 32] = [1u8; 32]; // Initial hash.
        let iterations: u64 = DEFAULT_HASHES_PER_REV;
        let event_data: &'static [u8; 10] = b"Test event";
        // Create a valid hash chain with event.
        let mut current_hash: [u8; 32] = digest::hash_with_data(&seed, event_data);

        current_hash = digest::extend_hash_chain(&current_hash, iterations);
        // Verify the valid hash chain.
        assert!(
            digest::verify_hash_chain(&seed, &current_hash, iterations, Some(event_data)),
            "Valid hash chain verification failed."
        );

        // Modify hash and ensure verification fails.
        let mut bad_hash: [u8; 32] = current_hash;
        bad_hash[0] ^= 0xFF; // Corrupt the hash.

        assert!(
            !digest::verify_hash_chain(&seed, &bad_hash, iterations, Some(event_data)),
            "Corrupted hash chain verification didn't fail."
        );
    }

    #[test]
    fn event_insertion() {
        let seed: [u8; 64] = [b'0'; 64];
        let mut poh: PoH = PoH::new(&seed);

        let event_data: &'static str = "Test event data";

        let rev1: Record = poh.next_rev(); // Normal rev.
        let rev2: Record = poh.insert_event(event_data.as_bytes()); // Rev with event.
        let rev3: Record = poh.next_rev(); // Normal rev.

        // Check that event was stored.
        assert!(rev2.event.is_some());
        assert_eq!(rev2.event.clone().unwrap(), event_data.as_bytes());
        // Check that non-event revs don't have events.
        assert!(rev1.event.is_none());
        assert!(rev3.event.is_none());
        // Verify hash chain integrity across all revs.
        let records: Vec<Record> = vec![rev1, rev2, rev3];
        assert!(PoH::verify_records(&records), "Records with event failed verification.");
    }

    #[test]
    fn phase_transition() {
        let seed: [u8; 64] = [b'0'; 64];

        let mut poh: PoH = PoH::new(&seed);
        let mut records: Vec<Record> = Vec::with_capacity((DEFAULT_REVS_PER_PHASE + 5) as usize);

        // Generate revs across a phase boundary.
        for _ in 0..DEFAULT_REVS_PER_PHASE + 5 {
            records.push(poh.next_rev());
        }

        let last_rev: &Record = &records[DEFAULT_REVS_PER_PHASE as usize - 1];
        let first_rev: &Record = &records[DEFAULT_REVS_PER_PHASE as usize];

        // Verify phase transition.
        // Phase indexing starts at 0, so the last rev of phase 0 should be at index DEFAULT_REVS_PER_PHASE-1.
        assert_eq!(last_rev.phase_index, 0, "Last rev of phase 0 has incorrect phase_index.");
        // The first rev of phase 1 should be at index DEFAULT_REVS_PER_PHASE.
        assert_eq!(first_rev.phase_index, 1, "First rev of phase 1 has incorrect phase_index.");
        // Verify hash chain integrity across phase boundary.
        assert!(PoH::verify_records(&records), "Records across phase boundary failed verification.");
    }

    #[test]
    fn timestamp_consistency() {
        let seed: [u8; 64] = [b'0'; 64];
        let mut poh: PoH = PoH::new(&seed);
        let count: usize = 100;
        let mut records: Vec<Record> = Vec::with_capacity(count);

        for _ in 0..count {
            records.push(poh.next_rev());
        }
        // Verify timestamps are monotonically increasing.
        for i in 1..records.len() {
            assert!(
                records[i].timestamp_ms >= records[i - 1].timestamp_ms,
                "Timestamps not monotonically increasing at index {}.",
                i
            );
        }

        // Check average rev duration is reasonable (without strict assertions).
        let mut total_diff: u64 = 0u64;
        let mut count_diffs: i32 = 0;

        for i in 1..records.len() {
            total_diff += records[i].timestamp_ms - records[i - 1].timestamp_ms;
            count_diffs += 1;
        }
        if count_diffs > 0 {
            let avg_rev_ms: f64 = total_diff as f64 / count_diffs as f64;
            // Allow wide tolerance since test environment timing can vary.
            assert!(avg_rev_ms > 0.0, "Average rev duration should be positive.");
            println!("Average rev duration: {:.3} ms.", avg_rev_ms);
        }
    }

    #[test]
    fn corruption_detection() {
        let seed: [u8; 64] = [b'0'; 64];
        let mut poh: PoH = PoH::new(&seed);
        let count: usize = 10;
        let mut records: Vec<Record> = Vec::with_capacity(count);

        for _ in 0..count {
            records.push(poh.next_rev());
        }

        // Verify original records are valid.
        assert!(PoH::verify_records(&records), "Valid records failed verification.");
        // Test various corruption scenarios
        let mut corrupted: Vec<Record> = records.clone();
        // 1. Corrupt a hash.
        corrupted[5].hash[0] ^= 0xFF;
        assert!(!PoH::verify_records(&corrupted), "Failed to detect hash corruption.");
        // 2. Corrupt rev index.
        corrupted = records.clone();
        corrupted[3].rev_index += 2; // Skip a rev index.
        assert!(!PoH::verify_records(&corrupted), "Failed to detect rev index corruption.");
        // 3. Corrupt phase index.
        corrupted = records.clone();
        corrupted[4].phase_index += 1; // Incorrect phase index.
        assert!(!PoH::verify_records(&corrupted), "Failed to detect phase index corruption.");
        // 4. Corrupt cycle.
        corrupted = records.clone();
        corrupted[5].cycle_index += 1; // Incorrect cycle.
        assert!(!PoH::verify_records(&corrupted), "Failed to detect cycle corruption.");
    }

    #[test]
    fn constant_time_eq() {
        // Can't test the actual constant-time property, but can test correctness.
        let hash1: [u8; 32] = [0u8; 32];
        let hash2: [u8; 32] = [0u8; 32];
        let hash3: [u8; 32] = {
            let mut h: [u8; 32] = [0u8; 32];
            h[31] = 1; // Differs at the last byte.
            h
        };

        // Test the function through verify_hash_chain which uses constant_time_eq.
        assert!(digest::verify_hash_chain(&hash1, &hash2, 0, None), "Equal hashes not recognized as equal.");
        assert!(!digest::verify_hash_chain(&hash1, &hash3, 0, None), "Different hashes not recognized as different.");
    }

    #[test]
    fn realistic_poh_operations() {
        // Default seed - 64 bytes of '0'.
        let seed: [u8; 64] = [b'0'; 64];
        // Generate enough revs for 1.2 seconds (about 192 revs at 6.25ms per rev).
        // 1.2 seconds = 1,200 milliseconds.
        // 1,200 milliseconds / 6.25 milliseconds per rev = 192 revs.
        let test_revs: u64 = 192;
        let start_time: Instant = Instant::now();

        let (tx, rx) = sync_channel(DEFAULT_CHANNEL_CAPACITY);
        let seed_vec: Vec<u8> = seed.to_vec();
        let worker: Native = Native::new("poh-test-thread".to_string(), Config::default()).expect("Failed to create thread worker.");
        let _: JoinHandle<()> = worker
            .spawn(move || {
                let mut poh: PoH = PoH::new(&seed_vec);
                let mut records_batch: Vec<Record> = Vec::with_capacity(DEFAULT_BATCH_SIZE);

                for i in 0..test_revs {
                    // Simulate event insertion every 10 revs.
                    let record: Record = if i % 10 == 0 {
                        let event_data: String = format!("Event at rev {}.", i);
                        poh.insert_event(event_data.as_bytes())
                    } else {
                        poh.next_rev()
                    };

                    records_batch.push(record);
                    // Send in batches.
                    if records_batch.len() >= DEFAULT_BATCH_SIZE {
                        for record in records_batch.drain(..) {
                            if tx.send(record).is_err() {
                                return; // Exit if receiver has been dropped.
                            }
                        }
                    }
                }
                // Send any remaining records
                for record in records_batch.drain(..) {
                    let _ = tx.send(record);
                }
            })
            .expect("Failed to spawn PoH thread.");

        // Collect and analyze records.
        let mut records_received: Vec<Record> = Vec::with_capacity(test_revs as usize);
        let mut last_phase: u64 = 0;
        let mut phase_transitions: i32 = 0;
        let mut counter: u64 = 0;

        while let Ok(record) = rx.recv() {
            if record.phase_index != last_phase {
                phase_transitions = phase_transitions.saturating_add(1);
                last_phase = record.phase_index;
            }
            records_received.push(record);
            counter = counter.saturating_add(1);
            if counter >= test_revs {
                break; // Exit loop after collecting the required number of records.
            }
        }

        let elapsed_time: Duration = start_time.elapsed();

        assert_eq!(records_received.len(), test_revs as usize, "Incorrect number of revs generated.");
        assert_eq!(
            phase_transitions, 2,
            "Incorrect number of phase transitions. Expected 2 transitions but got {}.",
            phase_transitions
        );

        let expected_duration: Duration = Duration::from_micros(DEFAULT_US_PER_REV * test_revs);
        let expected_ms: u64 = expected_duration.as_millis() as u64;
        let actual_ms: u64 = elapsed_time.as_millis() as u64;

        // More lenient tolerance: 75% to 300% of the expected value.
        let lower_bound: u64 = (expected_ms as f64 * 0.75) as u64;
        let upper_bound: u64 = (expected_ms as f64 * 3.0) as u64;

        // Print timing info for debugging.
        println!(
            "Expected: ~{} ms, Got: {} ms, Acceptable range: {} ms - {} ms.",
            expected_ms, actual_ms, lower_bound, upper_bound
        );

        // Ensure actual time falls within the tolerance range.
        if !cfg!(debug_assertions) {
            assert!(
                actual_ms >= lower_bound && actual_ms <= upper_bound,
                "Timing outside acceptable range: expected ~{} ms, got {} ms.",
                expected_ms,
                actual_ms
            );
        }

        assert!(PoH::verify_records(&records_received), "PoH integrity check failed.");

        for i in 1..records_received.len() {
            let prev: &Record = &records_received[i - 1];
            let curr: &Record = &records_received[i];

            assert_eq!(curr.rev_index, prev.rev_index + 1, "Non-sequential rev indices at position {}.", i);
            assert_eq!(
                curr.phase_index,
                curr.rev_index / DEFAULT_REVS_PER_PHASE,
                "Incorrect phase index calculation at position {}.",
                i
            );
        }
    }

    #[test]
    fn hash_rate_constant() {
        // Verify that DEFAULT_HASHES_PER_REV = 12500 as specified in requirements.
        assert_eq!(DEFAULT_HASHES_PER_REV, 12500, "DEFAULT_HASHES_PER_REV should be 12500.");
    }

    #[test]
    fn rev_duration_constant() {
        // Verify that DEFAULT_US_PER_REV is approximately 6250 microseconds (6.25ms).
        assert_eq!(DEFAULT_US_PER_REV, 6250, "DEFAULT_US_PER_REV should be 6250 microseconds (6.25ms).");
    }

    #[test]
    fn phase_duration_constant() {
        // Verify that a phase (64 revs) should take approximately 400ms.
        let phase_duration_ms: u64 = (DEFAULT_US_PER_REV * DEFAULT_REVS_PER_PHASE) / 1_000;
        assert_eq!(phase_duration_ms, 400, "1 phase should be 400ms duration.");
    }

    #[test]
    fn cycle_constants() {
        // Verify that 1 cycle = 432000 phases.
        assert_eq!(DEFAULT_PHASES_PER_CYCLE, 432000, "DEFAULT_PHASES_PER_CYCLE should be 432000.");
        // Verify that 1 cycle = 2 days.
        // 1 phase = 400ms.
        // 1 day = 24 * 60 * 60 * 1000 ms = 86400000 ms.
        // 2 days = 172800000 ms.
        // 172800000 ms / 400 ms per phase = 432000 phases per cycle.
        let ms_per_phase: u64 = (DEFAULT_US_PER_REV * DEFAULT_REVS_PER_PHASE) / 1_000;
        let ms_per_cycle: u64 = ms_per_phase * DEFAULT_PHASES_PER_CYCLE;
        let days_per_cycle: f64 = ms_per_cycle as f64 / (24.0 * 60.0 * 60.0 * 1_000.0);

        assert!((days_per_cycle - 2.0).abs() < 0.001, "1 cycle should be approximately 2 days.");
    }
}
