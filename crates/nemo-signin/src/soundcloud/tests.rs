//! The clock and the readers of the SoundCloud window. No webview is needed: they are pure.

use std::time::{Duration, Instant};

use super::*;

const TOKEN: &str = "2-293232-1234567-AbCdEfGh";

#[test]
fn the_clock_fires_once_a_second_however_often_the_loop_wakes() {
    let start = Instant::now();
    let mut poller = Poller::new(start, false);
    assert!(poller.due(start), "the first look is at once");
    // A busy page wakes the loop every 5 ms; the deadline does not move with it.
    let mut looks = 0;
    for ms in (5..3_000).step_by(5) {
        if poller.due(start + Duration::from_millis(ms)) {
            looks += 1;
        }
    }
    assert_eq!(looks, 2, "at about 1 s and 2 s of 3 s");
    assert!(poller.deadline() > start + Duration::from_millis(2_900));
}

#[test]
fn a_look_can_be_asked_for_at_once() {
    let start = Instant::now();
    let mut poller = Poller::new(start, false);
    assert!(poller.due(start));
    assert!(!poller.due(start + Duration::from_millis(10)));
    poller.look_now(start + Duration::from_millis(10));
    assert!(poller.due(start + Duration::from_millis(10)));
}

#[test]
fn the_hint_comes_once_after_a_soundcloud_page_waited_in_vain() {
    let start = Instant::now();
    let mut poller = Poller::new(start, false);
    assert!(
        !poller.hint_due(start + HINT_AFTER * 4),
        "no page loaded yet"
    );
    poller.page_loaded("https://accounts.example/other", start);
    assert!(
        !poller.hint_due(start + HINT_AFTER * 4),
        "not a SoundCloud page"
    );
    poller.page_loaded("https://soundcloud.com/discover", start);
    assert!(!poller.hint_due(start + (HINT_AFTER / 2)));
    assert!(poller.hint_due(start + HINT_AFTER));
    assert!(!poller.hint_due(start + HINT_AFTER * 2), "only once");
}

#[test]
fn a_later_page_load_restarts_the_wait_before_the_hint() {
    let start = Instant::now();
    let mut poller = Poller::new(start, false);
    poller.page_loaded("https://soundcloud.com/signin", start);
    let later = start + Duration::from_secs(20);
    poller.page_loaded("https://soundcloud.com/discover", later);
    assert!(
        !poller.hint_due(start + HINT_AFTER),
        "20 s ago was the signed-in page"
    );
    assert!(poller.hint_due(later + HINT_AFTER));
}

#[test]
fn the_diagnostic_lists_at_first_then_every_ten_seconds_and_only_when_on() {
    let start = Instant::now();
    let mut off = Poller::new(start, false);
    assert!(!off.debug_due(start));
    let mut on = Poller::new(start, true);
    assert!(on.debug_due(start));
    assert!(!on.debug_due(start + Duration::from_secs(9)));
    assert!(on.debug_due(start + Duration::from_secs(10)));
}

#[test]
fn a_document_cookie_answer_is_a_json_string_and_gives_only_the_token() {
    let answer = serde_json::to_string(&format!("a=b; oauth_token={TOKEN}; c=d")).expect("json");
    assert_eq!(read_document_answer(&answer, false).as_deref(), Some(TOKEN));
    let quoted = serde_json::to_string(&format!("oauth_token=\"{TOKEN}\"")).expect("json");
    assert_eq!(read_document_answer(&quoted, false).as_deref(), Some(TOKEN));
    let escaped = serde_json::to_string("oauth_token=2-293232-1234567-Ab%2FCdEf").expect("json");
    assert_eq!(
        read_document_answer(&escaped, false).as_deref(),
        Some("2-293232-1234567-Ab/CdEf")
    );
    for none in ["\"\"", "\"a=b\"", "null", "", "42", "not json"] {
        assert_eq!(read_document_answer(none, false), None, "{none}");
    }
}

#[test]
fn the_storage_answer_gives_names_and_lengths_and_no_value() {
    let inner = serde_json::json!({
        "ls": [["sc_user_123456789", 40], ["theme", 5]],
        "ss": [],
        "hy": [["apiClient", 61], ["user", 900]],
    })
    .to_string();
    let answer = serde_json::to_string(&inner).expect("json");
    let lines = storage_lines(&answer);
    assert!(lines.contains(&"localStorage: 2 entries".to_owned()));
    assert!(lines.contains(&"localStorage name=sc_user_# length=40".to_owned()));
    assert!(lines.contains(&"sessionStorage: 0 entries".to_owned()));
    assert!(lines.contains(&"hydration name=apiClient length=61".to_owned()));
    assert_eq!(storage_lines("garbage").len(), 1);
    assert_eq!(storage_lines("\"not an object\"").len(), 1);
}
