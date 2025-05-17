use crate::native::types::CoreAllocation;

use anyhow::{Result, bail};

impl Default for CoreAllocation {
    fn default() -> Self {
        return Self::OsDefault;
    }
}

impl CoreAllocation {
    /// Converts into a vector of core IDs.
    pub fn as_core_mask_vector(&self) -> Vec<usize> {
        return match *self {
            CoreAllocation::PinnedCores { min, max } | CoreAllocation::DedicatedCoreSet { min, max } => {
                if min > max {
                    vec![]
                } else {
                    (min..=max).collect()
                }
            }
            CoreAllocation::OsDefault => (0..num_cpus::get()).collect(),
        };
    }

    /// Validates the core allocation configuration.
    pub fn validate(&self) -> Result<()> {
        return match self {
            CoreAllocation::PinnedCores { min, max } | CoreAllocation::DedicatedCoreSet { min, max } => {
                if *min > *max {
                    bail!("Invalid core range: min({}) > max({}).", min, max);
                }
                if *max >= num_cpus::get() {
                    bail!("Max core ID ({}) exceeds available cores ({}).", max, num_cpus::get().saturating_sub(1));
                }
                Ok(())
            }
            CoreAllocation::OsDefault => Ok(()),
        };
    }
}
