use blake3::Hasher as Blake3Hasher;
use ring::digest::{Context as RingContext, Digest, SHA256, digest};

#[derive(Debug, Default, Eq, Clone, Copy, PartialEq)]
pub enum Algorithm {
    #[default]
    SHA256 = 0,
    BLAKE3 = 1,
}

impl From<u8> for Algorithm {
    fn from(value: u8) -> Self {
        return match value {
            1 => Algorithm::BLAKE3,
            _ => Algorithm::SHA256, // Default to SHA-256 for any other value.
        };
    }
}

impl From<Algorithm> for u8 {
    fn from(value: Algorithm) -> Self {
        return value as u8;
    }
}

impl Algorithm {
    pub fn name(&self) -> &'static str {
        return match self {
            Algorithm::SHA256 => "SHA-256",
            Algorithm::BLAKE3 => "BLAKE3",
        };
    }
}

#[derive(Default)]
pub struct Hasher {
    algorithm: Algorithm,
}

impl Hasher {
    pub fn new(algorithm: Algorithm) -> Self {
        return Self { algorithm };
    }

    pub fn algorithm(&self) -> Algorithm {
        return self.algorithm;
    }

    pub fn set_algorithm(&mut self, algorithm: Algorithm) {
        return self.algorithm = algorithm;
    }

    pub fn algorithm_name(&self) -> &'static str {
        return self.algorithm.name();
    }

    #[inline]
    pub fn hash(&self, data: &[u8]) -> [u8; 32] {
        return match self.algorithm {
            Algorithm::BLAKE3 => *blake3::hash(data).as_bytes(),
            Algorithm::SHA256 => {
                let hash_result: Digest = digest(&SHA256, data);
                let mut hash_bytes: [u8; 32] = [0u8; 32];
                hash_bytes.copy_from_slice(hash_result.as_ref());
                hash_bytes
            }
        };
    }

    #[inline]
    pub fn embed_data(&self, previous_hash: &[u8; 32], data: &[u8]) -> [u8; 32] {
        return match self.algorithm {
            Algorithm::BLAKE3 => {
                let mut hasher: Blake3Hasher = Blake3Hasher::new();
                hasher.update(previous_hash);
                hasher.update(data);
                *hasher.finalize().as_bytes()
            }
            Algorithm::SHA256 => {
                let mut context: RingContext = RingContext::new(&SHA256);
                context.update(previous_hash);
                context.update(data);
                let result: Digest = context.finish();
                let mut hash_bytes: [u8; 32] = [0u8; 32];
                hash_bytes.copy_from_slice(result.as_ref());
                hash_bytes
            }
        };
    }

    #[inline(always)]
    pub fn previous_hash(&self, hash: &[u8; 32]) -> [u8; 32] {
        return match self.algorithm {
            Algorithm::BLAKE3 => {
                let mut hasher: Blake3Hasher = Blake3Hasher::new();
                hasher.update(hash);
                *hasher.finalize().as_bytes()
            }
            Algorithm::SHA256 => {
                let mut context: RingContext = RingContext::new(&SHA256);
                context.update(hash);
                let result: Digest = context.finish();
                let mut hash_bytes: [u8; 32] = [0u8; 32];
                hash_bytes.copy_from_slice(result.as_ref());
                hash_bytes
            }
        };
    }

    pub fn extend_hash_chain(&self, previous_hash: &[u8; 32], iterations: u64) -> [u8; 32] {
        let mut current_hash: [u8; 32] = *previous_hash;

        // Short path for small iteration counts.
        if iterations < 8 {
            for _ in 0..iterations {
                current_hash = self.previous_hash(&current_hash);
            }
            return current_hash;
        }

        // Main loop with unrolling for better pipelining.
        let mut i: u64 = 0;
        while let Some(next_i) = i.checked_add(8) {
            if next_i > iterations {
                break;
            }
            current_hash = self.previous_hash(&current_hash);
            current_hash = self.previous_hash(&current_hash);
            current_hash = self.previous_hash(&current_hash);
            current_hash = self.previous_hash(&current_hash);
            current_hash = self.previous_hash(&current_hash);
            current_hash = self.previous_hash(&current_hash);
            current_hash = self.previous_hash(&current_hash);
            current_hash = self.previous_hash(&current_hash);
            i = next_i;
        }

        // Handle remaining iterations.
        for _ in i..iterations {
            current_hash = self.previous_hash(&current_hash);
        }

        return current_hash;
    }

    pub fn verify_hash_chain(&self, previous_hash: &[u8; 32], next_hash: &[u8; 32], iterations: u64, event_data: Option<&[u8]>) -> bool {
        let mut expected_hash: [u8; 32] = *previous_hash;

        // If there's event data, hash it with the previous hash first.
        if let Some(data) = event_data {
            expected_hash = self.embed_data(&expected_hash, data);
        }

        // Extend the hash chain by the specified number of iterations.
        expected_hash = self.extend_hash_chain(&expected_hash, iterations);
        // Constant-time comparison to prevent timing attacks.
        return self.constant_time_eq(&expected_hash, next_hash);
    }

    pub fn compute_hashes(&self, iterations: u64) {
        // Use a zero-initialized hash as starting point.
        let zero_hash: [u8; 32] = [0u8; 32];
        let _ = self.extend_hash_chain(&zero_hash, iterations);
    }

    #[inline]
    fn constant_time_eq(&self, a: &[u8; 32], b: &[u8; 32]) -> bool {
        let mut result: u8 = 0;
        for i in 0..32 {
            result |= a[i] ^ b[i];
        }
        return result == 0;
    }
}
