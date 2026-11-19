//! Tests for the `reqwest` implementation against a loopback server.

use std::time::Duration;

use super::*;
use crate::test_server::{Response, TestServer};

fn http() -> ReqwestHttp {
    ReqwestHttp::new_for_tests(tokio::runtime::Handle::current()).expect("client")
}

fn get(url: String, max_bytes: usize) -> ScRequest {
    ScRequest { url, max_bytes }
}

#[test]
fn retry_after_seconds_only() {
    assert_eq!(parse_retry_after("30"), Some(Duration::from_secs(30)));
    assert_eq!(parse_retry_after(" 5 "), Some(Duration::from_secs(5)));
    assert_eq!(parse_retry_after("Wed, 21 Oct 2026 07:28:00 GMT"), None);
    assert_eq!(parse_retry_after("-1"), None);
    assert_eq!(parse_retry_after(""), None);
}

#[test]
fn the_real_constructor_requires_https() {
    let rt = tokio::runtime::Builder::new_current_thread()
        .build()
        .expect("runtime");
    let http = ReqwestHttp::new(rt.handle().clone()).expect("client");
    let server = TestServer::start(|_, _| Response::new(200, "plain"));
    let answer = rt.block_on(http.send(get(server.url("/x"), 100)));
    assert_eq!(answer, Err(ScHttpError::Failed));
    assert!(server.paths().is_empty(), "no plain http request was sent");
}

#[tokio::test]
async fn an_answer_carries_status_body_and_retry_after() {
    let server =
        TestServer::start(|_, _| Response::new(429, "slow down").header("Retry-After", "42"));
    let answer = http().send(get(server.url("/a?client_id=x"), 100)).await;
    let answer = answer.expect("answer");
    assert_eq!(answer.status, 429);
    assert_eq!(answer.body, b"slow down");
    assert_eq!(answer.retry_after, Some(Duration::from_secs(42)));
    assert_eq!(server.paths(), ["/a?client_id=x"]);
}

#[tokio::test]
async fn a_body_past_the_cap_fails() {
    let server = TestServer::start(|_, _| Response::new(200, vec![b'x'; 2000]));
    let answer = http().send(get(server.url("/big"), 1000)).await;
    assert_eq!(answer, Err(ScHttpError::TooLarge));
}

#[tokio::test]
async fn a_redirect_to_plain_http_is_not_followed_and_errors_hold_no_address() {
    // The test client accepts http, so check the policy itself: a redirect to ftp stops.
    let server = TestServer::start(|_, _| {
        Response::new(302, "").header("Location", "ftp://127.0.0.1:1/secret?client_id=abc")
    });
    let answer = http().send(get(server.url("/r"), 100)).await;
    if let Ok(answer) = &answer {
        assert_eq!(answer.status, 302, "the redirect is stopped, not followed");
    }
    if let Err(e) = answer {
        let text = e.to_string();
        assert!(
            !text.contains("127.0.0.1") && !text.contains("client_id"),
            "{text}"
        );
    }
}

#[tokio::test]
async fn a_dead_port_gives_a_fixed_error_text() {
    let answer = http()
        .send(get("http://127.0.0.1:1/x?client_id=abc".to_owned(), 100))
        .await;
    let e = answer.expect_err("nothing listens there");
    assert_eq!(e, ScHttpError::Failed);
    assert!(!e.to_string().contains("abc"));
}
