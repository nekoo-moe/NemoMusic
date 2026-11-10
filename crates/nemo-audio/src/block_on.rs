//! A minimal `block_on` for the runtime-agnostic futures of the service traits.
//!
//! Service implementations run their work on their own runtime and return a future that only
//! awaits a join handle, so a thread-parking waker is enough. It is called on helper threads,
//! never on the UI executor, the control thread or the audio callback.

use std::future::Future;
use std::pin::pin;
use std::sync::Arc;
use std::task::{Context, Poll, Wake, Waker};
use std::thread::{self, Thread};

struct ThreadWaker(Thread);

impl Wake for ThreadWaker {
    fn wake(self: Arc<Self>) {
        self.0.unpark();
    }
}

/// Poll `future` on the current thread until it is ready, parking the thread in between.
pub fn block_on<F: Future>(future: F) -> F::Output {
    let mut future = pin!(future);
    let waker = Waker::from(Arc::new(ThreadWaker(thread::current())));
    let mut cx = Context::from_waker(&waker);
    loop {
        if let Poll::Ready(value) = future.as_mut().poll(&mut cx) {
            return value;
        }
        thread::park();
    }
}

#[cfg(test)]
mod tests {
    use super::block_on;

    #[test]
    fn ready_future_returns_its_value() {
        assert_eq!(block_on(async { 7 }), 7);
    }

    #[test]
    fn future_woken_from_another_thread_completes() {
        let (tx, rx) = std::sync::mpsc::channel();
        let handle = std::thread::spawn(move || {
            std::thread::sleep(std::time::Duration::from_millis(20));
            tx.send(5).expect("send");
        });
        let value = block_on(async move { rx.recv().expect("recv") });
        assert_eq!(value, 5);
        handle.join().expect("join");
    }
}
