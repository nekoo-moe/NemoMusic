//! Shots of the brand intro (brand-intro task 1): five key moments in dark and light (t = 0, 300,
//! 700, 1100 ms and the exit), then amoled, the larger window, reduced motion, the slow note and
//! the failure.

use std::path::Path;

use iced::widget::image::Handle;
use nemo_ui::gui::app::Message;
use nemo_ui::gui::headless::render_view_png;
use nemo_ui::gui::views::splash::{SplashLook, view};
use nemo_ui::scheme::{AccentIntensity, BaseMode, ThemeChoice, accent_palette};
use nemo_ui::splash::intro::{BLOOM_PEAK, BLOOM_PX, bloom_rgba};
use nemo_ui::splash::{Splash, StartStep};
use nemo_ui::theme::color;

/// One splash shot: name, base mode, window, and what moment it shows.
struct Shot {
    name: &'static str,
    base: BaseMode,
    size: (u32, u32),
    kind: Kind,
}

#[derive(Clone, Copy)]
enum Kind {
    /// The intro `ms` after the first frame, with the steps a normal start has finished by then.
    At(u64),
    /// Reduce motion: the intro at rest.
    Reduced,
    /// Six seconds in, still not ready.
    Slow,
    /// The start failed.
    Failed,
    /// Half way through the exit (opacity about 0.5).
    Exit,
}

const fn shot(name: &'static str, base: BaseMode, size: (u32, u32), kind: Kind) -> Shot {
    Shot {
        name,
        base,
        size,
        kind,
    }
}

const SMALL: (u32, u32) = (960, 600);
const SHOTS: [Shot; 15] = [
    shot(
        "startup-intro-dark-t0-960x600",
        BaseMode::Dark,
        SMALL,
        Kind::At(0),
    ),
    shot(
        "startup-intro-dark-t300-960x600",
        BaseMode::Dark,
        SMALL,
        Kind::At(300),
    ),
    shot(
        "startup-intro-dark-t700-960x600",
        BaseMode::Dark,
        SMALL,
        Kind::At(700),
    ),
    shot(
        "startup-intro-dark-t1100-960x600",
        BaseMode::Dark,
        SMALL,
        Kind::At(1_100),
    ),
    shot(
        "startup-intro-dark-exit-960x600",
        BaseMode::Dark,
        SMALL,
        Kind::Exit,
    ),
    shot(
        "startup-intro-light-t0-960x600",
        BaseMode::Light,
        SMALL,
        Kind::At(0),
    ),
    shot(
        "startup-intro-light-t300-960x600",
        BaseMode::Light,
        SMALL,
        Kind::At(300),
    ),
    shot(
        "startup-intro-light-t700-960x600",
        BaseMode::Light,
        SMALL,
        Kind::At(700),
    ),
    shot(
        "startup-intro-light-t1100-960x600",
        BaseMode::Light,
        SMALL,
        Kind::At(1_100),
    ),
    shot(
        "startup-intro-light-exit-960x600",
        BaseMode::Light,
        SMALL,
        Kind::Exit,
    ),
    shot(
        "startup-intro-amoled-t1100-960x600",
        BaseMode::Amoled,
        SMALL,
        Kind::At(1_100),
    ),
    shot(
        "startup-intro-dark-t1100-1366x697",
        BaseMode::Dark,
        (1366, 697),
        Kind::At(1_100),
    ),
    shot(
        "startup-intro-reduced-dark-960x600",
        BaseMode::Dark,
        SMALL,
        Kind::Reduced,
    ),
    shot(
        "startup-intro-slow-dark-960x600",
        BaseMode::Dark,
        SMALL,
        Kind::Slow,
    ),
    shot(
        "startup-intro-failed-light-960x600",
        BaseMode::Light,
        SMALL,
        Kind::Failed,
    ),
];

/// The steps a normal start has finished `ms` after the first frame.
fn finish_steps(splash: &mut Splash, ms: u64) {
    for (step, at) in [
        (StartStep::Library, 120),
        (StartStep::CredentialStore, 400),
        (StartStep::Player, 650),
        (StartStep::FirstScreen, 900),
    ] {
        if ms >= at {
            splash.finish(step, at);
        }
    }
}

/// Render the splash shots named in `only` (all when empty) into `out_dir`.
pub(super) fn render(only: &[String], out_dir: &Path) -> Result<(), String> {
    for shot in SHOTS {
        if !only.is_empty() && !only.iter().any(|n| n == shot.name) {
            continue;
        }
        // The colour roles are global: set them right before the view is built and drawn.
        color::set_active(shot.base.neutrals());
        let reduced = matches!(shot.kind, Kind::Reduced);
        let mut splash = Splash::new(reduced);
        let now = match shot.kind {
            Kind::At(ms) => {
                finish_steps(&mut splash, ms);
                ms
            }
            Kind::Reduced => {
                finish_steps(&mut splash, 1_100);
                1_100
            }
            Kind::Slow => {
                finish_steps(&mut splash, 400);
                6_200
            }
            Kind::Failed => {
                finish_steps(&mut splash, 400);
                splash.fail(
                    "NemoMusic could not start. Cannot start lyrics: the data folder is read-only."
                        .to_owned(),
                );
                1_300
            }
            Kind::Exit => {
                finish_steps(&mut splash, 1_100);
                splash.tick(1_200);
                1_360
            }
        };
        let status = splash.status(now);
        let accent = accent_palette(
            ThemeChoice::Dynamic,
            shot.base,
            AccentIntensity::Standard,
            None,
        );
        let bloom = Handle::from_rgba(BLOOM_PX, BLOOM_PX, bloom_rgba(accent.primary, BLOOM_PEAK));
        let look = SplashLook {
            fraction: splash.fraction(),
            status: &status,
            opacity: splash.opacity(now),
            accent,
            failed: splash.failure().is_some(),
            t_ms: now,
            reduced,
            bloom: &bloom,
        };
        let path = out_dir.join(format!("{}.png", shot.name));
        render_view_png(view::<Message>(&look, Message::Noop), shot.size, &path)
            .map_err(|e| format!("{}: {e}", shot.name))?;
        println!("wrote {}", path.display());
    }
    color::set_active(BaseMode::Dark.neutrals());
    Ok(())
}
