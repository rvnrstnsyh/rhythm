use std::thread as std_thread;

use crate::native::{
    platform::{set_affinity, set_priority},
    types::CoreAllocation,
};

use anyhow::Result;

/// Apply thread policy based on core allocation and priority.
pub fn apply_policy(alloc: &CoreAllocation, priority: u8, cores: &[usize]) -> Result<()> {
    // Set thread priority.
    set_priority(priority)?;
    // Apply core affinity based on allocation strategy.
    match alloc {
        CoreAllocation::PinnedCores { .. } => {
            if !cores.is_empty() {
                // For pinned cores, we pick one core for this thread.
                // Use a stable approach to select a core without using thread::current().id().as_u64().
                // Generate a pseudo-random core selection based on thread name hash.
                let thread_name: String = std_thread::current().name().unwrap_or("unknown").to_string();
                let name_hash: u64 = thread_name.bytes().fold(0u64, |acc, b| acc.wrapping_add(b as u64));
                #[allow(clippy::arithmetic_side_effects)]
                let core_idx: usize = if cores.is_empty() {
                    0
                } else {
                    let len: u64 = cores.len() as u64;
                    (name_hash % len) as usize
                };
                set_affinity(&[cores[core_idx]])?;
            }
        }
        CoreAllocation::DedicatedCoreSet { .. } => {
            // For dedicated core set, use all cores in the set.
            if !cores.is_empty() {
                set_affinity(cores)?;
            }
        }
        CoreAllocation::OsDefault => {
            // Let OS handle thread scheduling.
        }
    }
    return Ok(());
}
