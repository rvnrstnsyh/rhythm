use std::{
    collections::VecDeque,
    sync::{
        Arc, Condvar, Mutex, MutexGuard,
        atomic::{AtomicBool, AtomicUsize, Ordering},
    },
    thread,
    time::{Duration, Instant},
};

use crate::native_runtime::types::{Config, Job, JobFn, JobOption, JobQueue, JoinHandle, Native, ThreadPool, ThreadPoolStats};

use anyhow::{Result, bail};

impl ThreadPool {
    pub fn new(name: String, config: Config) -> Result<Self> {
        // Validate configuration.
        config.validate()?;

        let worker: Native = Native::new(name, config.clone())?;
        let job_queue: JobQueue = Arc::new(Mutex::new(VecDeque::with_capacity(32))); // Pre-allocate some capacity.
        let signal: Arc<Condvar> = Arc::new(Condvar::new());
        let shutdown: Arc<AtomicBool> = Arc::new(AtomicBool::new(false));
        let active_workers: Arc<AtomicUsize> = Arc::new(AtomicUsize::new(0));
        let completed_jobs: Arc<AtomicUsize> = Arc::new(AtomicUsize::new(0));
        let stats: Arc<Mutex<ThreadPoolStats>> = Arc::new(Mutex::new(ThreadPoolStats::default()));

        let mut workers: Vec<JoinHandle<()>> = Vec::with_capacity(config.max_threads);

        // Create worker threads.
        for id in 0..config.max_threads {
            workers.push(Self::spawn_worker(
                &worker,
                id,
                job_queue.clone(),
                signal.clone(),
                shutdown.clone(),
                active_workers.clone(),
                completed_jobs.clone(),
                stats.clone(),
            )?);
        }

        return Ok(Self {
            worker,
            job_queue,
            signal,
            shutdown,
            active_workers,
            completed_jobs,
            workers,
            stats,
        });
    }

