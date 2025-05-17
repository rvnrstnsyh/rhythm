use std::time::Instant;

use crate::formats;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PoHRecord {
    #[serde(with = "formats")]
    pub hash: [u8; 32],
    pub rev_index: u64,
    pub phase_index: u64,
    pub cycle_index: u64,
    pub timestamp_ms: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub event: Option<Vec<u8>>,
}

pub struct PoH {
    pub current_hash: [u8; 32],
    pub rev_count: u64,
    pub phase_count: u64,
    pub cycle_count: u64,
    pub start_time: Instant,
    pub next_rev_target_us: u64,
}
