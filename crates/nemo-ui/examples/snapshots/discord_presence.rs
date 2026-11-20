//! Shots of Settings, Privacy with the Discord presence row (discord-presence, PR 3): dark and
//! light, 960 x 600 and 1366 x 697, with the switch off and on; the panel that opens when the
//! switch is turned on (dark, 960 x 600); and a tall window with the whole card, on and with
//! `Hide song names on Discord` on.

use std::path::Path;
use std::sync::Arc;

use nemo_core::integrations::IntegrationsService;
use nemo_core::opt_in::OptInInfo;
use nemo_core::services::Backend;
use nemo_ui::gui::app::{DiscordMsg, Message, SettingsSection};
use nemo_ui::gui::headless::{Scenario, build_app, render_png};
use nemo_ui::mock::{MockIntegrations, mock_backend};
use nemo_ui::route::Route;
use nemo_ui::scheme::BaseMode;
use nemo_ui::settings::UiSettings;

const SMALL: (u32, u32) = (960, 600);
const LARGE: (u32, u32) = (1366, 697);
const TALL: (u32, u32) = (960, 1000);

/// The notice of `nemo_integrations::info::DISCORD`, copied word for word: this crate does not
/// depend on `nemo-integrations`.
const DISCORD: OptInInfo = OptInInfo {
    id: "discord",
    title: "Discord presence",
    sends: "The title and artist of the song you play, with its source (YouTube Music, SoundCloud or a local file), the address of its cover and the position in the song, and whether you are paused, idle, browsing or away, to the Discord app on this computer. File names and paths are never sent.",
    hosts: &[],
    risk: "They are shown on your Discord profile, and your Discord friends and server members can see them, as your Discord privacy settings allow. Discord's own servers load the cover from its address. The switch under this one hides the song names.",
    version: 1,
    default_on: false,
};

/// Where the row stands in a shot.
#[derive(Clone, Copy)]
enum Scene {
    Off,
    Panel,
    On,
    OnHidden,
}

fn tag(base: BaseMode) -> &'static str {
    if base == BaseMode::Light {
        "light"
    } else {
        "dark"
    }
}

fn scenario(base: BaseMode, size: (u32, u32)) -> Scenario {
    Scenario {
        route: Route::Settings,
        section: SettingsSection::Privacy,
        size,
        settings: UiSettings {
            base,
            tos_acknowledged: true,
            ..UiSettings::default()
        },
        ..Scenario::default()
    }
}

fn shot(
    name: &str,
    base: BaseMode,
    size: (u32, u32),
    scene: Scene,
    dir: &Path,
) -> Result<(), String> {
    let service = Arc::new(MockIntegrations::new(DISCORD));
    let backend = Backend {
        integrations: Arc::clone(&service) as _,
        ..mock_backend()
    };
    if matches!(scene, Scene::On | Scene::OnHidden) {
        service.set_discord(true);
    }
    service.set_discord_private(matches!(scene, Scene::OnHidden));
    let mut app = build_app(&backend, &scenario(base, size));
    if matches!(scene, Scene::Panel) {
        drop(app.update(Message::Discord(DiscordMsg::Ask)));
    }
    let path = dir.join(format!("{name}.png"));
    render_png(&app, size, &path)?;
    println!("wrote {}", path.display());
    Ok(())
}

/// Render every shot of this spec whose name is in `only` (all when it is empty).
pub(super) fn render(only: &[String], dir: &Path) -> Result<(), String> {
    for base in [BaseMode::Dark, BaseMode::Light] {
        for (size, dims) in [(SMALL, "960x600"), (LARGE, "1366x697")] {
            for (stem, scene) in [("off", Scene::Off), ("on", Scene::On)] {
                let name = format!("discord-privacy-{stem}-{}-{dims}", tag(base));
                if only.is_empty() || only.contains(&name) {
                    shot(&name, base, size, scene, dir)?;
                }
            }
        }
    }
    // A tall window, so the whole card shows, with `Hide song names on Discord` under it.
    for base in [BaseMode::Dark, BaseMode::Light] {
        let name = format!("discord-privacy-hidden-{}-960x1000", tag(base));
        if only.is_empty() || only.contains(&name) {
            shot(&name, base, TALL, Scene::OnHidden, dir)?;
        }
    }
    let name = "discord-privacy-panel-dark-960x600".to_owned();
    if only.is_empty() || only.contains(&name) {
        shot(&name, BaseMode::Dark, SMALL, Scene::Panel, dir)?;
    }
    Ok(())
}
