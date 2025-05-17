mod core;
mod formats;
mod types;

pub mod digest;

pub use crate::types::{PoH, PoHRecord};

// 0: SHA-256 (default) 1: BLAKE3
pub static mut DEFAULT_HASH: u8 = 0;
// Number of seconds per day.
pub const DEFAULT_SECONDS_PER_DAY: u64 = 24 * 60 * 60;
// Number of revs per second.
pub const DEFAULT_REVS_PER_SECOND: u64 = 160;
// Number of revs per day.
pub const DEFAULT_REVS_PER_DAY: u64 = DEFAULT_REVS_PER_SECOND * DEFAULT_SECONDS_PER_DAY;
// Rev duration in milliseconds (1000 / 160 = 6.25ms approximated as 6ms base).
pub const DEFAULT_MS_PER_REV: u64 = 1_000 / DEFAULT_REVS_PER_SECOND;
// Additional timing tolerance per rev in microseconds (0.25ms).
pub const DEFAULT_US_TOLERANCE_PER_REV: u64 = 250;
// Final rev duration in microseconds: ~6ms + 0.25ms = 6250μs.
pub const DEFAULT_US_PER_REV: u64 = (DEFAULT_MS_PER_REV * 1000) + DEFAULT_US_TOLERANCE_PER_REV;
// Number of revs per phase (64 revs = 1 phase).
pub const DEFAULT_REVS_PER_PHASE: u64 = 64;
// GCP n1-standard hardware and also a xeon e5-2520 v4 are about this rate of hashes/s.
pub const DEFAULT_HASHES_PER_SECOND: u64 = 2_000_000;
// Number of hashes per rev.
pub const DEFAULT_HASHES_PER_REV: u64 = DEFAULT_HASHES_PER_SECOND / DEFAULT_REVS_PER_SECOND;
// Expected duration of a phase in seconds.
pub const DEFAULT_S_PER_PHASE: f64 = DEFAULT_REVS_PER_PHASE as f64 / DEFAULT_REVS_PER_SECOND as f64;
// Expected duration of a phase (400 milliseconds).
pub const DEFAULT_MS_PER_PHASE: u64 = 1_000 * DEFAULT_REVS_PER_PHASE / DEFAULT_REVS_PER_SECOND;
// Number of phases per cycle (432000 phases = 1 cycle).
pub const DEFAULT_PHASES_PER_CYCLE: u64 = 2 * DEFAULT_REVS_PER_DAY / DEFAULT_REVS_PER_PHASE;
// 1 Dev Cycle = 400 ms * 8192 ~= 55 minutes.
pub const DEFAULT_DEV_PHASES_PER_CYCLE: u64 = 8_192;
// leader schedule is governed by this.
pub const DEFAULT_NUM_CONSECUTIVE_LEADER_PHASES: u64 = 4;
// Channel capacity for the PoH thread.
pub const DEFAULT_CHANNEL_CAPACITY: usize = 1_000;
// Batch size for sending PoH records.
pub const DEFAULT_BATCH_SIZE: usize = 64;
// Use spinlock for precise timing under threshold.
pub const DEFAULT_SPINLOCK_THRESHOLD_US: u64 = 250;
