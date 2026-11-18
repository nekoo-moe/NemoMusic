use super::*;

fn tm(start: i64, end: i64) -> Timing {
    Timing {
        start_ms: start,
        end_ms: end,
    }
}

fn close(a: f32, b: f32) -> bool {
    (a - b).abs() < 1e-3
}

#[test]
fn the_edge_is_at_the_end_of_the_piece_when_the_piece_ends() {
    let w = tm(1000, 1400);
    assert!(
        close(sweep_edge(800, w), -0.2),
        "starts half a length early"
    );
    assert!(close(sweep_edge(1000, w), 0.2), "at the start it is 20% in");
    assert!(
        close(sweep_edge(1400, w), 1.0),
        "at the end it is at the end"
    );
    assert!(close(sweep_edge(1600, w), 1.4), "then the tail moves past");
    assert!(close(sweep_edge(-5000, w), -0.2) && close(sweep_edge(99_000, w), 1.4));
    let mut last = -1.0;
    for t in 800..=1600 {
        let e = sweep_edge(t, w);
        assert!(e >= last, "monotonic at {t}");
        last = e;
    }
}

#[test]
fn the_edge_is_soft_and_a_tenth_of_a_piece_wide() {
    assert!(close(coverage(0.5, 0.6), 1.0), "behind the edge: bright");
    assert!(close(coverage(0.5, 0.55), 0.5), "inside it: half");
    assert!(close(coverage(0.5, 0.5), 0.0) && close(coverage(0.5, 0.2), 0.0));
    assert!(close(SOFT_EDGE, 0.1));
}

#[test]
fn the_glow_grows_with_the_edge_and_fades_after_the_piece() {
    let w = tm(1000, 1400);
    assert!(
        glow_at(800, w).abs() < 1e-6,
        "nothing before the edge arrives"
    );
    assert!(glow_at(1000, w) > 0.0 && glow_at(1000, w) < 1.0);
    assert!(close(glow_at(1400, w), 1.0));
    // A 400 ms piece fades over 1.2 s (the minimum), not over 1.2 x 400 ms.
    let mid = glow_at(2000, w);
    assert!(mid > 0.05 && mid < 0.95, "{mid}");
    assert!(glow_at(2600, w).abs() < 1e-3);
    // A 3 s piece fades over 3.6 s.
    let long = tm(0, 3000);
    assert!(glow_at(3000 + 3000, long) > 0.0, "still glowing 3 s after");
    assert!(glow_at(3000 + 3600, long).abs() < 1e-3);
}

#[test]
fn the_wobble_peaks_at_an_eighth_of_a_second_and_is_over_at_75_percent() {
    let w = tm(1000, 1400);
    let at = |t| wobble_at(t, w);
    assert!(
        close(at(999).shift_em, 0.0) && close(at(999).scale, 1.0),
        "before"
    );
    assert!(close(at(1000).shift_em, 0.0));
    assert!(
        close(at(1125).shift_em, 0.05) && close(at(1125).scale, 1.025),
        "peak"
    );
    let mid = at(1400);
    assert!(mid.shift_em > 0.0 && mid.shift_em < 0.05, "settling");
    assert!(
        close(at(1750).shift_em, 0.0) && close(at(1750).scale, 1.0),
        "settled"
    );
    assert!(close(at(5000).scale, 1.0));
}

#[test]
fn only_pieces_held_for_1_5_seconds_are_long() {
    assert!(!is_long(tm(0, 1499)) && is_long(tm(0, 1500)));
}

#[test]
fn letters_lift_one_after_the_other_and_settle() {
    let w = tm(1000, 3500);
    let at = |i, t| letter_at(i, 6, t, w);
    assert!(close(at(0, 1000).lift_em, 0.0) && close(at(0, 1000).scale, 1.0));
    // The first letter's crest is 360 ms in: 0.05 em up and 11% bigger (the piece is 2.5 s).
    assert!(close(at(0, 1360).lift_em, 0.05) && close(at(0, 1360).scale, 1.11));
    // The last letters wait: 0.6 x 2500 ms x 5/6 = 1250 ms.
    assert!(at(5, 1500).lift_em < at(0, 1500).lift_em);
    assert!(close(at(5, 1000 + 1250).lift_em, 0.0));
    // Settled at 0.02 em, still up while the piece is held.
    assert!(close(at(0, 2200).lift_em, 0.02) && close(at(0, 3400).lift_em, 0.02));
    assert!(at(0, 2200).scale > 1.0 && at(0, 2200).scale < 1.11);
    // Back at rest 500 ms after the piece ends.
    assert!(at(0, 3750).lift_em > 0.0 && at(0, 3750).lift_em < 0.02);
    assert!(close(at(0, 4000).lift_em, 0.0) && close(at(0, 4000).scale, 1.0));
}

#[test]
fn a_piece_of_1_5_to_2_seconds_waves_without_the_swell() {
    let w = tm(0, 1700);
    let crest = letter_at(0, 4, 360, w);
    assert!(close(crest.lift_em, 0.05));
    assert!(close(crest.scale, 1.0), "no swell under 2 s");
}

#[test]
fn state_follows_the_sweep_and_settles_after_every_effect() {
    let w = tm(1000, 1400);
    assert_eq!(state_at(799, w, false), WordState::Upcoming);
    assert_eq!(state_at(800, w, false), WordState::Active);
    // The glow fades over 1.2 s after the end at 1400.
    assert_eq!(state_at(2599, w, false), WordState::Active);
    assert_eq!(state_at(2600, w, false), WordState::Past);
    assert_eq!(settle_ms(w), 2600);
    // A long piece also waits for its wave and fade-out.
    let long = tm(1000, 3500);
    assert!(settle_ms(long) >= 3500 + 500);
    assert_eq!(
        state_at(settle_ms(long) - 1, long, false),
        WordState::Active
    );
}

#[test]
fn reduced_motion_turns_a_piece_bright_when_it_starts() {
    let w = tm(1000, 1400);
    assert_eq!(state_at(999, w, true), WordState::Upcoming);
    assert_eq!(state_at(1000, w, true), WordState::Past);
    let before = word_fx(999, w, true);
    let after = word_fx(1000, w, true);
    assert!(close(before.edge, EDGE_FROM) && close(after.edge, EDGE_TO));
    for fx in [before, after] {
        assert!(fx.glow.abs() < f32::EPSILON);
        assert!(fx.wobble.shift_em.abs() < f32::EPSILON);
        assert!((fx.wobble.scale - 1.0).abs() < f32::EPSILON);
    }
    let live = word_fx(1200, w, false);
    assert!(live.edge > EDGE_FROM && live.edge < EDGE_TO && live.glow > 0.0);
}
