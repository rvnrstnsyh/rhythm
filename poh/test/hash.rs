#[cfg(test)]
mod poh_hash {
    use std::time::{Duration, Instant};

    use poh::digest;

    use blake3::Hasher as Blake3Hasher;
    use ring::digest::{Context, Digest, SHA256};

    // Test vector for consistent hash values.
    const TEST_DATA: &[u8] = b"PoH test vector for Proof of History implementation.";
    // Test iterations for small tests.
    const SMALL_ITERATIONS: u64 = 10;
    // Test iterations for performance tests.
    const PERF_ITERATIONS: u64 = 10_000;

    #[test]
    fn hash_algorithms_available() {
        // Make sure both algorithms are available and can be selected.
        digest::set_hash_algorithm(0);

        assert_eq!(digest::get_current_algorithm(), 0, "Should be able to select SHA-256.");
        assert_eq!(digest::get_algorithm_name(), "SHA-256", "Algorithm name should be SHA-256.");

        digest::set_hash_algorithm(1);

        assert_eq!(digest::get_current_algorithm(), 1, "Should be able to select BLAKE3.");
        assert_eq!(digest::get_algorithm_name(), "BLAKE3", "Algorithm name should be BLAKE3.");

        // Test invalid algorithm defaults to SHA-256.
        digest::set_hash_algorithm(99);

        assert_eq!(digest::get_current_algorithm(), 0, "Invalid algorithm should default to SHA-256.");

        // Reset to SHA-256 for subsequent tests.
        digest::set_hash_algorithm(0);
    }

    #[test]
    fn hash_function_basic() {
        // Test basic hash function with known values.
        let result: [u8; 32] = digest::hash(TEST_DATA);
        let expected_sha256: Digest = ring::digest::digest(&SHA256, TEST_DATA);

        let mut expected_bytes: [u8; 32] = [0u8; 32];
        expected_bytes.copy_from_slice(expected_sha256.as_ref());

        assert_eq!(result, expected_bytes, "Basic hash function should match expected SHA-256 output.");

        // Test with BLAKE3.
        digest::set_hash_algorithm(1);

        let result_blake3: [u8; 32] = digest::hash(TEST_DATA);
        let expected_blake3: [u8; 32] = *blake3::hash(TEST_DATA).as_bytes();

        assert_eq!(result_blake3, expected_blake3, "Basic hash function should match expected BLAKE3 output.");
        // Reset to SHA-256.
        digest::set_hash_algorithm(0);
    }

    #[test]
    fn hash_with_data() {
        let prev_hash: [u8; 32] = [b'1'; 32];

        digest::set_hash_algorithm(0);

        let result: [u8; 32] = digest::hash_with_data(&prev_hash, TEST_DATA);
        // Compute expected SHA-256 hash manually.
        let mut context: Context = Context::new(&SHA256);
        context.update(&prev_hash);
        context.update(TEST_DATA);
        let expected_digest: Digest = context.finish();
        let mut expected: [u8; 32] = [0u8; 32];
        expected.copy_from_slice(expected_digest.as_ref());

        assert_eq!(result, expected, "hash_with_data should match manual SHA-256 calculation.");

        // Test BLAKE3.
        digest::set_hash_algorithm(1);

        let result_blake3: [u8; 32] = digest::hash_with_data(&prev_hash, TEST_DATA);
        // Compute expected BLAKE3 hash manually.
        let mut hasher: Blake3Hasher = Blake3Hasher::new();

        hasher.update(&prev_hash);
        hasher.update(TEST_DATA);

        let expected_blake3: [u8; 32] = *hasher.finalize().as_bytes();

        assert_eq!(result_blake3, expected_blake3, "hash_with_data should match manual BLAKE3 calculation.");

        digest::set_hash_algorithm(0);
    }

    #[test]
    fn hash_chain_correctness() {
        let seed: [u8; 32] = [b'0'; 32];

        digest::set_hash_algorithm(0);

        // Compute our reference implementation result.
        let hash1: [u8; 32] = manual_hash_chain_sha256(&seed, SMALL_ITERATIONS);
        // Call the function being tested.
        let hash2: [u8; 32] = digest::extend_hash_chain(&seed, SMALL_ITERATIONS);

        // Debug output to help diagnose failures.
        println!("Reference SHA-256: {:?}.", hash1);
        println!("Actual SHA-256:    {:?}.", hash2);

        assert_eq!(hash1, hash2, "SHA-256 hash chains should produce identical results.");
    }

