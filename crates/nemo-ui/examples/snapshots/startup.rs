//! Shots of the start-up splash (startup-flow task 1): each base mode, the minimum window, the
//! slow note, the failure and a frame in the middle of the fade.

use std::path::Path;

use nemo_ui::gui::app::Message;
use nemo_ui::gui::headless::render_view_png;
use nemo_ui::gui::views::splash::{SplashLook, view};
use nemo_ui::scheme::{AccentIntensity, BaseMode, ThemeChoice, accent_palette};
use nemo_ui::splash::{Splash, StartStep};
use nemo_ui::theme::color;

/// One splash shot: name, base mode, window, steps finished, and what it shows.
struct Shot {
    name: &'static str,
    base: BaseMode,
    size: (u32, u32),
    kind: Kind,
}

#[derive(Clone, Copy)]
enum Kind {
    /// Three of five steps are in.
    Progress,
    /// Six seconds in, still not ready.
    Slow,
    /// The start failed.
    Failed,
    /// Half way through the fade (opacity 0.5).
    Fade,
}

const SHOTS: [Shot; 7] = [
    Shot {
        name: "startup-splash-dark-960x600",
        base: BaseMode::Dark,
        size: (960, 600),
        kind: Kind::Progress,
    },
    Shot {
        name: "startup-splash-amoled-960x600",
        base: BaseMode::Amoled,
        size: (960, 600),
        kind: Kind::Progress,
    },
    Shot {
        name: "startup-splash-light-960x600",
        base: BaseMode::Light,
        size: (960, 600),
        kind: Kind::Progress,
    },
    Shot {
        name: "startup-splash-dark-1366x697",
        base: BaseMode::Dark,
        size: (1366, 697),
        kind: Kind::Progress,
    },
    Shot {
        name: "startup-splash-slow-dark-960x600",
        base: BaseMode::Dark,
        size: (960, 600),
        kind: Kind::Slow,
    },
    Shot {
        name: "startup-splash-failed-light-960x600",
        base: BaseMode::Light,
        size: (960, 600),
        kind: Kind::Failed,
    },
    Shot {
        name: "startup-splash-fade-dark-960x600",
        base: BaseMode::Dark,
        size: (960, 600),
        kind: Kind::Fade,
    },
];

/// Render the splash shots named in `only` (all when empty) into `out_dir`.
pub(super) fn render(only: &[String], out_dir: &Path) -> Result<(), String> {
    for shot in SHOTS {
        if !only.is_empty() && !only.iter().any(|n| n == shot.name) {
            continue;
        }
        // The colour roles are global: set them right before the view is built and drawn.
        color::set_active(shot.base.neutrals());
        let mut splash = Splash::new(false);
        splash.finish(StartStep::Library, 40);
        splash.finish(StartStep::CredentialStore, 90);
        let now = match shot.kind {
            Kind::Slow => 6_200,
            Kind::Failed => {
                splash.fail(
                    "NemoMusic could not start. Cannot start lyrics: the data folder is read-only."
                        .to_owned(),
                );
                300
            }
            Kind::Fade => {
                splash.finish(StartStep::Player, 100);
                splash.finish(StartStep::FirstScreen, 120);
                splash.tick(500);
                600
            }
            Kind::Progress => 300,
        };
        let status = splash.status(now);
        let look = SplashLook {
            fraction: splash.fraction(),
            status: &status,
            opacity: splash.opacity(now),
            accent: accent_palette(
                ThemeChoice::Dynamic,
                shot.base,
                AccentIntensity::Standard,
                None,
            ),
            failed: splash.failure().is_some(),
        };
        let path = out_dir.join(format!("{}.png", shot.name));
        render_view_png(view::<Message>(&look, Message::Noop), shot.size, &path)
            .map_err(|e| format!("{}: {e}", shot.name))?;
        println!("wrote {}", path.display());
    }
    color::set_active(BaseMode::Dark.neutrals());
    Ok(())
}
