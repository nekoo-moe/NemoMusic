//! Unit tests for the reqwest-backed HTTP client against a local test server.

use std::time::Instant;

use super::*;
use crate::test_server::{Response, TestServer};

fn runtime() -> tokio::runtime::Runtime {
    tokio::runtime::Builder::new_multi_thread()
        .worker_threads(1)
        .enable_all()
        .build()
        .expect("runtime builds")
}

/// Poll with a tiny executor-free `block_on` from another thread's point of view:
/// the future must not need the tokio runtime of the caller.
fn block_on_plain<F: std::future::Future>(future: F) -> F::Output {
    use std::sync::Arc;
    use std::task::{Context, Poll, Wake, Waker};

    struct ThreadWaker(std::thread::Thread);
    impl Wake for ThreadWaker {
        fn wake(self: Arc<Self>) {
            self.0.unpark();
        }
    }
    let waker = Waker::from(Arc::new(ThreadWaker(std::thread::current())));
    let mut context = Context::from_waker(&waker);
    let mut future = Box::pin(future);
    loop {
        if let Poll::Ready(value) = future.as_mut().poll(&mut context) {
            return value;
        }
        std::thread::park();
    }
}

#[test]
fn answers_are_read_with_status_body_and_retry_after() {
    let server = TestServer::start(|request, _| match request.path.as_str() {
        "/ok" => Response::new(200, b"[]".to_vec()),
        "/limited" => Response::new(429, Vec::new()).header("Retry-After", "17"),
        _ => Response::new(500, Vec::new()),
    });
    let rt = runtime();
    let http = ReqwestHttp::build(rt.handle().clone(), false).expect("builds");
    let ok = block_on_plain(http.get(&server.url("/ok"))).expect("answers");
    assert_eq!(
        (ok.status, ok.body.as_str(), ok.retry_after),
        (200, "[]", None)
    );
    let limited = block_on_plain(http.get(&server.url("/limited"))).expect("answers");
    assert_eq!(limited.status, 429);
    assert_eq!(limited.retry_after, Some(Duration::from_secs(17)));
}

#[test]
fn plain_http_is_refused_and_the_error_has_no_url() {
    let server = TestServer::start(|_, _| Response::new(200, Vec::new()));
    let rt = runtime();
    let http = ReqwestHttp::new(rt.handle().clone()).expect("builds");
    let url = server.url("/secret?sig=SECRET");
    let error = block_on_plain(http.get(&url)).unwrap_err();
    assert!(
        !error.contains("127.0.0.1") && !error.contains("SECRET"),
        "{error}"
    );
    assert_eq!(server.requests().len(), 0);
}

#[test]
fn a_slow_server_times_out_in_about_three_seconds() {
    let server = TestServer::start(|_, _| {
        std::thread::sleep(Duration::from_millis(3500));
        Response::new(200, Vec::new())
    });
    let rt = runtime();
    let http = ReqwestHttp::build(rt.handle().clone(), false).expect("builds");
    let started = Instant::now();
    let error = block_on_plain(http.get(&server.url("/slow"))).unwrap_err();
    assert_eq!(error, "request timed out");
    assert!(started.elapsed() < Duration::from_millis(3400));
}

#[test]
fn works_as_the_http_of_a_source() {
    let body = r#"[{"videoID":"abcdefghijk","segments":[{"segment":[1,2],"UUID":"u","category":"sponsor","actionType":"skip","videoDuration":0}]}]"#;
    let server = TestServer::start(move |_, _| Response::new(200, body.as_bytes().to_vec()));
    let rt = runtime();
    let http = ReqwestHttp::build(rt.handle().clone(), false).expect("builds");
    // The real URL points at sponsor.ajay.app; only the transport is checked here.
    let response = block_on_plain(http.get(&server.url("/api/skipSegments/ca2f"))).expect("ok");
    let id = VideoId::parse("abcdefghijk").expect("valid id");
    let segments = parse_response(&response.body, &id, Duration::from_secs(100)).expect("parses");
    assert_eq!(segments.len(), 1);
}
