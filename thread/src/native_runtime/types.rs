use std::{
    collections::VecDeque,
    sync::{
        Arc, Condvar, Mutex, MutexGuard,
        atomic::{AtomicBool, AtomicUsize},
    },
    thread,
    time::Duration,
};

use anyhow::{Error, Result};
use serde::{Deserialize, Serialize};

pub type JobFn = Box<dyn FnOnce() -> std::result::Result<(), Error> + Send + 'static>;
pub type JobOption = Option<JobFn>;
pub type JobQueue = Arc<Mutex<VecDeque<JobFn>>>;
pub type OptionalJoinHandle = Option<JoinHandle<()>>;
pub type SharedJoinHandle = Arc<Mutex<OptionalJoinHandle>>;
pub type ThreadHandlePool = Mutex<Vec<SharedJoinHandle>>;
pub type ThreadHandleGuard<'a> = MutexGuard<'a, Vec<SharedJoinHandle>>;

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub enum CoreAllocation {
    /// Use OS default allocation (do not alter core affinity).
    OsDefault,
    /// Pin each thread to a core in given range.
    PinnedCores { min: usize, max: usize },
    /// Pin all threads to a set of cores.
    DedicatedCoreSet { min: usize, max: usize },
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default)]
pub struct Config {
    pub core_allocation: CoreAllocation,
    pub max_threads: usize,
    pub priority: u8,
    pub stack_size_bytes: usize,
}

#[derive(Debug)]
pub struct NativeInner {
    pub id_count: AtomicUsize,
    pub running_count: Arc<AtomicUsize>,
    pub config: Config,
    pub name: String,
    pub cores_mask: Mutex<Vec<usize>>,
}

#[derive(Debug, Clone)]
pub struct Native {
    pub inner: Arc<NativeInner>,
}

pub struct JoinHandle<T> {
    pub std_handle: Option<thread::JoinHandle<T>>,
    pub running_count: Arc<AtomicUsize>,
    pub name: String,
}

/// A job to be executed by the thread pool.
pub type Job = Box<dyn FnOnce() -> Result<()> + Send + 'static>;

/// A Thread Pool implementation that manages a set of worker threads
/// and distributes jobs among them.
pub struct ThreadPool {
    pub worker: Native,
    pub job_queue: Arc<Mutex<VecDeque<Job>>>,
    pub signal: Arc<Condvar>,
    pub shutdown: Arc<AtomicBool>,
    pub active_workers: Arc<AtomicUsize>,
    pub completed_jobs: Arc<AtomicUsize>,
    pub workers: Vec<JoinHandle<()>>,
    pub stats: Arc<Mutex<ThreadPoolStats>>,
}

/// Statistics for thread pool monitoring.
#[derive(Debug, Clone, Default)]
pub struct ThreadPoolStats {
    pub total_jobs_completed: usize,
    pub total_processing_time: Duration,
    pub peak_queue_size: usize,
    pub avg_processing_time: Option<Duration>,
    pub failed_jobs: usize,
    pub peak_active_workers: usize,
}
