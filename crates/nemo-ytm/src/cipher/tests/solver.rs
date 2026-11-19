//! The engine and its limits, with hand-written player scripts: no network.

use std::time::{Duration, Instant};

use crate::cipher::solver::Engine;
use crate::cipher::{Challenges, JsSolver, Script, SolveError};

/// A "preprocessed player": what `core` hands to `Function('_result', ...)`.
const FAKE_PLAYER: &str = "_result.n = function (x) { return 'n-' + x.toUpperCase(); };\
                           _result.sig = function (s) { return s.split('').reverse().join(''); };";

fn limit() -> Duration {
    Duration::from_secs(5)
}

fn asked(n: &[&str], sig: &[&str]) -> Challenges {
    Challenges {
        n: n.iter().map(|s| (*s).to_owned()).collect(),
        sig: sig.iter().map(|s| (*s).to_owned()).collect(),
    }
}

fn pre<'a>(id: &'a str, text: &'a str) -> Script<'a> {
    Script::Preprocessed { id, text }
}

#[test]
fn the_functions_of_a_player_answer_n_and_signature_challenges() {
    let solver = JsSolver::start().expect("worker");
    let solved = solver
        .solve(
            pre("aaaaaaaa", FAKE_PLAYER),
            &asked(&["abc"], &["xyz"]),
            limit(),
        )
        .expect("solved");
    assert_eq!(solved.n.get("abc").map(String::as_str), Some("n-ABC"));
    assert_eq!(solved.sig.get("xyz").map(String::as_str), Some("zyx"));
}

#[test]
fn a_second_solve_for_the_same_player_needs_no_script() {
    let solver = JsSolver::start().expect("worker");
    let _ = solver.solve(pre("aaaaaaaa", FAKE_PLAYER), &asked(&["a"], &[]), limit());
    let again = solver
        .solve(pre("aaaaaaaa", ""), &asked(&["b"], &["cd"]), limit())
        .expect("the kept functions answer");
    assert_eq!(again.n.get("b").map(String::as_str), Some("n-B"));
    assert_eq!(again.sig.get("cd").map(String::as_str), Some("dc"));
    let other = solver.solve(pre("bbbbbbbb", ""), &asked(&["b"], &[]), limit());
    assert!(other.is_err(), "another player needs its own script");
}

#[test]
fn a_player_without_a_signature_function_fails_only_when_one_is_asked() {
    let solver = JsSolver::start().expect("worker");
    let script = "_result.n = function (x) { return x; };";
    assert!(
        solver
            .solve(pre("cccccccc", script), &asked(&["a"], &[]), limit())
            .is_ok()
    );
    let err = solver
        .solve(pre("cccccccc", script), &asked(&[], &["s"]), limit())
        .expect_err("no signature function");
    assert_eq!(err, SolveError::NoFunction("signature"));
}

#[test]
fn a_whole_script_that_is_not_a_player_fails_with_a_fixed_text() {
    let solver = JsSolver::start().expect("worker");
    let secret = "SECRET-PLAYER-TEXT";
    let err = solver
        .solve(
            Script::Whole {
                id: "dddddddd",
                text: &format!("var x = '{secret}';"),
            },
            &asked(&["a"], &[]),
            limit(),
        )
        .expect_err("not a player");
    assert!(!err.to_string().contains(secret));
    assert!(!format!("{err:?}").contains(secret));
}

#[test]
fn a_script_that_never_ends_is_stopped_at_the_time_limit_and_the_worker_goes_on() {
    let solver = JsSolver::start().expect("worker");
    let began = Instant::now();
    let err = solver
        .solve(
            pre("eeeeeeee", "_result.n = function (x) { for (;;) {} };"),
            &asked(&["a"], &[]),
            Duration::from_millis(300),
        )
        .expect_err("stopped");
    assert!(
        began.elapsed() < Duration::from_secs(5),
        "{:?}",
        began.elapsed()
    );
    assert!(
        matches!(err, SolveError::Timeout | SolveError::NoFunction(_)),
        "{err:?}"
    );
    // The engine was rebuilt: the next job works.
    let ok = solver
        .solve(pre("ffffffff", FAKE_PLAYER), &asked(&["a"], &[]), limit())
        .expect("the worker recovered");
    assert_eq!(ok.n.get("a").map(String::as_str), Some("n-A"));
}

#[test]
fn the_engine_stops_a_loop_and_a_memory_hog() {
    let engine = Engine::with_memory(32 * 1024 * 1024).expect("engine");
    let began = Instant::now();
    assert_eq!(
        engine.eval(Duration::from_millis(200), &[], "while (true) {} ''"),
        Err(SolveError::Timeout)
    );
    assert!(began.elapsed() < Duration::from_secs(5));
    let hog = "var a = []; for (;;) { a.push(new Array(1000000).fill(1)); } ''";
    assert_eq!(
        engine.eval(Duration::from_secs(20), &[], hog),
        Err(SolveError::Script),
        "out of memory is a script failure, not a crash"
    );
}

#[test]
fn the_engine_has_no_files_network_or_modules() {
    let engine = Engine::with_memory(32 * 1024 * 1024).expect("engine");
    let probe = "[typeof require, typeof process, typeof fetch, typeof XMLHttpRequest, \
                 typeof std, typeof os, typeof readFile].join(',')";
    let seen = engine.eval(limit(), &[], probe).expect("evaluated");
    assert_eq!(
        seen,
        "undefined,undefined,undefined,undefined,undefined,undefined,undefined"
    );
}
