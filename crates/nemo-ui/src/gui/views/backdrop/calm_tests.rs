//! The window backdrop is calm (backdrop-calm spec): no flash, one frame clock, one bloom per
//! song change, the same image handles every frame. With no display here, the numbers come from
//! the pure maths: the real field textures, drawn at the rectangles `drift::field_rect` gives,
//! over the flat colour, at a coarse grid of the window.

#![allow(
    clippy::cast_precision_loss,
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    clippy::many_single_char_names
)] // test maths on small bounded numbers

use std::time::{Duration, Instant};

use nemo_core::player::{PlayState, PlayerCommand, PlayerEvent};

use super::{moves, window_layers};
use crate::canvas::{MODES, mode_index};
use crate::drift::{self, Clock};
use crate::gui::app::{App, Message};
use crate::gui::headless::{Scenario, build_app};
use crate::mesh::{self, BLOB_PX, Mesh};
use crate::mock::mock_backend;
use crate::route::Route;
use crate::scheme::BaseMode;
use crate::state::Event;
use crate::theme::motion;

const WINDOW: (f32, f32) = (1280.0, 800.0);
const GRID: (usize, usize) = (40, 25);
/// Most the mean lightness may change from one frame to the next, in points of the 0 to 100
/// scale (0.5 % of the range).
const FRAME_STEP_L: f64 = 0.5;
/// Most it may change over one breath (1.5 % of the range).
const CYCLE_SWING_L: f64 = 1.5;

fn solid(c: [u8; 3]) -> Vec<u8> {
    (0..24 * 24).flat_map(|_| [c[0], c[1], c[2], 255]).collect()
}

fn stripes(colors: &[[u8; 3]]) -> Vec<u8> {
    (0..24 * 24)
        .flat_map(|i| {
            let c = colors[(i % 24) * colors.len() / 24];
            [c[0], c[1], c[2], 255]
        })
        .collect()
}

/// Covers that stress the fields: black, white, grey, saturated, yellow, three and four hues.
fn covers() -> Vec<(&'static str, Vec<u8>)> {
    vec![
        ("black", solid([0, 0, 0])),
        ("white", solid([255, 255, 255])),
        ("grey", solid([128, 128, 128])),
        ("saturated", solid([255, 20, 140])),
        ("yellow", solid([250, 230, 0])),
        (
            "rgb",
            stripes(&[[220, 30, 40], [30, 180, 60], [40, 60, 220]]),
        ),
        (
            "four",
            stripes(&[[220, 30, 40], [240, 220, 20], [20, 170, 200], [40, 60, 220]]),
        ),
    ]
}

/// CIE lightness L* (0 to 100) of an sRGB colour with channels 0 to 255.
fn lightness(rgb: [f64; 3]) -> f64 {
    let lin = |c: f64| {
        let s = c / 255.0;
        if s <= 0.040_45 {
            s / 12.92
        } else {
            ((s + 0.055) / 1.055).powf(2.4)
        }
    };
    let y = 0.2126 * lin(rgb[0]) + 0.7152 * lin(rgb[1]) + 0.0722 * lin(rgb[2]);
    if y > 216.0 / 24_389.0 {
        116.0 * y.cbrt() - 16.0
    } else {
        24_389.0 / 27.0 * y
    }
}

/// Opacity of a field texture at `(u, v)` (0 to 1 across it), bilinear like the GPU.
fn alpha_at(tex: &[u8], u: f32, v: f32) -> f64 {
    if !(0.0..1.0).contains(&u) || !(0.0..1.0).contains(&v) {
        return 0.0;
    }
    let n = BLOB_PX as usize;
    let at = |x: usize, y: usize| f64::from(tex[(y * n + x) * 4 + 3]) / 255.0;
    let (fx, fy) = (
        (f64::from(u) * n as f64 - 0.5).clamp(0.0, n as f64 - 1.001),
        (f64::from(v) * n as f64 - 0.5).clamp(0.0, n as f64 - 1.001),
    );
    let (x, y) = (fx as usize, fy as usize);
    let (tx, ty) = (fx - x as f64, fy - y as f64);
    let top = at(x, y) * (1.0 - tx) + at(x + 1, y) * tx;
    let bottom = at(x, y + 1) * (1.0 - tx) + at(x + 1, y + 1) * tx;
    top * (1.0 - ty) + bottom * ty
}

