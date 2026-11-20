//! A fake [`ScHttp`] for tests: a closure answers each request, and the requests are recorded.

use std::sync::Mutex;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;

use nemo_core::BoxFuture;

use crate::http::{ScAnswer, ScHttp, ScHttpError, ScMethod, ScRequest};

type Handler = Box<dyn Fn(&str, usize) -> Result<ScAnswer, ScHttpError> + Send + Sync>;

/// Answers requests with `handler(url, index)` and keeps the urls it saw.
pub struct FakeHttp {
    handler: Handler,
    seen: Mutex<Vec<String>>,
    /// The `Authorization` token of each request, in order.
    tokens: Mutex<Vec<Option<String>>>,
    /// The method and body of each request, in order.
    sent: Mutex<Vec<(ScMethod, Option<String>)>>,
    in_flight: AtomicUsize,
    max_in_flight: AtomicUsize,
    delay: Duration,
}

impl FakeHttp {
    pub fn new<F>(handler: F) -> Self
    where
        F: Fn(&str, usize) -> Result<ScAnswer, ScHttpError> + Send + Sync + 'static,
    {
        Self {
            handler: Box::new(handler),
            seen: Mutex::new(Vec::new()),
            tokens: Mutex::new(Vec::new()),
            sent: Mutex::new(Vec::new()),
            in_flight: AtomicUsize::new(0),
            max_in_flight: AtomicUsize::new(0),
            delay: Duration::from_millis(2),
        }
    }

    /// Take `delay` to answer each request (default 2 ms).
    #[must_use]
    pub fn slow(mut self, delay: Duration) -> Self {
        self.delay = delay;
        self
    }

    /// Every url requested so far, in order.
    pub fn urls(&self) -> Vec<String> {
        self.seen.lock().expect("lock").clone()
    }

    /// The token each request carried (`None` for a request without one), in order.
    pub fn tokens(&self) -> Vec<Option<String>> {
        self.tokens.lock().expect("lock").clone()
    }

    /// The method and the JSON body of each request, in order (`None` for a request without one).
    pub fn sent(&self) -> Vec<(ScMethod, Option<String>)> {
        self.sent.lock().expect("lock").clone()
    }

    /// The most requests that were open at the same moment.
    pub fn max_in_flight(&self) -> usize {
        self.max_in_flight.load(Ordering::SeqCst)
    }
}

/// A 200 answer with `body`.
pub fn ok(body: impl Into<Vec<u8>>) -> Result<ScAnswer, ScHttpError> {
    status(200, body)
}

/// An answer with `status` and `body`. It returns a `Result` so it reads like [`ok`] in a handler.
#[allow(clippy::unnecessary_wraps)]
pub fn status(status: u16, body: impl Into<Vec<u8>>) -> Result<ScAnswer, ScHttpError> {
    Ok(ScAnswer {
        status,
        body: body.into(),
        retry_after: None,
    })
}

impl ScHttp for FakeHttp {
    fn send(&self, request: ScRequest) -> BoxFuture<'_, Result<ScAnswer, ScHttpError>> {
        Box::pin(async move {
            let index = {
                let mut seen = self.seen.lock().expect("lock");
                seen.push(request.url.clone());
                self.sent.lock().expect("lock").push((
                    request.method,
                    request
                        .body
                        .as_ref()
                        .map(|b| String::from_utf8_lossy(b).into_owned()),
                ));
                self.tokens
                    .lock()
                    .expect("lock")
                    .push(request.authorization.as_ref().map(|t| t.expose().clone()));
                seen.len() - 1
            };
            let open = self.in_flight.fetch_add(1, Ordering::SeqCst) + 1;
            self.max_in_flight.fetch_max(open, Ordering::SeqCst);
            tokio::time::sleep(self.delay).await;
            let answer = (self.handler)(&request.url, index);
            self.in_flight.fetch_sub(1, Ordering::SeqCst);
            answer
        })
    }
}
