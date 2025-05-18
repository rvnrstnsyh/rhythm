use std::{any::Any, sync::atomic::Ordering, thread};

use crate::native_runtime::types::JoinHandle;

impl<T> JoinHandle<T> {
    fn join_inner(&mut self) -> thread::Result<T> {
        return match self.std_handle.take() {
            Some(jh) => {
                let result: Result<T, Box<dyn Any + Send + 'static>> = jh.join();
                self.running_count.fetch_sub(1, Ordering::Release);
                result
            }
            None => {
                panic!("Thread already joined.");
            }
        };
    }

    /// Join the thread and get its result.
    ///
    /// # Returns
    /// Returns `Ok(T)` if the thread completed successfully, or
    /// `Err(Box<dyn Any + Send>)` if the thread panicked.
    pub fn join(mut self) -> thread::Result<T> {
        return self.join_inner();
    }

    /// Check if the thread has finished execution.
    pub fn is_finished(&self) -> bool {
        return match self.std_handle {
            Some(ref jh) => jh.is_finished(),
            None => true,
        };
    }

    /// Get the name of the thread.
    pub fn name(&self) -> &str {
        return &self.name;
    }
}

impl<T> Drop for JoinHandle<T> {
    fn drop(&mut self) {
        if self.std_handle.is_some() {
            eprintln!(
                "Warning: Dropping a JoinHandle for thread '{}' without joining. This will leak thread IDs.",
                self.name
            );
            // Ignore any panic when dropping.
            let _ = self.join_inner();
        }
    }
}
