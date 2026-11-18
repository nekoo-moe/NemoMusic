use super::*;

fn close(a: f32, b: f32) -> bool {
    (a - b).abs() < 0.01
}

#[test]
fn durations_follow_the_better_lyrics_numbers() {
    assert!(close(duration_ms(5, 5), 750.0), "the active line");
    // Above: 750 + ln(i + 1) * 10.
    assert!(close(duration_ms(4, 5), 750.0 + 2.0_f32.ln() * 10.0));
    assert!(close(duration_ms(2, 5), 750.0 + 4.0_f32.ln() * 10.0));
    // Below: 750 + ln(i + 1) * 80 + i * 20.
    assert!(close(duration_ms(6, 5), 750.0 + 2.0_f32.ln() * 80.0 + 20.0));
    assert!(close(duration_ms(8, 5), 750.0 + 4.0_f32.ln() * 80.0 + 60.0));
}

#[test]
fn lines_below_arrive_later_and_far_lines_are_capped() {
    let mut last = duration_ms(5, 5);
    for line in 6..=5 + MAX_DISTANCE {
        let d = duration_ms(line, 5);
        assert!(d > last, "{line}");
        last = d;
    }
    assert!(close(duration_ms(500, 5), duration_ms(5 + MAX_DISTANCE, 5)));
    assert!(close(
        duration_ms(0, 500),
        duration_ms(500 - MAX_DISTANCE, 500)
    ));
    assert!(duration_ms(6, 5) > duration_ms(4, 5), "below lags above");
}

#[test]
fn the_easing_is_strong_in_and_out_and_monotonic() {
    assert!(ease(0.0).abs() < 1e-4 && (ease(1.0) - 1.0).abs() < 1e-4);
    assert!(ease(0.1) < 0.05, "slow start: {}", ease(0.1));
    assert!(ease(0.9) > 0.9, "slow end: {}", ease(0.9));
    let mut last = 0.0;
    for i in 0..=100_u8 {
        let e = ease(f32::from(i) / 100.0);
        assert!(e >= last - 1e-5, "{i}");
        last = e;
    }
}

#[test]
fn a_retarget_lays_out_at_the_new_place_and_every_line_starts_at_the_old_one() {
    let mut c = Cascade::new(100.0);
    c.retarget(260.0, 5);
    assert!(close(c.value, 260.0) && close(c.target, 260.0));
    assert!(!c.is_settled());
    for line in [0, 5, 9] {
        assert!(
            close(c.offset(line), 160.0),
            "line {line} starts {}",
            c.offset(line)
        );
    }
    assert!(close(c.reach(), 160.0));
}

#[test]
fn lines_glide_to_zero_and_the_cascade_settles() {
    let mut c = Cascade::new(0.0);
    c.retarget(100.0, 5);
    // Halfway through the active line's 750 ms the lines below still have further to go.
    c.step(0.375);
    let (above, active, below) = (c.offset(3), c.offset(5), c.offset(8));
    assert!(active > 0.0 && active < 100.0, "{active}");
    assert!(below > active, "below lags: {below} vs {active}");
    assert!(
        (above - active).abs() < 5.0,
        "above moves with it: {above} vs {active}"
    );
    c.step(0.4);
    assert!(c.offset(5).abs() < 1e-3, "the active line has arrived");
    assert!(c.offset(8) > 0.0, "the line below is still on its way");
    c.step(1.0);
    assert!(c.is_settled());
    assert!(c.offset(8).abs() < 1e-6 && c.reach().abs() < 1e-6);
}

#[test]
fn a_scroll_up_moves_lines_the_other_way() {
    let mut c = Cascade::new(300.0);
    c.retarget(120.0, 2);
    assert!(close(c.offset(2), -180.0));
    c.step(0.2);
    assert!(c.offset(2) < 0.0 && c.offset(2) > -180.0);
}

#[test]
fn running_glides_add_up() {
    let mut c = Cascade::new(0.0);
    c.retarget(100.0, 1);
    c.step(0.3);
    let first = c.offset(4);
    c.retarget(180.0, 2);
    // The new glide adds its own delta on top of what the first one still has left.
    assert!(close(c.offset(4), first + 80.0));
    assert!(close(c.reach(), 180.0));
    // Both end, in the end.
    c.step(3.0);
    assert!(c.is_settled());
}

#[test]
fn snap_drops_every_glide_and_tiny_moves_are_taken_at_once() {
    let mut c = Cascade::new(0.0);
    c.retarget(200.0, 3);
    c.snap(500.0);
    assert!(c.is_settled() && close(c.value, 500.0) && close(c.offset(3), 0.0));
    c.retarget(500.2, 3);
    assert!(c.is_settled(), "a 0.2 px move needs no glide");
    assert!(close(c.value, 500.2));
}

#[test]
fn a_burst_of_retargets_keeps_a_bounded_number_of_glides() {
    let mut c = Cascade::new(0.0);
    for i in 1..=40_u8 {
        c.retarget(f32::from(i) * 10.0, 3);
        c.step(0.01);
    }
    assert!(c.runs.len() <= MAX_RUNS);
    c.step(5.0);
    assert!(c.is_settled());
}