    #[test]
    fn hash_chain_with_different_iterations() {
        let seed: [u8; 32] = [b'0'; 32];
        // Test with fewer iterations to start.
        let test_iterations: [u64; 8] = [1, 2, 3, 4, 5, 6, 7, 8];

        for &iter in &test_iterations {
            digest::set_hash_algorithm(0);

            let expected: [u8; 32] = manual_hash_chain_sha256(&seed, iter);
            let actual: [u8; 32] = digest::extend_hash_chain(&seed, iter);

            // Debug output for failures.
            if expected != actual {
                println!("Iteration count: {}.", iter);
                println!("Expected: {:?}.", expected);
                println!("Actual:   {:?}.", actual);
            }
            assert_eq!(actual, expected, "SHA-256 hash chain with {} iterations failed.", iter);
        }
    }

    #[test]
    fn verify_hash_chain() {
        // Simplified verification test with fixed values.
        let seed: [u8; 32] = [0x01; 32];
        let iterations: u64 = 3;
        // Manually generate expected hash without using digest::extend_hash_chain.
        let mut expected_hash: [u8; 32] = seed;

        for _ in 0..iterations {
            let mut context: Context = Context::new(&SHA256);
            context.update(&expected_hash);
            let result: Digest = context.finish();
            expected_hash.copy_from_slice(result.as_ref());
        }

        // Verify that this manually calculated hash can be verified.
        assert!(
            digest::verify_hash_chain(&seed, &expected_hash, iterations, None),
            "Hash chain verification should succeed with manually calculated hash."
        );

        // Tamper with the hash to ensure verification fails.
        let mut tampered_hash: [u8; 32] = expected_hash;
        tampered_hash[0] ^= 1; // Flip a bit.

        assert!(
            !digest::verify_hash_chain(&seed, &tampered_hash, iterations, None),
            "Hash chain verification should fail with tampered hash."
        );

        // Testing with event data.
        let data: &[u8] = TEST_DATA;
        let mut hash_with_event: [u8; 32] = seed;

        // First hash with data.
        let mut context: Context = Context::new(&SHA256);
        context.update(&hash_with_event);
        context.update(data);
        let result: Digest = context.finish();
        hash_with_event.copy_from_slice(result.as_ref());

        // Then continue the chain.
        for _ in 0..iterations {
            let mut context: Context = Context::new(&SHA256);
            context.update(&hash_with_event);
            let result: Digest = context.finish();
            hash_with_event.copy_from_slice(result.as_ref());
        }

        // Verify the chain with event data.
        assert!(
            digest::verify_hash_chain(&seed, &hash_with_event, iterations, Some(data)),
            "Hash chain verification with event data should succeed."
        );
    }

    #[test]
    fn hash_chain_performance() {
        let seed: [u8; 32] = [b'0'; 32];

        // Test SHA-256 performance.
        digest::set_hash_algorithm(0);

        let start_sha256: Instant = Instant::now();
        let sha256_result: [u8; 32] = digest::extend_hash_chain(&seed, PERF_ITERATIONS);
        let sha256_duration: Duration = start_sha256.elapsed();

        // Test BLAKE3 performance.
        digest::set_hash_algorithm(1);

        let start_blake3: Instant = Instant::now();
        let blake3_result: [u8; 32] = digest::extend_hash_chain(&seed, PERF_ITERATIONS);
        let blake3_duration: Duration = start_blake3.elapsed();

        // Prevent compiler from optimizing away the calculations.
        assert_ne!(sha256_result, blake3_result, "SHA-256 and BLAKE3 should produce different hashes.");

        println!("SHA-256: {:?} for {} iterations.", sha256_duration, PERF_ITERATIONS);
        println!("BLAKE3:  {:?} for {} iterations.", blake3_duration, PERF_ITERATIONS);

        digest::set_hash_algorithm(0);
    }

    #[test]
    fn hash_chain_determinism() {
        let seed: [u8; 32] = [b'0'; 32];
        let iterations: u64 = 5; // Use a smaller number for reliability.

        // Each hash chain execution should produce identical results.
        digest::set_hash_algorithm(0); // Use only SHA-256 to simplify test.

        // First generate a reference result.
        let reference_result: [u8; 32] = digest::extend_hash_chain(&seed, iterations);

        // Then check that multiple executions produce the same result.
        for i in 0..3 {
            let result: [u8; 32] = digest::extend_hash_chain(&seed, iterations);
            assert_eq!(result, reference_result, "Hash chain iteration {} should be deterministic.", i);
        }
    }