/// How a frame is drawn: where on the drift clock, with the breath or not, the layer scale, and
/// the white lift over the fields (0 now; the old heartbeat used up to 5 %).
#[derive(Clone, Copy)]
struct Look {
    at_ms: f64,
    breathing: bool,
    scale: f32,
    lift: f64,
}

/// Mean L* over a grid of the window: the flat colour, the fields over it, the lift on top.
fn mean_lightness(m: &Mesh, base: BaseMode, look: Look) -> f64 {
    let bi = mode_index(base);
    let under = m.base[bi];
    let bounds = (0.0, 0.0, WINDOW.0, WINDOW.1);
    let rects: Vec<_> = (0..m.n)
        .map(|i| drift::field_rect(bounds, i, look.at_ms, look.scale, look.breathing))
        .collect();
    let mut sum = 0.0;
    for gy in 0..GRID.1 {
        for gx in 0..GRID.0 {
            let px = (gx as f32 + 0.5) / GRID.0 as f32 * WINDOW.0;
            let py = (gy as f32 + 0.5) / GRID.1 as f32 * WINDOW.1;
            let mut c = [under.r, under.g, under.b].map(f64::from);
            for (i, &(x, y, side)) in rects.iter().enumerate() {
                let a = alpha_at(&m.textures[bi][i], (px - x) / side, (py - y) / side);
                let field = m.colors[bi][i];
                for (k, ch) in [field.r, field.g, field.b].into_iter().enumerate() {
                    c[k] = c[k] * (1.0 - a) + f64::from(ch) * a;
                }
            }
            sum += lightness(c.map(|v| v * (1.0 - look.lift) + 255.0 * look.lift));
        }
    }
    sum / (GRID.0 * GRID.1) as f64
}

const fn drift_only(at_ms: f64) -> Look {
    Look {
        at_ms,
        breathing: false,
        scale: 1.0,
        lift: 0.0,
    }
}

/// Redraw times at `fps` with every gap off by up to 35 % (a fixed pseudo-random sequence).
fn frame_times(fps: f64, frames: usize) -> Vec<Instant> {
    let t0 = Instant::now();
    let mut seed = 0x2545_F491_u32;
    let mut at = 0.0;
    (0..frames)
        .map(|_| {
            seed = seed.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
            let jitter = 1.0 + (f64::from(seed >> 8) / f64::from(1_u32 << 24) - 0.5) * 0.7;
            at += 1000.0 / fps * jitter;
            t0 + Duration::from_secs_f64(at / 1000.0)
        })
        .collect()
}

#[test]
fn a_breath_never_changes_the_window_by_more_than_one_and_a_half_points() {
    let cycle = f64::from(motion::HEART_CYCLE_MS);
    let mut worst = 0.0_f64;
    for (name, rgba) in covers() {
        let m = mesh::build(&rgba);
        for base in MODES {
            for start in [0.0, 7_300.0, 19_100.0, 33_700.0] {
                let l: Vec<f64> = (0..=52)
                    .map(|k| {
                        let at_ms = start + cycle * f64::from(k) / 52.0;
                        mean_lightness(
                            &m,
                            base,
                            Look {
                                breathing: true,
                                ..drift_only(at_ms)
                            },
                        )
                    })
                    .collect();
                let swing = l.iter().fold(f64::MIN, |a, &b| a.max(b))
                    - l.iter().fold(f64::MAX, |a, &b| a.min(b));
                worst = worst.max(swing);
                assert!(
                    swing <= CYCLE_SWING_L,
                    "{name} {base:?} {start}: {swing:.3}"
                );
            }
        }
    }
    eprintln!("breath: worst swing over one cycle {worst:.3} L* points");
}

