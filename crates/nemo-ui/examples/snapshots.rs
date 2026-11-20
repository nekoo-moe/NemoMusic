//! Renders the reference screens headless to `docs/ui/snapshots/*.png` (tiny-skia, no GPU).
//!
//! ```text
//! $env:ICED_TEST_BACKEND = 'tiny-skia'
//! cargo run -p nemo-ui --example snapshots --features gui
//! ```
//!
//! Images are 2x scale (2732 x 1394 for a 1366 x 697 window). Pass names as arguments to render
//! only those (for example `home-1366x697`). Set `NEMO_SNAPSHOT_DIR` to write somewhere else.

#[path = "snapshots/appearance_page.rs"]
mod appearance_page;
#[path = "snapshots/backdrop.rs"]
mod backdrop;
#[path = "snapshots/feature.rs"]
mod feature;
#[path = "snapshots/font_compare.rs"]
mod font_compare;
#[path = "snapshots/glass.rs"]
mod glass;
#[path = "snapshots/lyrics_polish.rs"]
mod lyrics_polish;
#[path = "snapshots/lyrics_rebuild.rs"]
mod lyrics_rebuild;
#[path = "snapshots/motion.rs"]
mod motion;
#[path = "snapshots/np_classic.rs"]
mod np_classic;
#[path = "snapshots/opt_in.rs"]
mod opt_in;
#[path = "snapshots/player_bar.rs"]
mod player_bar;
#[path = "snapshots/screen.rs"]
mod screen;
#[path = "snapshots/setup.rs"]
mod setup;
#[path = "snapshots/skeleton.rs"]
mod skeleton;
#[path = "snapshots/song_colour.rs"]
mod song_colour;
#[path = "snapshots/soundcloud_account.rs"]
mod soundcloud_account;
#[path = "snapshots/startup.rs"]
mod startup;
#[path = "snapshots/state.rs"]
mod state;
#[path = "snapshots/syllable_motion.rs"]
mod syllable_motion;
#[path = "snapshots/theme.rs"]
mod theme;
#[path = "snapshots/wave_lyrics.rs"]
mod wave_lyrics;

use std::path::PathBuf;

use nemo_core::services::Backend;
use nemo_core::source_account::SourceAccountState;
use nemo_ui::gui::headless::{Scenario, build_app, render_png, render_view_png, specimen};
use nemo_ui::mock::{
    mock_backend, mock_backend_seeded, mock_backend_signed_in, mock_backend_soundcloud,
    mock_backend_soundcloud_empty,
};

/// The two window sizes the owner cares about: the maximized 1366 x 768 laptop and full HD.
const SIZES: [(u32, u32); 2] = [(1366, 697), (1920, 1040)];

/// One image to render.
struct Shot {
    name: String,
    seeded: bool,
    signed_in: bool,
    /// SoundCloud runs in the backend, with its account in this state.
    soundcloud: Option<SourceAccountState>,
    /// With `soundcloud`: the account has nothing in it.
    empty: bool,
    scenario: Scenario,
}

fn shot(name: String, scenario: Scenario) -> Shot {
    Shot {
        name,
        seeded: false,
        signed_in: false,
        soundcloud: None,
        empty: false,
        scenario,
    }
}

fn named(stem: &str, size: (u32, u32)) -> String {
    format!("{stem}-{}x{}", size.0, size.1)
}

fn scenarios() -> Vec<Shot> {
    let mut out = Vec::new();
    screen::screens(&mut out);
    state::states(&mut out);
    theme::themes(&mut out);
    feature::features(&mut out);
    backdrop::backdrops(&mut out);
    motion::shots(&mut out);
    setup::shots(&mut out);
    song_colour::shots(&mut out);
    soundcloud_account::shots(&mut out);
    soundcloud_account::library_shots(&mut out);
    soundcloud_account::search_shots(&mut out);
    soundcloud_account::sidebar_shots(&mut out);
    skeleton::shots(&mut out);
    out.extend(wave_lyrics::shots());
    out.extend(syllable_motion::shots());
    out
}

fn main() {
    if std::env::var("ICED_TEST_BACKEND").as_deref() != Ok("tiny-skia") {
        eprintln!("set ICED_TEST_BACKEND=tiny-skia first (headless rendering without a GPU)");
        std::process::exit(2);
    }
    let only: Vec<String> = std::env::args().skip(1).collect();
    let out_dir = std::env::var_os("NEMO_SNAPSHOT_DIR").map_or_else(
        || PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../docs/ui/snapshots"),
        PathBuf::from,
    );
    if only.is_empty() || only.iter().any(|n| n == "specimen-vietnamese") {
        let path = out_dir.join("specimen-vietnamese.png");
        match render_view_png(specimen(), (1100, 420), &path) {
            Ok(()) => println!("wrote {}", path.display()),
            Err(e) => {
                eprintln!("specimen: {e}");
                std::process::exit(1);
            }
        }
    }
    if only.iter().any(|n| n == "song-frame-cost") {
        song_colour::frame_cost();
        return;
    }
    if let Err(e) = np_classic::render(&only, &out_dir) {
        eprintln!("{e}");
        std::process::exit(1);
    }
    if let Err(e) = font_compare::render(&only, &out_dir) {
        eprintln!("{e}");
        std::process::exit(1);
    }
    if let Err(e) = startup::render(&only, &out_dir) {
        eprintln!("{e}");
        std::process::exit(1);
    }
    if let Err(e) = appearance_page::render(&only, &out_dir) {
        eprintln!("{e}");
        std::process::exit(1);
    }
    if let Err(e) = glass::render(&only, &out_dir) {
        eprintln!("{e}");
        std::process::exit(1);
    }
    if let Err(e) = lyrics_polish::render(&only, &out_dir) {
        eprintln!("{e}");
        std::process::exit(1);
    }
    if let Err(e) = lyrics_rebuild::render(&only, &out_dir) {
        eprintln!("{e}");
        std::process::exit(1);
    }
    if let Err(e) = player_bar::render(&only, &out_dir) {
        eprintln!("{e}");
        std::process::exit(1);
    }
    for (name, view, size) in opt_in::shots() {
        if !only.is_empty() && !only.contains(&name) {
            continue;
        }
        let path = out_dir.join(format!("{name}.png"));
        match render_view_png(view, size, &path) {
            Ok(()) => println!("wrote {}", path.display()),
            Err(e) => {
                eprintln!("{name}: {e}");
                std::process::exit(1);
            }
        }
    }
    for s in scenarios() {
        if !only.is_empty() && !only.contains(&s.name) {
            continue;
        }
        let backend: Backend = match (s.seeded, s.signed_in, s.soundcloud.clone()) {
            (_, _, Some(state)) if s.empty => mock_backend_soundcloud_empty(state),
            (_, _, Some(state)) => mock_backend_soundcloud(state),
            (true, _, None) => mock_backend_seeded(),
            (false, true, None) => mock_backend_signed_in(),
            (false, false, None) => mock_backend(),
        };
        let app = build_app(&backend, &s.scenario);
        let path = out_dir.join(format!("{}.png", s.name));
        match render_png(&app, s.scenario.size, &path) {
            Ok(()) => println!("wrote {}", path.display()),
            Err(e) => {
                eprintln!("{}: {e}", s.name);
                std::process::exit(1);
            }
        }
    }
}