    fn spawn_worker(
        worker: &Native,
        id: usize,
        job_queue: Arc<Mutex<VecDeque<Job>>>,
        signal: Arc<Condvar>,
        shutdown: Arc<AtomicBool>,
        active_workers: Arc<AtomicUsize>,
        completed_jobs: Arc<AtomicUsize>,
        stats: Arc<Mutex<ThreadPoolStats>>,
    ) -> Result<JoinHandle<()>> {
        let worker_name: String = format!("worker-{}", id);

        worker.spawn_named(worker_name, move || {
            let mut consecutive_idle: i32 = 0;

            while !shutdown.load(Ordering::Acquire) {
                // Wait for a job or shutdown signal.
                let job_option: JobOption = {
                    let mut queue: MutexGuard<'_, VecDeque<JobFn>> = job_queue.lock().unwrap();
                    // Update peak queue size in stats.
                    {
                        let mut pool_stats: MutexGuard<'_, ThreadPoolStats> = stats.lock().unwrap();
                        pool_stats.peak_queue_size = pool_stats.peak_queue_size.max(queue.len());
                    }
                    // Use a short timeout when queue is empty to check for shutdown periodically.
                    if queue.is_empty() {
                        consecutive_idle = consecutive_idle.saturating_add(1);
                        // Exponential backoff with a maximum wait time.
                        let wait_time: Duration = if consecutive_idle > 10 {
                            Duration::from_millis(100)
                        } else {
                            Duration::from_millis((1 << consecutive_idle.min(6)) as u64)
                        };

                        let (new_queue, timeout_result) = signal.wait_timeout(queue, wait_time).unwrap();
                        queue = new_queue;
                        // If we timed out and still nothing to do, continue the loop to check shutdown flag.
                        if timeout_result.timed_out() && queue.is_empty() {
                            continue;
                        }
                    }
                    // If shutting down and the queue is empty, exit the loop.
                    if shutdown.load(Ordering::Acquire) && queue.is_empty() {
                        break;
                    }
                    // Get a job from the queue.
                    if !queue.is_empty() {
                        consecutive_idle = 0; // Reset consecutive idle counter.
                        queue.pop_front()
                    } else {
                        None
                    }
                };

                // Execute the job if we got one.
                if let Some(job) = job_option {
                    let active_count: usize = active_workers.fetch_add(1, Ordering::AcqRel).saturating_add(1);
                    let start_time: Instant = Instant::now();
                    // Update peak active workers stat.
                    {
                        let mut pool_stats: MutexGuard<'_, ThreadPoolStats> = stats.lock().unwrap();
                        pool_stats.peak_active_workers = pool_stats.peak_active_workers.max(active_count);
                    }
                    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(job));
                    let processing_time: Duration = start_time.elapsed();

                    active_workers.fetch_sub(1, Ordering::Release);
                    completed_jobs.fetch_add(1, Ordering::Release);

                    // Update statistics.
                    let mut pool_stats: std::sync::MutexGuard<'_, ThreadPoolStats> = stats.lock().unwrap();

                    pool_stats.total_jobs_completed = pool_stats.total_jobs_completed.saturating_add(1);
                    pool_stats.total_processing_time = pool_stats.total_processing_time.saturating_add(processing_time);

                    // Calculate average processing time - avoid division by zero.
                    if pool_stats.total_jobs_completed > 0 {
                        let avg_nanos: u128 = pool_stats
                            .total_processing_time
                            .as_nanos()
                            .checked_div(pool_stats.total_jobs_completed as u128)
                            .unwrap_or(0);
                        pool_stats.avg_processing_time = Some(Duration::from_nanos(avg_nanos as u64));
                    }
                    // Log failures.
                    if result.is_err() {
                        pool_stats.failed_jobs = pool_stats.failed_jobs.saturating_add(1);
                        eprintln!("Worker thread caught a panic while executing job.");
                    } else if let Err(e) = result.unwrap() {
                        pool_stats.failed_jobs = pool_stats.failed_jobs.saturating_add(1);
                        eprintln!("Job execution failed with error: {}", e);
                    }
                }
            }
        })
    }

    /// Execute a function on the thread pool.
    ///
    /// # Arguments
    /// * `f` - The function to execute.
    /// # Returns
    /// `Ok(())` if the job was successfully queued, or an error if the pool is shutting down.
    pub fn execute<F: FnOnce() -> Result<()> + Send + 'static>(&self, f: F) -> Result<()> {
        if self.shutdown.load(Ordering::Acquire) {
            bail!("Thread pool is shutting down.");
        }

        {
            let mut queue: MutexGuard<'_, VecDeque<JobFn>> = self.job_queue.lock().unwrap();
            queue.push_back(Box::new(f));
        }
        // Notify one worker that a job is available.
        self.signal.notify_one();
        return Ok(());
    }

    /// Execute a batch of similar functions on the thread pool.
    /// This is more efficient than calling execute() multiple times.
    ///
    /// # Arguments
    /// * `jobs` - Iterator of functions to execute
    /// # Returns
    /// The number of jobs queued, or an error if the pool is shutting down.
    pub fn execute_batch<F: FnOnce() -> Result<()> + Send + 'static, I: IntoIterator<Item = F>>(&self, jobs: I) -> Result<usize> {
        if self.shutdown.load(Ordering::Acquire) {
            bail!("Thread pool is shutting down.");
        }

        let mut count: usize = 0;

        {
            let mut queue: MutexGuard<'_, VecDeque<JobFn>> = self.job_queue.lock().unwrap();
            for job in jobs {
                queue.push_back(Box::new(job));
                count = count.saturating_add(1);
            }
        }

        // Notify workers that jobs are available - wake up enough workers for the jobs.
        for _ in 0..count.min(self.worker_count()) {
            self.signal.notify_one();
        }

        return Ok(count);
    }

    /// Execute a function on the thread pool and wait for it to complete.
    ///
    /// # Arguments
    /// * `f` - The function to execute
    /// # Returns
    /// The result of the function execution.
    pub fn execute_wait<F: FnOnce() -> Result<R> + Send + 'static, R: Send + 'static>(&self, f: F) -> Result<R> {
        if self.shutdown.load(Ordering::Acquire) {
            bail!("Thread pool is shutting down.");
        }

        let result: Arc<Mutex<Option<std::result::Result<R, anyhow::Error>>>> = Arc::new(Mutex::new(None));
        let result_clone: Arc<Mutex<Option<std::result::Result<R, anyhow::Error>>>> = result.clone();
        let done_signal: Arc<Condvar> = Arc::new(Condvar::new());
        let done_signal_clone: Arc<Condvar> = done_signal.clone();
        let done_flag: Arc<AtomicBool> = Arc::new(AtomicBool::new(false));
        let done_flag_clone: Arc<AtomicBool> = done_flag.clone();

        self.execute(move || {
            let job_result: std::result::Result<R, anyhow::Error> = f();
            let mut slot: MutexGuard<'_, Option<std::result::Result<R, anyhow::Error>>> = result_clone.lock().unwrap();

            *slot = Some(job_result);
            done_flag_clone.store(true, Ordering::Release);
            done_signal_clone.notify_all(); // Notify all in case multiple threads are waiting.

            return Ok(());
        })?;

        // Wait for the job to complete.
        let mut guard: MutexGuard<'_, Option<std::result::Result<R, anyhow::Error>>> = result.lock().unwrap();
        while !done_flag.load(Ordering::Acquire) {
            guard = done_signal.wait(guard).unwrap();
        }

        // Extract and return the result.
        return match guard.take().unwrap() {
            Ok(value) => Ok(value),
            Err(e) => Err(e),
        };
    }

    /// Gracefully shut down the thread pool, waiting for all jobs to complete.
    pub fn shutdown(&self) -> Result<()> {
        self.shutdown.store(true, Ordering::Release);
        self.signal.notify_all(); // Notify all workers to check shutdown flag.
        return Ok(());
    }

    /// Forcefully shut down the thread pool, discarding pending jobs.
    pub fn shutdown_now(&mut self) -> Result<()> {
        self.shutdown.store(true, Ordering::Release);

        // Clear the job queue.
        {
            let mut queue: MutexGuard<'_, VecDeque<JobFn>> = self.job_queue.lock().unwrap();
            let job_count: usize = queue.len();

            queue.clear();

            if job_count > 0 {
                eprintln!("Warning: Discarded {} pending jobs during forced shutdown.", job_count);
            }
        }

        self.signal.notify_all();
        return Ok(());
    }

    /// Wait for all worker threads to finish.
    pub fn join(mut self) -> Result<ThreadPoolStats> {
        // Ensure the pool is shutting down.
        self.shutdown()?;
        // Join all worker threads.
        let mut worker_panics: usize = 0;

        for worker in self.workers.drain(..) {
            let worker_name: String = worker.name().to_string();
            if let Err(e) = worker.join() {
                worker_panics = worker_panics.saturating_add(1);
                eprintln!("Worker thread '{}' panicked during shutdown: {:?}.", worker_name, e);
            }
        }
        // Update stats with any worker panics
        if worker_panics > 0 {
            let mut stats = self.stats.lock().unwrap();
            stats.failed_jobs = stats.failed_jobs.saturating_add(worker_panics);
        }

        // Return the final stats.
        let stats: ThreadPoolStats = self.stats.lock().unwrap().clone();
        return Ok(stats);
    }

    /// Get the number of worker threads.
    pub fn worker_count(&self) -> usize {
        return self.workers.len();
    }

    /// Get the number of currently active worker threads.
    pub fn active_worker_count(&self) -> usize {
        return self.active_workers.load(Ordering::Acquire);
    }

    /// Get the number of pending jobs in the queue.
    pub fn queued_job_count(&self) -> usize {
        return self.job_queue.lock().unwrap().len();
    }

    /// Get the number of completed jobs.
    pub fn completed_job_count(&self) -> usize {
        return self.completed_jobs.load(Ordering::Acquire);
    }

    /// Get current statistics about the thread pool.
    pub fn stats(&self) -> ThreadPoolStats {
        return self.stats.lock().unwrap().clone();
    }

    /// Check if the thread pool is shutting down.
    pub fn is_shutting_down(&self) -> bool {
        return self.shutdown.load(Ordering::Acquire);
    }

    /// Get a reference to the underlying thread native.
    pub fn thread_native(&self) -> &Native {
        return &self.worker;
    }

    /// Wait for all currently queued jobs to complete.
    pub fn wait_for_completion(&self) -> Result<()> {
        if self.is_shutting_down() {
            bail!("Cannot wait for completion on a shutting down pool.");
        }
        // Keep checking until queue is empty and no active workers.
        while self.queued_job_count() > 0 || self.active_worker_count() > 0 {
            thread::sleep(Duration::from_millis(10));
        }
        return Ok(());
    }

    pub fn default_pool(name: &str) -> Result<ThreadPool> {
        return ThreadPool::new(name.to_string(), Config::default());
    }
}
