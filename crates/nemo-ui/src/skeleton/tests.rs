//! Timing of the skeleton gate: delay, hold, fade, reduced motion and the frame requests.

use super::*;

/// Advance in 10 ms steps for `ms` milliseconds.
fn run(g: &mut Gate, ms: u32, reduced: bool) {
    for _ in 0..ms / 10 {
        g.advance(10.0, reduced);
    }
}

#[test]
fn nothing_wanted_shows_content_and_asks_for_no_frames() {
    let g = Gate::default();
    assert_eq!(g.show(), Show::Content);
    assert!(!g.visible() && !g.needs_frames(false) && !g.needs_frames(true));
}

#[test]
fn the_skeleton_waits_150_ms_before_it_is_drawn() {
    let mut g = Gate::default();
    g.set(true, false);
    assert_eq!(g.show(), Show::Content, "wanted, but not yet");
    run(&mut g, 140, false);
    assert_eq!(g.show(), Show::Content, "140 ms: still waiting");
    run(&mut g, 10, false);
    assert_eq!(g.show(), Show::Skeleton, "150 ms: drawn");
}

#[test]
fn a_fast_answer_never_shows_it() {
    let mut g = Gate::default();
    g.set(true, false);
    run(&mut g, 100, false);
    g.set(false, false);
    assert_eq!(g.show(), Show::Content);
    assert!(!g.visible() && !g.needs_frames(false), "back to idle");
    run(&mut g, 400, false);
    assert_eq!(g.show(), Show::Content);
}

#[test]
fn once_shown_it_stays_at_least_300_ms_then_fades_for_200_ms() {
    let mut g = Gate::default();
    g.set(true, false);
    run(&mut g, 150, false);
    g.set(false, false);
    assert_eq!(g.show(), Show::Skeleton, "the data is in, the hold is on");
    run(&mut g, 280, false);
    assert_eq!(g.show(), Show::Skeleton, "280 ms of 300");
    run(&mut g, 20, false);
    let Show::Fade(a) = g.show() else {
        panic!("{:?}", g.show());
    };
    assert!(
        (a - 1.0).abs() < 1e-4,
        "the fade starts at full opacity: {a}"
    );
    run(&mut g, 100, false);
    let Show::Fade(mid) = g.show() else {
        panic!("{:?}", g.show());
    };
    assert!(mid > 0.0 && mid < 1.0, "{mid}");
    run(&mut g, 100, false);
    assert_eq!(g.show(), Show::Content);
    assert!(!g.visible() && !g.needs_frames(false));
}

#[test]
fn data_arriving_after_the_hold_starts_the_fade_at_once() {
    let mut g = Gate::default();
    g.set(true, false);
    run(&mut g, 150 + 500, false);
    assert_eq!(g.show(), Show::Skeleton);
    g.set(false, false);
    assert!(matches!(g.show(), Show::Fade(_)), "no extra frame needed");
}

#[test]
fn a_new_load_during_the_fade_brings_the_skeleton_back() {
    let mut g = Gate::default();
    g.set(true, false);
    run(&mut g, 150 + 300, false);
    g.set(false, false);
    run(&mut g, 100, false);
    assert!(matches!(g.show(), Show::Fade(_)));
    g.set(true, false);
    assert_eq!(g.show(), Show::Skeleton);
    g.set(false, false);
    assert!(
        matches!(g.show(), Show::Fade(_)),
        "already held long enough"
    );
}

#[test]
fn a_skeleton_that_is_still_wanted_never_ends() {
    let mut g = Gate::default();
    g.set(true, false);
    run(&mut g, 60_000, false);
    assert_eq!(g.show(), Show::Skeleton);
}

#[test]
fn reduced_motion_is_still_and_swaps_without_a_fade() {
    let mut g = Gate::default();
    g.set(true, true);
    assert!(g.needs_frames(true), "the 150 ms wait counts time");
    run(&mut g, 150, true);
    assert_eq!(g.show(), Show::Skeleton);
    assert!(!g.needs_frames(true), "a still skeleton asks for no frames");
    g.set(false, true);
    assert!(g.needs_frames(true), "the hold counts time");
    run(&mut g, 300, true);
    assert_eq!(g.show(), Show::Content, "no fade");
    assert!(!g.needs_frames(true));
}

#[test]
fn frames_are_asked_for_in_every_moving_stage_with_motion_on() {
    let mut g = Gate::default();
    g.set(true, false);
    assert!(g.needs_frames(false), "waiting");
    run(&mut g, 150, false);
    assert!(g.needs_frames(false), "the shimmer moves");
    g.set(false, false);
    assert!(g.needs_frames(false), "holding");
    run(&mut g, 300, false);
    assert!(g.needs_frames(false), "fading");
    run(&mut g, 200, false);
    assert!(!g.needs_frames(false), "idle again");
}

#[test]
fn leaving_the_page_drops_a_waiting_gate_at_once() {
    let mut g = Gate::default();
    g.set(true, false);
    run(&mut g, 100, false);
    g.set(false, false);
    assert!(!g.needs_frames(false));
}

#[test]
fn the_shimmer_clock_runs_only_while_a_skeleton_is_drawn_with_motion_on() {
    let mut s = Skeletons::default();
    s.advance(500.0, false);
    assert!(s.phase().abs() < 1e-6, "nothing drawn: the clock rests");
    s.set(Region::Home, true, false);
    s.advance(100.0, false);
    assert!(s.phase().abs() < 1e-6, "waiting is not drawing");
    s.advance(100.0, false);
    assert!(s.any_visible());
    s.advance(700.0, false);
    assert!((s.phase() - 0.5).abs() < 1e-3, "{}", s.phase());
    s.advance(700.0, false);
    assert!(s.phase().abs() < 1e-3, "one loop is 1.4 s: {}", s.phase());
    s.advance(350.0, false);
    assert!((s.phase() - 0.25).abs() < 1e-3);
    s.advance(100.0, true);
    assert!(s.phase().abs() < 1e-6, "reduced motion: no band");
}

#[test]
fn each_region_has_its_own_gate() {
    let mut s = Skeletons::default();
    s.set(Region::Search, true, false);
    s.advance(200.0, false);
    assert_eq!(s.show(Region::Search), Show::Skeleton);
    assert_eq!(s.show(Region::Home), Show::Content);
    assert_eq!(s.show(Region::Sidebar), Show::Content);
    assert!(s.needs_frames(false));
    s.set(Region::Search, false, false);
    s.advance(300.0, false);
    s.advance(300.0, false);
    assert!(!s.needs_frames(false));
}

#[test]
fn the_band_enters_left_and_leaves_right_with_standard_easing() {
    assert!(band_position(0.0).abs() < 1e-4);
    assert!((band_position(0.999) - 1.0).abs() < 0.01);
    let mut prev = 0.0;
    for i in 1..100_u8 {
        let p = band_position(f32::from(i) / 100.0);
        assert!(p >= prev - 1e-6, "monotonic at {i}");
        prev = p;
    }
}
