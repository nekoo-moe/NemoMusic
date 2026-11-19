//! The solver against the `yt-dlp/ejs` test vectors, on real player scripts.
//!
//! `cargo test -p nemo-ytm cipher::tests::vectors -- --ignored --nocapture`. It downloads up to
//! seven player scripts (about 2.7 MB each) from `www.youtube.com`, one second apart, into
//! `$NEMO_PLAYER_DIR` (default: a folder in the temp directory) and reuses them on later runs.

use std::path::PathBuf;
use std::time::{Duration, Instant};

use serde_json::Value;

use crate::cipher::{Challenges, JsSolver, SOLVE_TIMEOUT, Script};

const VECTORS: &str = include_str!("vectors.json");

fn folder() -> PathBuf {
    std::env::var_os("NEMO_PLAYER_DIR")
        .map_or_else(|| std::env::temp_dir().join("nemo-players"), PathBuf::from)
}

fn player(id: &str) -> String {
    let path = folder().join(format!("{id}-main"));
    if let Ok(text) = std::fs::read_to_string(&path) {
        return text;
    }
    std::thread::sleep(Duration::from_secs(1));
    let url = format!("https://www.youtube.com/s/player/{id}/player_ias.vflset/en_US/base.js");
    let text = reqwest::blocking::get(url)
        .and_then(reqwest::blocking::Response::text)
        .expect("the player script downloads");
    std::fs::create_dir_all(folder()).expect("folder");
    std::fs::write(&path, &text).expect("write");
    text
}

fn steps(entry: &Value, key: &str) -> Vec<(String, String)> {
    entry
        .get(key)
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .map(|s| {
            let text = |k: &str| s[k].as_str().expect("text").to_owned();
            (text("input"), text("expected"))
        })
        .collect()
}

#[test]
#[ignore = "downloads player scripts from youtube.com"]
fn the_solver_gives_the_expected_answers_for_every_vector() {
    let vectors: Vec<Value> = serde_json::from_str(VECTORS).expect("vectors");
    let solver = JsSolver::start().expect("worker");
    for entry in &vectors {
        let id = entry["player"].as_str().expect("id");
        let (n, sig) = (steps(entry, "n"), steps(entry, "sig"));
        let challenges = Challenges {
            n: n.iter().map(|(i, _)| i.clone()).collect(),
            sig: sig.iter().map(|(i, _)| i.clone()).collect(),
        };
        let script = player(id);
        let began = Instant::now();
        let whole = solver
            .solve(
                Script::Whole { id, text: &script },
                &challenges,
                SOLVE_TIMEOUT,
            )
            .expect("solved");
        let first = began.elapsed();
        for (input, expected) in &n {
            assert_eq!(whole.n.get(input), Some(expected), "n of player {id}");
        }
        for (input, expected) in &sig {
            assert_eq!(whole.sig.get(input), Some(expected), "sig of player {id}");
        }
        // The same player again: the kept functions answer, no script is read.
        let began = Instant::now();
        let kept = solver
            .solve(
                Script::Preprocessed { id, text: "" },
                &challenges,
                SOLVE_TIMEOUT,
            )
            .expect("solved from the kept functions");
        let kept_ms = began.elapsed().as_millis();
        assert_eq!((&kept.n, &kept.sig), (&whole.n, &whole.sig));
        // After another player was loaded, the preprocessed script (what the cache file holds)
        // gives the same answers.
        let small = whole.preprocessed.expect("preprocessed script");
        let _ = solver.solve(
            Script::Preprocessed {
                id: "other",
                text: "x",
            },
            &Challenges::default(),
            SOLVE_TIMEOUT,
        );
        let began = Instant::now();
        let again = solver
            .solve(
                Script::Preprocessed { id, text: &small },
                &challenges,
                SOLVE_TIMEOUT,
            )
            .expect("solved again");
        assert_eq!((again.n, again.sig), (whole.n, whole.sig));
        println!(
            "player {id}: whole script {} ms, kept functions {kept_ms} ms, reloaded from the \
             preprocessed script ({} kB) {} ms",
            first.as_millis(),
            small.len() / 1000,
            began.elapsed().as_millis()
        );
    }
}