#[test]
fn no_frame_jumps_at_24_or_60_fps_with_uneven_frame_times() {
    let m = mesh::build(&covers()[5].1);
    let bounds = (0.0, 0.0, 1920.0, 1080.0);
    for fps in [24.0, 60.0] {
        for base in MODES {
            let (mut clock, mut prev) = (Clock::default(), None);
            let (mut max_l, mut max_px, mut max_scale) = (0.0_f64, 0.0_f32, 0.0_f32);
            for now in frame_times(fps, 600) {
                clock.tick(now, true);
                let rects: Vec<_> = (0..m.n)
                    .map(|i| drift::field_rect(bounds, i, clock.elapsed_ms, 1.0, true))
                    .collect();
                let l = mean_lightness(
                    &m,
                    base,
                    Look {
                        breathing: true,
                        ..drift_only(clock.elapsed_ms)
                    },
                );
                if let Some((last_rects, last_l)) = prev.replace((rects.clone(), l)) {
                    max_l = max_l.max((l - last_l).abs());
                    for (a, b) in rects.iter().zip(&last_rects) {
                        // The centre of the field: its corner also moves as the field grows.
                        let (ca, cb) = (
                            (a.0 + a.2 / 2.0, a.1 + a.2 / 2.0),
                            (b.0 + b.2 / 2.0, b.1 + b.2 / 2.0),
                        );
                        max_px = max_px.max((ca.0 - cb.0).abs().max((ca.1 - cb.1).abs()));
                        max_scale = max_scale.max((a.2 / b.2 - 1.0).abs());
                    }
                }
            }
            eprintln!(
                "{fps} fps {base:?}: step {max_l:.4} L*, {max_px:.2} px, scale {max_scale:.5}"
            );
            assert!(max_l <= FRAME_STEP_L / 5.0, "{fps} fps {base:?}: {max_l}");
            assert!(max_px <= 2.5, "{fps} fps: moved {max_px} px in one frame");
            assert!(
                max_scale <= 0.002,
                "{fps} fps: scaled {max_scale} in one frame"
            );
        }
    }
}

/// The heartbeat of song-colour-theme (cycle 1.6 s, two beats), as it was drawn before.
fn old_beat(ms: f64) -> f64 {
    let phase = ms.rem_euclid(1600.0) / 1600.0;
    let bump = |at: f64| {
        let d = (phase - at).abs();
        if d >= 0.10 {
            0.0
        } else {
            0.5 * (1.0 + (std::f64::consts::PI * d / 0.10).cos())
        }
    };
    (bump(0.10) + 0.7 * bump(0.34)).clamp(0.0, 1.0)
}

#[test]
fn the_old_white_lift_was_the_flash() {
    let m = mesh::build(&covers()[5].1);
    for fps in [24.0, 60.0] {
        let step = 1000.0 / fps;
        let l: Vec<f64> = (0..200)
            .map(|k| {
                let at_ms = f64::from(k) * step;
                let pulse = old_beat(at_ms);
                let look = Look {
                    at_ms,
                    breathing: false,
                    scale: 1.0 + 0.025 * pulse as f32,
                    lift: 0.05 * pulse,
                };
                mean_lightness(&m, BaseMode::Dark, look)
            })
            .collect();
        let step_l = l
            .windows(2)
            .fold(0.0_f64, |a, w| a.max((w[1] - w[0]).abs()));
        let swing =
            l.iter().fold(f64::MIN, |a, &b| a.max(b)) - l.iter().fold(f64::MAX, |a, &b| a.min(b));
        eprintln!("old heartbeat, dark, {fps} fps: step {step_l:.2} L*, swing {swing:.2} L*");
        assert!(swing > 4.0, "the lift swings the window by {swing} points");
        assert!(step_l > FRAME_STEP_L, "and by {step_l} points in one frame");
    }
}

fn playing_app() -> App {
    build_app(
        &mock_backend(),
        &Scenario {
            route: Route::Home,
            track: 1,
            playing: true,
            full_motion: true,
            ..Scenario::default()
        },
    )
}

fn frame(app: &mut App, t0: Instant, k: u32) {
    let at = t0 + Duration::from_micros(u64::from(k) * 16_667);
    drop(app.update_core(Message::Frame(at)));
    drop(
        app.update_core(Message::Ui(Event::Position(Duration::from_millis(
            u64::from(k) * 17,
        )))),
    );
}

