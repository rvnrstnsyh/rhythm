#[cfg(test)]
mod thread_native_operations {
    use std::{
        any::Any,
        sync::{
            Arc, Mutex, MutexGuard,
            atomic::{AtomicUsize, Ordering},
        },
        thread as std_thread,
        time::Duration,
    };

    use anyhow::Result;
    use thread::native_runtime::types::{Config, CoreAllocation, JoinHandle, Native, ThreadPool, ThreadPoolStats};

    #[test]
    fn thread_worker_basic() -> Result<()> {
        let worker: Native = Native::default_thread("test-worker")?;

        assert_eq!(worker.name(), "test-worker");
        assert_eq!(worker.running_count(), 0);
        assert!(!worker.is_full());

        let counter: Arc<AtomicUsize> = Arc::new(AtomicUsize::new(0));
        let counter_clone: Arc<AtomicUsize> = counter.clone();
        let handle: JoinHandle<&'static str> = worker.spawn(move || {
            counter_clone.fetch_add(1, Ordering::SeqCst);
            std_thread::sleep(Duration::from_millis(50));
            counter_clone.fetch_add(1, Ordering::SeqCst);
            "test result"
        })?;

        assert_eq!(worker.running_count(), 1);

        // Wait for thread to complete.
        let result: &'static str = handle.join().unwrap();
        assert_eq!(result, "test result");
        assert_eq!(counter.load(Ordering::SeqCst), 2);
        assert_eq!(worker.running_count(), 0);

        return Ok(());
    }

    #[test]
    fn thread_worker_named() -> Result<()> {
        let worker: Native = Native::default_thread("test-worker")?;

        let shared_data: Arc<Mutex<Vec<String>>> = Arc::new(Mutex::new(Vec::new()));
        let shared_data_clone: Arc<Mutex<Vec<String>>> = shared_data.clone();

        let handle: JoinHandle<&'static str> = worker.spawn_named("custom-worker".to_string(), move || {
            let thread_name: String = std_thread::current().name().unwrap_or("unknown").to_string();
            let mut data: MutexGuard<'_, Vec<String>> = shared_data_clone.lock().unwrap();
            data.push(thread_name);
            "done"
        })?;

        let result: &'static str = handle.join().unwrap();
        let data: MutexGuard<'_, Vec<String>> = shared_data.lock().unwrap();

        assert_eq!(result, "done");
        assert_eq!(data.len(), 1);
        assert_eq!(data[0], "custom-worker");

        return Ok(());
    }

    #[test]
    fn thread_worker_max_threads() -> Result<()> {
        let config: Config = Config {
            max_threads: 2,
            ..Default::default()
        };
        let worker: Native = Native::new("limited-worker".to_string(), config)?;
        let handle1: JoinHandle<i32> = worker.spawn(|| {
            std_thread::sleep(Duration::from_millis(50));
            1
        })?;
        let handle2: JoinHandle<i32> = worker.spawn(|| {
            std_thread::sleep(Duration::from_millis(50));
            2
        })?;

        assert_eq!(worker.running_count(), 2);
        assert!(worker.is_full());

        // Try to spawn a third thread - should fail.
        let result: Result<JoinHandle<i32>, anyhow::Error> = worker.spawn(|| {
            std_thread::sleep(Duration::from_millis(50));
            3
        });

        assert!(result.is_err());
        // Join threads.
        assert_eq!(handle1.join().unwrap(), 1);
        assert_eq!(handle2.join().unwrap(), 2);
        assert_eq!(worker.running_count(), 0);

        assert!(!worker.is_full());

        return Ok(());
    }

    #[test]
    fn core_allocation() -> Result<()> {
        // Test core mask conversion.
        let alloc: CoreAllocation = CoreAllocation::PinnedCores { min: 0, max: 3 };
        let cores: Vec<usize> = alloc.as_core_mask_vector();

        assert_eq!(cores, vec![0, 1, 2, 3]);

        // Test invalid range.
        let invalid: CoreAllocation = CoreAllocation::PinnedCores { min: 5, max: 3 };
        let cores: Vec<usize> = invalid.as_core_mask_vector();

        assert!(cores.is_empty());
        // Test validation.
        assert!(alloc.validate().is_ok());

        // If system has less than 100 cores, this should fail validation.
        let too_high: CoreAllocation = CoreAllocation::DedicatedCoreSet { min: 0, max: 100 };
        if num_cpus::get() <= 100 {
            assert!(too_high.validate().is_err());
        }

        let bad_range: CoreAllocation = CoreAllocation::PinnedCores { min: 5, max: 3 };
        assert!(bad_range.validate().is_err());

        return Ok(());
    }

    #[test]
    fn thread_pool_basic() -> Result<()> {
        let pool: ThreadPool = ThreadPool::default_pool("test-pool")?;
        let counter: Arc<AtomicUsize> = Arc::new(AtomicUsize::new(0));
        let counter_clone: Arc<AtomicUsize> = counter.clone();

        pool.execute(move || {
            counter_clone.fetch_add(1, Ordering::SeqCst);
            return Ok(());
        })?;

        // Allow time for task to execute.
        std_thread::sleep(Duration::from_millis(100));

        assert_eq!(counter.load(Ordering::SeqCst), 1);

        let stats: ThreadPoolStats = pool.stats();

        assert_eq!(stats.total_jobs_completed, 1);
        assert_eq!(stats.failed_jobs, 0);

        // Cleanup.
        pool.shutdown()?;

        return Ok(());
    }

    #[test]
    fn thread_pool_execute_wait() -> Result<()> {
        let pool: ThreadPool = ThreadPool::default_pool("test-pool")?;
        let result: i32 = pool.execute_wait(|| {
            std_thread::sleep(Duration::from_millis(50));
            return Ok(42);
        })?;

        assert_eq!(result, 42);

        // Test error propagation.
        let err_result: Result<(), anyhow::Error> = pool.execute_wait::<_, ()>(|| {
            anyhow::bail!("Test error");
        });

        assert!(err_result.is_err());
        assert!(err_result.unwrap_err().to_string().contains("Test error"));

        // Cleanup.
        pool.shutdown()?;

        return Ok(());
    }

    #[test]
    fn thread_pool_execute_batch() -> Result<()> {
        let pool: ThreadPool = ThreadPool::default_pool("batch-pool")?;
        let counter: Arc<AtomicUsize> = Arc::new(AtomicUsize::new(0));
        let jobs = (0..10)
            .map(|_| {
                let counter: Arc<AtomicUsize> = counter.clone();
                move || {
                    counter.fetch_add(1, Ordering::SeqCst);
                    return Ok(());
                }
            })
            .collect::<Vec<_>>();

        let count: usize = pool.execute_batch(jobs)?;

        assert_eq!(count, 10);

        // Wait for all jobs to complete.
        pool.wait_for_completion()?;

        assert_eq!(counter.load(Ordering::SeqCst), 10);

        let stats: ThreadPoolStats = pool.stats();

        assert_eq!(stats.total_jobs_completed, 10);
        assert_eq!(stats.failed_jobs, 0);

        // Cleanup.
        pool.shutdown()?;

        return Ok(());
    }

    #[test]
    fn thread_pool_shutdown() -> Result<()> {
        let mut pool: ThreadPool = ThreadPool::default_pool("shutdown-pool")?;

        // Add a job that takes some time.
        pool.execute(|| {
            std_thread::sleep(Duration::from_millis(100));
            return Ok(());
        })?;
        // Shutdown now should abort the job.
        pool.shutdown_now()?;

        // Should not be able to add more jobs.
        let result: Result<(), anyhow::Error> = pool.execute(|| Ok(()));

        assert!(result.is_err());

        // Join should not hang and return stats.
        let stats: ThreadPoolStats = pool.join()?;

        // Can't assert on exact stats since the job might or might not have completed
        // before shutdown_now, but can assert that join succeeded.
        assert!(stats.total_jobs_completed <= 1);

        return Ok(());
    }

    #[test]
    fn thread_pool_stats() -> Result<()> {
        let pool: ThreadPool = ThreadPool::default_pool("stats-pool")?;

        // Execute multiple jobs with different sleep durations.
        for i in 0..5 {
            let sleep_ms: u64 = 10 * (i + 1);
            pool.execute(move || {
                std_thread::sleep(Duration::from_millis(sleep_ms));
                return Ok(());
            })?;
        }

        // Add one failing job.
        pool.execute(|| {
            anyhow::bail!("Test failure");
        })?;

        // Wait for all jobs to complete.
        pool.wait_for_completion()?;

        let stats: ThreadPoolStats = pool.stats();

        assert_eq!(stats.total_jobs_completed, 6);
        assert_eq!(stats.failed_jobs, 1);
        assert!(stats.avg_processing_time.is_some());
        // Check other stats methods.
        assert_eq!(pool.completed_job_count(), 6);
        assert_eq!(pool.queued_job_count(), 0);
        assert!(!pool.is_shutting_down());

        // Cleanup.
        pool.shutdown()?;

        return Ok(());
    }

    #[test]
    fn thread_config_validation() -> Result<()> {
        // Valid config should pass validation.
        let config: Config = Config::default();

        assert!(config.validate().is_ok());

        // Zero max_threads should fail.
        let invalid_max: Config = Config {
            max_threads: 0,
            ..Default::default()
        };
        assert!(invalid_max.validate().is_err());

        // Too small stack size should fail.
        let invalid_stack: Config = Config {
            stack_size_bytes: 1024, // Too small (< 64KB).
            ..Default::default()
        };

        assert!(invalid_stack.validate().is_err());

        // Invalid core allocation should fail.
        let invalid_cores: Config = Config {
            core_allocation: CoreAllocation::PinnedCores { min: 5, max: 3 },
            ..Default::default()
        };

        assert!(invalid_cores.validate().is_err());

        return Ok(());
    }

    #[test]
    fn thread_worker_panic_handling() -> Result<()> {
        let worker: Native = Native::default_thread("panic-test")?;
        let handle: Result<JoinHandle<&'static str>, anyhow::Error> = worker.spawn(|| {
            if true {
                panic!("Test panic");
            }
            "should not reach here"
        });

        assert!(handle.is_ok());

        let handle: JoinHandle<&'static str> = handle.unwrap();
        let result: Result<&'static str, Box<dyn Any + Send + 'static>> = handle.join();

        assert!(result.is_err());

        // Ensure worker is still usable after a thread panic.
        let counter: Arc<AtomicUsize> = Arc::new(AtomicUsize::new(0));
        let counter_clone: Arc<AtomicUsize> = counter.clone();
        let handle: JoinHandle<&'static str> = worker.spawn(move || {
            counter_clone.fetch_add(1, Ordering::SeqCst);
            "success after panic"
        })?;
        let result: &'static str = handle.join().unwrap();

        assert_eq!(result, "success after panic");
        assert_eq!(counter.load(Ordering::SeqCst), 1);

        return Ok(());
    }

    #[test]
    fn thread_pool_concurrent_stress() -> Result<()> {
        let pool: ThreadPool = ThreadPool::default_pool("stress-pool")?;
        let counter: Arc<AtomicUsize> = Arc::new(AtomicUsize::new(0));
        let total_jobs: usize = 100;

        // Submit many jobs concurrently.
        for _ in 0..total_jobs {
            let counter: Arc<AtomicUsize> = counter.clone();
            pool.execute(move || {
                // Simulate some work with random duration.
                let sleep_ms: u64 = (std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap_or_default().subsec_nanos() % 10) as u64;
                std_thread::sleep(Duration::from_millis(sleep_ms));
                counter.fetch_add(1, Ordering::SeqCst);
                return Ok(());
            })?;
        }

        // Wait for all jobs to complete.
        pool.wait_for_completion()?;

        assert_eq!(counter.load(Ordering::SeqCst), total_jobs);
        assert_eq!(pool.completed_job_count(), total_jobs);

        // Cleanup.
        pool.shutdown()?;

        return Ok(());
    }
}