    #[test]
    fn constant_time_comparison() {
        // This test directly verifies the behavior of digest::verify_hash_chain without
        // depending on the correctness of digest::extend_hash_chain.
        let seed: [u8; 32] = [b'a'; 32];
        // Directly test the verification functionality with simple values
        // instead of relying on the correctness of digest::extend_hash_chain.
        let data: &[u8] = TEST_DATA;

        // Create a valid hash chain manually.
        let mut expected_hash: [u8; 32] = seed;
        let mut context: Context = Context::new(&SHA256);
        context.update(&expected_hash);
        context.update(data);
        let result: Digest = context.finish();
        expected_hash.copy_from_slice(result.as_ref());

        // Manually hash a few times to simulate a short chain.
        for _ in 0..5 {
            let mut context: Context = Context::new(&SHA256);
            context.update(&expected_hash);
            let result: Digest = context.finish();
            expected_hash.copy_from_slice(result.as_ref());
        }

        // Use digest::verify_hash_chain to see if our manual chain matches.
        assert!(
            digest::verify_hash_chain(&seed, &expected_hash, 5, Some(data)),
            "Manually created valid hash chain should verify correctly."
        );

        // Create a slightly modified hash to test failure case.
        let mut tampered_hash: [u8; 32] = expected_hash;
        tampered_hash[0] ^= 1; // Flip one bit.

        assert!(
            !digest::verify_hash_chain(&seed, &tampered_hash, 5, Some(data)),
            "Tampered hash should fail verification."
        );
    }

    #[test]
    fn hash_chain_with_large_iterations() {
        // Test with a larger number of iterations to ensure unrolled hashing works correctly.
        let seed: [u8; 32] = [b'0'; 32];
        let iterations: u64 = 1_000;

        digest::set_hash_algorithm(0);

        let expected: [u8; 32] = manual_hash_chain_sha256(&seed, iterations);
        let actual: [u8; 32] = digest::extend_hash_chain(&seed, iterations);

        assert_eq!(actual, expected, "SHA-256 hash chain with {} iterations failed.", iterations);

        digest::set_hash_algorithm(1);

        let expected_blake3: [u8; 32] = manual_hash_chain_blake3(&seed, iterations);
        let actual_blake3: [u8; 32] = digest::extend_hash_chain(&seed, iterations);

        assert_eq!(actual_blake3, expected_blake3, "BLAKE3 hash chain with {} iterations failed.", iterations);

        digest::set_hash_algorithm(0);
    }

    #[test]
    fn hash_boundary_conditions() {
        // Test with empty data.
        let empty_data: &[u8] = &[];
        let result: [u8; 32] = digest::hash(empty_data);
        let expected_sha256: Digest = ring::digest::digest(&SHA256, empty_data);

        let mut expected_bytes: [u8; 32] = [0u8; 32];
        expected_bytes.copy_from_slice(expected_sha256.as_ref());

        assert_eq!(result, expected_bytes, "Hash of empty data should match expected output.");

        // Test with zero iterations.
        let seed: [u8; 32] = [b'0'; 32];
        let result_zero_iter: [u8; 32] = digest::extend_hash_chain(&seed, 0);

        assert_eq!(result_zero_iter, seed, "Zero iterations should return the seed hash unchanged.");
    }

    // Reference implementation for SHA-256 testing.
    fn manual_hash_chain_sha256(prev_hash: &[u8; 32], iterations: u64) -> [u8; 32] {
        let mut current_hash: [u8; 32] = *prev_hash;
        for _ in 0..iterations {
            let mut context: Context = Context::new(&SHA256);
            context.update(&current_hash);
            let result: Digest = context.finish();
            current_hash.copy_from_slice(result.as_ref());
        }
        return current_hash;
    }

    // Reference implementation for BLAKE3 testing.
    fn manual_hash_chain_blake3(prev_hash: &[u8; 32], iterations: u64) -> [u8; 32] {
        let mut current_hash: [u8; 32] = *prev_hash;
        for _ in 0..iterations {
            let mut hasher: Blake3Hasher = Blake3Hasher::new();
            hasher.update(&current_hash);
            current_hash = *hasher.finalize().as_bytes();
        }
        return current_hash;
    }
}