#[test]
fn building_the_backdrop_again_reuses_the_image_handles() {
    let app = playing_app();
    let cover = app.cover.clone().expect("the mock song has a cover");
    let (under_a, a) = window_layers(&app, &cover);
    let (under_b, b) = window_layers(&app, &cover);
    assert_eq!(under_a, under_b);
    assert_eq!(a.len(), 1, "settled: one layer, no old cover under it");
    assert!((a[0].opacity - 1.0).abs() < 1e-6 && (a[0].scale - 1.0).abs() < 1e-6);
    assert_eq!(a[0].blobs.len(), b[0].blobs.len());
    for (x, y) in a[0].blobs.iter().zip(&b[0].blobs) {
        assert_eq!(
            x.id(),
            y.id(),
            "a new texture id per frame would re-upload it"
        );
    }
    assert_eq!(a[0].still.id(), b[0].still.id());
}

#[test]
fn one_frame_clock_drives_the_window_while_a_song_plays() {
    let mut app = playing_app();
    assert!(moves(&app) && app.needs_frames());
    let playing = app.subscription().units();
    drop(app.update_core(Message::Ui(Event::Command(PlayerCommand::Pause))));
    // The accent and the panes settle within a second, then nothing asks for a frame.
    let t0 = Instant::now();
    for k in 0..90 {
        frame(&mut app, t0, k);
    }
    assert!(!moves(&app), "paused: the backdrop holds still");
    assert!(
        !app.needs_frames(),
        "paused: no frames at all (ui {}, anim {}, scheme {}, bg {}, micro {}, surfaces {})",
        app.ui.needs_frames(),
        app.anim.active(),
        !app.scheme_fade.is_done(),
        !app.bg_fade.is_done(),
        !app.micro.settled(),
        !app.surfaces.settled()
    );
    let paused = app.subscription().units();
    assert_eq!(
        playing - paused,
        1,
        "the frame ticker is the only extra source"
    );
    // It never stops and starts again on its own while the song plays.
    let mut app = playing_app();
    let t0 = Instant::now();
    for k in 0..600 {
        frame(&mut app, t0, k);
        assert!(app.needs_frames(), "frame {k}");
    }
}

#[test]
fn the_stop_rules_stop_the_frames() {
    let mut app = playing_app();
    drop(app.update_core(Message::Focus(false)));
    assert!(!app.needs_frames(), "unfocused");
    drop(app.update_core(Message::Focus(true)));
    assert!(app.needs_frames());
    app.ui.settings.reduce_transparency = true;
    assert!(!moves(&app), "reduce transparency");
    app.ui.settings.reduce_transparency = false;
    app.ui.settings.song_colour_theme = false;
    assert!(!moves(&app), "song colours off");
    app.ui.settings.song_colour_theme = true;
    drop(app.update_core(Message::Ui(Event::Go(Route::NowPlaying))));
    assert!(!moves(&app), "full-screen Now Playing hides the backdrop");
}

#[test]
fn the_bloom_runs_once_per_song_change_and_never_on_position_ticks() {
    let mut app = playing_app();
    let t0 = Instant::now();
    let first = app.cover.clone().expect("a cover").track;
    for k in 0..300 {
        frame(&mut app, t0, k);
        drop(app.update_core(Message::Player(PlayerEvent::StateChanged(
            PlayState::Playing,
        ))));
        assert!(
            (app.anim.bloom() - 1.0).abs() < 1e-6,
            "tick {k} restarted the bloom"
        );
        assert!(app.prev_cover.is_none());
    }
    let jobs = app.update_core(Message::Ui(Event::Command(PlayerCommand::Play(
        crate::mock::track(9),
    ))));
    app.settle_blocking(jobs);
    assert_ne!(app.cover.as_ref().map(|c| c.track.clone()), Some(first));
    assert!(app.anim.bloom() < 0.05, "the new song starts the bloom");
    let mut last = app.anim.bloom();
    for k in 300..420 {
        frame(&mut app, t0, k);
        drop(app.update_core(Message::Player(PlayerEvent::StateChanged(
            PlayState::Playing,
        ))));
        let now = app.anim.bloom();
        assert!(
            now >= last - 1e-6,
            "the bloom went back at tick {k}: {last} to {now}"
        );
        last = now;
    }
    assert!((last - 1.0).abs() < 1e-6, "done within 2 s: {last}");
}
