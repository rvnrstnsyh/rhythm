use crate::DEFAULT_HASH;

use blake3::Hasher as Blake3Hasher;
use ring::digest::{Context as RingContext, Digest, SHA256, digest};

#[inline]
fn get_hash_algorithm() -> u8 {
    return unsafe {
        match DEFAULT_HASH {
            0 => 0, // SHA-256.
            1 => 1, // BLAKE3.
            _ => 0, // Default to SHA-256 for any other value.
        }
    };
}

#[inline]
pub fn hash(data: &[u8]) -> [u8; 32] {
    match get_hash_algorithm() {
        1 => {
            // BLAKE3.
            *blake3::hash(data).as_bytes()
        }
        _ => {
            // SHA-256 (default).
            let hash_result: Digest = digest(&SHA256, data);
            let mut hash_bytes: [u8; 32] = [0u8; 32];
            hash_bytes.copy_from_slice(hash_result.as_ref());
            hash_bytes
        }
    }
}

#[inline]
pub fn hash_with_data(prev_hash: &[u8; 32], data: &[u8]) -> [u8; 32] {
    match get_hash_algorithm() {
        1 => {
            // BLAKE3.
            let mut hasher: Blake3Hasher = Blake3Hasher::new();
            hasher.update(prev_hash);
            hasher.update(data);
            *hasher.finalize().as_bytes()
        }
        _ => {
            // SHA-256 (default).
            let mut context: RingContext = RingContext::new(&SHA256);
            context.update(prev_hash);
            context.update(data);
            let result: Digest = context.finish();
            let mut hash_bytes: [u8; 32] = [0u8; 32];
            hash_bytes.copy_from_slice(result.as_ref());
            hash_bytes
        }
    }
}

pub fn extend_hash_chain(prev_hash: &[u8; 32], iterations: u64) -> [u8; 32] {
    let mut current_hash: [u8; 32] = *prev_hash;
    // Short path for small iteration counts.
    if iterations < 8 {
        for _ in 0..iterations {
            current_hash = hash_single(&current_hash);
        }
        return current_hash;
    }
    // Main loop with unrolling for better pipelining.
    let mut i: u64 = 0;
    while let Some(next_i) = i.checked_add(8) {
        if next_i > iterations {
            break;
        }
        current_hash = hash_single(&current_hash);
        current_hash = hash_single(&current_hash);
        current_hash = hash_single(&current_hash);
        current_hash = hash_single(&current_hash);
        current_hash = hash_single(&current_hash);
        current_hash = hash_single(&current_hash);
        current_hash = hash_single(&current_hash);
        current_hash = hash_single(&current_hash);
        i = next_i;
    }
    // Handle remaining iterations.
    for _ in i..iterations {
        current_hash = hash_single(&current_hash);
    }
    return current_hash;
}

#[inline(always)]
fn hash_single(input: &[u8; 32]) -> [u8; 32] {
    match get_hash_algorithm() {
        1 => {
            // BLAKE3.
            let mut hasher: Blake3Hasher = Blake3Hasher::new();
            hasher.update(input);
            *hasher.finalize().as_bytes()
        }
        _ => {
            // SHA-256 (default).
            let mut context: RingContext = RingContext::new(&SHA256);
            context.update(input);
            let result: Digest = context.finish();
            let mut hash_bytes: [u8; 32] = [0u8; 32];
            hash_bytes.copy_from_slice(result.as_ref());
            hash_bytes
        }
    }
}

pub fn verify_hash_chain(prev_hash: &[u8; 32], next_hash: &[u8; 32], iterations: u64, event_data: Option<&[u8]>) -> bool {
    let mut expected_hash: [u8; 32] = *prev_hash;
    // If there's event data, hash it with the previous hash first.
    if let Some(data) = event_data {
        expected_hash = hash_with_data(&expected_hash, data);
    }
    // Extend the hash chain by the specified number of iterations.
    expected_hash = extend_hash_chain(&expected_hash, iterations);
    // Constant-time comparison to prevent timing attacks.
    return constant_time_eq(&expected_hash, next_hash);
}

#[inline]
fn constant_time_eq(a: &[u8; 32], b: &[u8; 32]) -> bool {
    let mut result: u8 = 0;
    for i in 0..32 {
        result |= a[i] ^ b[i];
    }
    return result == 0;
}

#[inline]
pub fn compute_hashes(iterations: u64) {
    // Use a zero-initialized hash as starting point.
    let zero_hash: [u8; 32] = [0u8; 32];
    let _ = extend_hash_chain(&zero_hash, iterations);
}

pub fn set_hash_algorithm(algorithm: u8) {
    return unsafe {
        DEFAULT_HASH = match algorithm {
            0 => 0, // SHA-256.
            1 => 1, // BLAKE3.
            _ => 0, // Default to SHA-256.
        };
    };
}

pub fn get_current_algorithm() -> u8 {
    return get_hash_algorithm();
}

pub fn get_algorithm_name() -> &'static str {
    match get_hash_algorithm() {
        1 => "BLAKE3",
        _ => "SHA-256",
    }
}
