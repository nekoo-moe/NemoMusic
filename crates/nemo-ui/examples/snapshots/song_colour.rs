//! Shots of the song colour theme (song-colour-theme spec): the two switches, the rule (song
//! colours on and off), and later the mesh, the heartbeat and the bloom.

use nemo_ui::UiSettings;
use nemo_ui::gui::app::SettingsSection;
use nemo_ui::gui::headless::Scenario;
use nemo_ui::route::Route;
use nemo_ui::scheme::BaseMode;

use super::{Shot, named, shot};

/// The window size of every shot here.
const SIZE: (u32, u32) = (1366, 697);

/// The scenario every shot starts from: the window backdrop drawn as the GPU draws it (the
/// headless renderer is the CPU one, which draws a composited still).
fn shown() -> Scenario {
    Scenario {
        backdrop_ms: Some(0.0),
        ..Scenario::default()
    }
}

fn settings(base: BaseMode, song_colour_theme: bool) -> UiSettings {
    UiSettings {
        tos_acknowledged: true,
        base,
        song_colour_theme,
        ..UiSettings::default()
    }
}

/// The switches on Settings, and Home with the song colours on and off.
pub(super) fn shots(out: &mut Vec<Shot>) {
    for (stem, base, on) in [
        ("dark", BaseMode::Dark, true),
        ("off-dark", BaseMode::Dark, false),
    ] {
        out.push(shot(
            named(&format!("song-settings-{stem}"), SIZE),
            Scenario {
                route: Route::Settings,
                size: SIZE,
                section: SettingsSection::Appearance,
                settings: settings(base, on),
                // Motion on, so the heartbeat switch is usable unless the song colours are off.
                full_motion: true,
                ..shown()
            },
        ));
    }
    // The mesh for three songs (red, violet, and a third) in the three surface modes, and the
    // smallest window.
    for (song, track) in [("a", 1_usize), ("b", 9), ("c", 4)] {
        for (stem, base) in [
            ("dark", BaseMode::Dark),
            ("light", BaseMode::Light),
            ("amoled", BaseMode::Amoled),
        ] {
            out.push(shot(
                named(&format!("song-mesh-{song}-{stem}"), SIZE),
                Scenario {
                    route: Route::Home,
                    size: SIZE,
                    track,
                    settings: settings(base, true),
                    ..shown()
                },
            ));
        }
    }
    let small = (960, 600);
    motion_shots(out, small);
    setup_shots(out);
    // What the CPU renderer draws: the fields composited into one still image.
    out.push(shot(
        named("song-cpu-b-dark", small),
        Scenario {
            route: Route::Home,
            size: small,
            track: 9,
            settings: settings(BaseMode::Dark, true),
            ..Scenario::default()
        },
    ));
    out.push(shot(
        named("song-mesh-b-dark", small),
        Scenario {
            route: Route::Home,
            size: small,
            track: 9,
            settings: settings(BaseMode::Dark, true),
            ..shown()
        },
    ));
    for (stem, base, on) in [
        ("on-dark", BaseMode::Dark, true),
        ("off-dark", BaseMode::Dark, false),
        ("on-light", BaseMode::Light, true),
    ] {
        out.push(shot(
            named(&format!("song-home-{stem}"), SIZE),
            Scenario {
                route: Route::Home,
                size: SIZE,
                track: 1,
                settings: settings(base, on),
                ..shown()
            },
        ));
    }
}

/// The Look step of the setup flow with both song-colour switches usable (motion on), at the
/// minimum window. (`setup-look-*` shows them with reduced motion, the heartbeat greyed out.)
fn setup_shots(out: &mut Vec<Shot>) {
    for (stem, base) in [("dark", BaseMode::Dark), ("light", BaseMode::Light)] {
        out.push(shot(
            named(&format!("song-setup-look-{stem}"), (960, 600)),
            Scenario {
                size: (960, 600),
                settings: settings(base, true),
                setup: Some(nemo_ui::setup::SetupStep::Look),
                full_motion: true,
                ..Scenario::default()
            },
        ));
    }
}

/// The bloom from song A (red) to song B (violet) before, in the middle and after, and the
/// heartbeat at the top of the first beat and at rest, in dark and light, at the minimum window.
fn motion_shots(out: &mut Vec<Shot>, size: (u32, u32)) {
    for (stem, base) in [("dark", BaseMode::Dark), ("light", BaseMode::Light)] {
        for (when, ms) in [("before", 0_u32), ("mid", 120), ("after", 700)] {
            out.push(shot(
                named(&format!("song-bloom-{when}-{stem}"), size),
                Scenario {
                    route: Route::Home,
                    size,
                    track: 1,
                    settings: settings(base, true),
                    full_motion: true,
                    bloom: Some((9, ms)),
                    ..shown()
                },
            ));
        }
        // 160 ms is the top of "lub" in the 1.6 s cycle, 1120 ms is the rest after "dub".
        for (when, at) in [("peak", 160.0), ("rest", 1120.0)] {
            out.push(shot(
                named(&format!("song-heart-{when}-{stem}"), size),
                Scenario {
                    route: Route::Home,
                    size,
                    track: 9,
                    playing: true,
                    settings: settings(base, true),
                    full_motion: true,
                    backdrop_ms: Some(at),
                    ..shown()
                },
            ));
        }
    }
}

/// Print the frame cost of the backdrop on the CPU renderer (`song-frame-cost`).
pub(super) fn frame_cost() {
    let backend = nemo_ui::mock::mock_backend();
    match nemo_ui::gui::headless::cost::backdrop_frame_cost(&backend, SIZE, 12) {
        Ok(c) => println!(
            "frame cost at {}x{} (2x, tiny-skia, fastest of 12): flat {:.1} ms, drift {:.1} ms, \
             drift + heartbeat {:.1} ms, CPU renderer path {:.1} ms; backdrop alone: drift {:.1} \
             ms, with heartbeat {:.1} ms, CPU renderer path {:.1} ms",
            SIZE.0,
            SIZE.1,
            c.flat_ms,
            c.drift_ms,
            c.drift_beat_ms,
            c.cpu_ms,
            c.drift_ms - c.flat_ms,
            c.drift_beat_ms - c.flat_ms,
            c.cpu_ms - c.flat_ms
        ),
        Err(e) => {
            eprintln!("frame cost: {e}");
            std::process::exit(1);
        }
    }
}
