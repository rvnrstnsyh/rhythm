use std::sync::atomic::{AtomicBool, Ordering};

use anyhow::Result;

static AFFINITY_WARNING_SHOWN: AtomicBool = AtomicBool::new(false);
static PRIORITY_WARNING_SHOWN: AtomicBool = AtomicBool::new(false);

pub fn set_affinity(cores: &[usize]) -> Result<()> {
    if cores.is_empty() {
        return Ok(());
    }
    #[cfg(target_os = "linux")]
    {
        #[allow(unused_imports)]
        use affinity::*;
        use anyhow::anyhow;

        set_thread_affinity(cores).map_err(|e| {
            if !AFFINITY_WARNING_SHOWN.swap(true, Ordering::Relaxed) {
                eprintln!("Warning: Thread affinity operations may require elevated privileges.");
            }
            anyhow!("Failed to set thread affinity: {}", e)
        })?;
        return Ok(());
    }
    #[cfg(not(target_os = "linux"))]
    {
        if !AFFINITY_WARNING_SHOWN.swap(true, Ordering::Relaxed) {
            eprintln!("Warning: Thread affinity is only supported on Linux platforms.");
        }
        return Ok(());
    }
    #[allow(unreachable_code)]
    return Ok(());
}

pub fn set_priority(value: u8) -> Result<()> {
    if value == 0 {
        return Ok(());
    }
    #[cfg(target_os = "linux")]
    {
        use anyhow::anyhow;
        use std::convert::TryFrom;
        use thread_priority::*;

        let priority_value: ThreadPriorityValue = ThreadPriorityValue::try_from(value).map_err(anyhow::Error::msg)?;
        let priority: ThreadPriority = ThreadPriority::Crossplatform(priority_value);
        let policy: ThreadSchedulePolicy = ThreadSchedulePolicy::Normal(NormalThreadSchedulePolicy::Other);

        // Add & to pass a reference to the thread.
        ThreadExt::set_priority_and_policy(&std::thread::current(), policy, priority).map_err(|e| {
            if !PRIORITY_WARNING_SHOWN.swap(true, Ordering::Relaxed) {
                eprintln!("Warning: Thread priority operations may require elevated privileges.");
            }
            anyhow!("Failed to set thread priority: {:?}", e)
        })?;
    }
    #[cfg(not(target_os = "linux"))]
    {
        if !PRIORITY_WARNING_SHOWN.swap(true, Ordering::Relaxed) {
            eprintln!("Warning: Thread priority is only supported on Linux platforms.");
        }
    }
    return Ok(());
}
