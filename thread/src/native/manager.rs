use std::{
    sync::{
        Arc, Mutex,
        atomic::{AtomicUsize, Ordering},
    },
    thread as std_thread,
};

use crate::native::policy::apply_policy;
use crate::native::types::{Config, CoreAllocation, JoinHandle, Manager, ManagerInner};

use anyhow::{Context, Result, bail};

pub const MAX_THREAD_NAME_CHARS: usize = 32;

impl Manager {
    pub fn new(name: String, config: Config) -> Result<Self> {
        if name.len() >= MAX_THREAD_NAME_CHARS {
            bail!("Thread name too long (max {} chars).", MAX_THREAD_NAME_CHARS);
        }

        // Validate configuration.
        config.validate()?;

        let cores_mask: Vec<usize> = config.core_allocation.as_core_mask_vector();

        return Ok(Self {
            inner: Arc::new(ManagerInner {
                id_count: AtomicUsize::new(0),
                running_count: Arc::new(AtomicUsize::new(0)),
                cores_mask: Mutex::new(cores_mask),
                config,
                name,
            }),
        });
    }

    pub fn spawn<F, T>(&self, f: F) -> Result<JoinHandle<T>>
    where
        F: FnOnce() -> T,
        F: Send + 'static,
        T: Send + 'static,
    {
        let n: usize = self.inner.id_count.fetch_add(1, Ordering::Relaxed);
        let name: String = format!("{}-{}", &self.inner.name, n);
        self.spawn_named(name, f)
    }

    pub fn spawn_named<F, T>(&self, name: String, f: F) -> Result<JoinHandle<T>>
    where
        F: FnOnce() -> T,
        F: Send + 'static,
        T: Send + 'static,
    {
        if name.len() >= MAX_THREAD_NAME_CHARS {
            bail!("Thread name too long (max {} chars.)", MAX_THREAD_NAME_CHARS);
        }

        let spawned: usize = self.inner.running_count.load(Ordering::Acquire);
        if spawned >= self.inner.config.max_threads {
            bail!("All allowed threads in this pool are already spawned (max: {}).", self.inner.config.max_threads);
        }

        let core_alloc: CoreAllocation = self.inner.config.core_allocation.clone();
        let priority: u8 = self.inner.config.priority;

        // Get a reference to the core mask - minimize lock holding time.
        let chosen_cores_mask: Vec<usize> = {
            let cores: std::sync::MutexGuard<'_, Vec<usize>> = self.inner.cores_mask.lock().unwrap();
            cores.clone()
        };

        // Clone necessary data for the thread.
        let running_count: Arc<AtomicUsize> = self.inner.running_count.clone();

        let thread_name: String = name.clone();
        let jh: std_thread::JoinHandle<T> = std_thread::Builder::new()
            .name(name.clone())
            .stack_size(self.inner.config.stack_size_bytes)
            .spawn(move || {
                // Apply thread configuration.
                apply_policy(&core_alloc, priority, &chosen_cores_mask).unwrap_or_else(|e| {
                    eprintln!("Warning in thread '{}': {}", thread_name, e);
                });
                // Run the actual thread function.
                f()
            })
            .with_context(|| format!("Failed to spawn thread '{}'.", name))?;

        self.inner.running_count.fetch_add(1, Ordering::Release);

        return Ok(JoinHandle {
            std_handle: Some(jh),
            running_count,
            name,
        });
    }

    /// Get the current number of running threads.
    pub fn running_count(&self) -> usize {
        return self.inner.running_count.load(Ordering::Acquire);
    }

    /// Check if the thread pool is full.
    pub fn is_full(&self) -> bool {
        return self.running_count() >= self.inner.config.max_threads;
    }

    /// Get a reference to the thread manager's configuration.
    pub fn config(&self) -> &Config {
        return &self.inner.config;
    }

    /// Get the thread manager's name.
    pub fn name(&self) -> &str {
        return &self.inner.name;
    }

    /// Get the number of threads that can still be spawned.
    pub fn available_slots(&self) -> usize {
        return self.inner.config.max_threads.saturating_sub(self.running_count());
    }
}

pub fn default_manager(name: &str) -> Result<Manager> {
    return Manager::new(name.to_string(), Config::default());
}
