//! Shots of the hidden developer panel (dev-tools spec): every tab, dark and light, 960 x 600 and
//! 1366 x 697, over the invented session of `nemo_ui::mock::sample_snapshot`. The panel opens
//! over Settings, About, where the ten clicks happen.

use nemo_ui::gui::app::SettingsSection;
use nemo_ui::gui::app::dev::{DevTab, UiFacts};
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

/// Write the report that `Copy report` would copy for the invented session, next to the images.
pub(super) fn write_report(only: &[String], out_dir: &std::path::Path) -> std::io::Result<()> {
    if !only.is_empty() && !only.iter().any(|n| n == "dev-report-sample") {
        return Ok(());
    }
    let facts = UiFacts {
        frames: Some("avg 16.7 ms, worst 17.1 ms, 60 frames".to_owned()),
        window: "960 x 600 px, scale 200%".to_owned(),
        reduce_motion: false,
    };
    let text = nemo_ui::gui::app::dev::report::text(&nemo_ui::mock::sample_snapshot(), &facts);
    let path = out_dir.join("dev-report-sample.txt");
    std::fs::write(&path, text)?;
    println!("wrote {}", path.display());
    Ok(())
}
