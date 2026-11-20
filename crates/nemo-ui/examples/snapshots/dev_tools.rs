//! Shots of the hidden developer panel (dev-tools spec): every tab, dark and light, 960 x 600 and
//! 1366 x 697, over the invented session of `nemo_ui::mock::sample_snapshot`. The panel opens
//! over Settings, About, where the ten clicks happen.

use nemo_ui::gui::app::SettingsSection;
use nemo_ui::gui::app::dev::DevTab;
use nemo_ui::gui::headless::Scenario;
use nemo_ui::route::Route;
use nemo_ui::scheme::BaseMode;
use nemo_ui::settings::UiSettings;

use super::{Shot, shot};

const SIZES: [((u32, u32), &str); 2] = [((960, 600), "960x600"), ((1366, 697), "1366x697")];

fn tag(base: BaseMode) -> &'static str {
    if base == BaseMode::Light {
        "light"
    } else {
        "dark"
    }
}

fn stem(tab: DevTab) -> &'static str {
    match tab {
        DevTab::Trace => "trace",
        DevTab::Log => "log",
        DevTab::Providers => "providers",
        DevTab::Account => "account",
        DevTab::System => "system",
    }
}

/// Every shot of this spec.
pub(super) fn shots(out: &mut Vec<Shot>) {
    for base in [BaseMode::Dark, BaseMode::Light] {
        for (size, dims) in SIZES {
            for tab in DevTab::ALL {
                let scenario = Scenario {
                    route: Route::Settings,
                    section: SettingsSection::About,
                    size,
                    settings: UiSettings {
                        base,
                        tos_acknowledged: true,
                        ..UiSettings::default()
                    },
                    dev: Some(tab),
                    ..Scenario::default()
                };
                out.push(shot(
                    format!("dev-{}-{}-{dims}", stem(tab), tag(base)),
                    scenario,
                ));
            }
        }
    }
}
