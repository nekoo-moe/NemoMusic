//! The cast service and the tokio runtime it owns.

use tokio::runtime::{Builder, Handle, Runtime};

/// A cast request failed. The texts hold no stream URL, token or account data.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum CastError {
    /// Casting cannot start on this computer (the network runtime did not start).
    #[error("casting is not available")]
    NotAvailable,
}

/// Cast discovery, session and server, behind one handle.
///
/// For now it only owns the crate's own tokio runtime; later tasks add discovery, the session
/// and the LAN server on top of it. Dropping the service stops the runtime without waiting for
/// its tasks.
#[derive(Debug)]
pub struct CastService {
    /// Always `Some` until drop, which takes it to shut it down without blocking.
    runtime: Option<Runtime>,
}

impl CastService {
    /// Starts the service and its runtime. No socket is opened.
    ///
    /// # Errors
    /// [`CastError::NotAvailable`] when the runtime cannot start.
    pub fn new() -> Result<Self, CastError> {
        let runtime = Builder::new_multi_thread()
            .worker_threads(2)
            .thread_name("nemo-cast")
            .enable_time()
            .build()
            .map_err(|_| CastError::NotAvailable)?;
        Ok(Self {
            runtime: Some(runtime),
        })
    }

    /// A handle to the crate's runtime, for tasks that spawn network work.
    #[must_use]
    pub fn handle(&self) -> Option<Handle> {
        self.runtime.as_ref().map(Runtime::handle).cloned()
    }
}

impl Drop for CastService {
    fn drop(&mut self) {
        if let Some(runtime) = self.runtime.take() {
            // Dropping a runtime inside another runtime panics; this never blocks or panics.
            runtime.shutdown_background();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::CastService;

    #[test]
    fn service_runs_a_task_on_its_own_runtime() {
        let service = CastService::new().expect("runtime starts in tests");
        let handle = service.handle().expect("handle exists before drop");
        let value = handle.block_on(async { 40 + 2 });
        assert_eq!(value, 42);
    }

    #[tokio::test(flavor = "current_thread")]
    async fn service_drops_inside_another_runtime() {
        let service = CastService::new().expect("runtime starts in tests");
        drop(service);
    }
}
