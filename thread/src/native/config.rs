use std::thread as std_thread;

use crate::native::types::{Config, CoreAllocation};

use anyhow::{Result, bail};

impl Default for Config {
    fn default() -> Self {
        return Self {
            core_allocation: CoreAllocation::OsDefault,
            max_threads: std_thread::available_parallelism().map_or(16, |p| p.get()),
            priority: 0,
            stack_size_bytes: 2 * 1024 * 1024, // 2 MB.
        };
    }
}

impl Config {
    /// Validates the thread configuration.
    pub fn validate(&self) -> Result<()> {
        if self.max_threads == 0 {
            bail!("max_threads must be greater than 0.");
        }
        if self.stack_size_bytes < 64 * 1024 {
            bail!("stack_size_bytes must be at least 64KB.");
        }
        return self.core_allocation.validate();
    }
}
