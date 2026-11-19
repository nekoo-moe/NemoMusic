//! A spawned task that ends when its handle is dropped.
//!
//! A resolver spawns its work on the app runtime and returns a future that awaits the join
//! handle. Dropping a plain `JoinHandle` leaves the task running, so a resolve that lost a race
//! (see `FallbackResolver`) would keep its `yt-dlp` process or HTTP request going. This handle
//! aborts the task instead; the `yt-dlp` child dies with it (`kill_on_drop`).

use std::future::Future;
use std::pin::Pin;
use std::task::{Context, Poll};

use tokio::task::{JoinError, JoinHandle};

/// Awaits a task and aborts it when dropped before the task ended.
pub(crate) struct AbortOnDrop<T>(pub(crate) JoinHandle<T>);

impl<T> Drop for AbortOnDrop<T> {
    fn drop(&mut self) {
        // A finished task ignores this.
        self.0.abort();
    }
}

impl<T> Future for AbortOnDrop<T> {
    type Output = Result<T, JoinError>;

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        Pin::new(&mut self.0).poll(cx)
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::time::Duration;

    use super::*;

    struct Flag(Arc<AtomicBool>);

    impl Drop for Flag {
        fn drop(&mut self) {
            self.0.store(true, Ordering::SeqCst);
        }
    }

    #[test]
    fn dropping_the_handle_ends_the_task_and_finishing_first_is_harmless() {
        let rt = tokio::runtime::Builder::new_multi_thread()
            .worker_threads(1)
            .enable_time()
            .build()
            .expect("runtime");
        let ended = Arc::new(AtomicBool::new(false));
        let guard = Flag(Arc::clone(&ended));
        let task = AbortOnDrop(rt.spawn(async move {
            let _guard = guard;
            tokio::time::sleep(Duration::from_secs(30)).await;
        }));
        std::thread::sleep(Duration::from_millis(50));
        assert!(!ended.load(Ordering::SeqCst));
        drop(task);
        std::thread::sleep(Duration::from_millis(100));
        assert!(ended.load(Ordering::SeqCst), "the task was aborted");

        let done = AbortOnDrop(rt.spawn(async { 7 }));
        assert_eq!(rt.block_on(done).expect("ran"), 7);
    }
}
