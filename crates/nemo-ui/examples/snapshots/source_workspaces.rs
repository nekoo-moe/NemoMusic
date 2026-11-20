//! Shots of the two listening areas (source-workspaces spec): the `Listening area` control in
//! the account sheet and in Settings, the sidebar and Library of each area, and the states where
//! SoundCloud is not offered. Dark and light, 960 x 600 and 1366 x 697.

use nemo_core::source_account::SourceAccountState;
use nemo_ui::gui::app::{ScTab, SettingsSection, Source};
use nemo_ui::gui::headless::soundcloud::{ScLibraryScene, ScScene};
use nemo_ui::gui::headless::{Scenario, Sheet};
use nemo_ui::mock::MockScAccount;
use nemo_ui::route::Route;
use nemo_ui::scheme::BaseMode;
use nemo_ui::settings::UiSettings;

use super::{Shot, shot};

const SMALL: (u32, u32) = (960, 600);
const LARGE: (u32, u32) = (1366, 697);
/// Tall enough to show the whole sheet without scrolling.
const TALL: (u32, u32) = (960, 1000);

fn tag(base: BaseMode) -> &'static str {
    if base == BaseMode::Light {
        "light"
    } else {
        "dark"
    }
}

fn dims(size: (u32, u32)) -> String {
    format!("{}x{}", size.0, size.1)
}

fn signed_in() -> SourceAccountState {
    SourceAccountState::SignedIn(MockScAccount::info())
}

fn base_scenario(base: BaseMode, size: (u32, u32), route: Route) -> Scenario {
    Scenario {
        route,
        size,
        settings: UiSettings {
            base,
            tos_acknowledged: true,
            ..UiSettings::default()
        },
        ..Scenario::default()
    }
}

/// A shot with SoundCloud running and its account in `state`.
fn running(name: String, scenario: Scenario, state: SourceAccountState) -> Shot {
    Shot {
        soundcloud: Some(state),
        ..shot(name, scenario)
    }
}

/// The account sheet over the page of the area.
fn sheet(base: BaseMode, size: (u32, u32), area: Source, route: Route) -> Scenario {
    Scenario {
        sheet: Sheet::SignIn,
        soundcloud: Some(ScScene::Account),
        area: Some(area),
        ..base_scenario(base, size, route)
    }
}

pub fn shots(out: &mut Vec<Shot>) {
    for base in [BaseMode::Dark, BaseMode::Light] {
        for size in [SMALL, LARGE] {
            let t = tag(base);
            let d = dims(size);
            out.push(running(
                format!("area-sheet-{t}-{d}"),
                sheet(base, size, Source::YouTube, Route::Home),
                signed_in(),
            ));
            out.push(running(
                format!("area-sheet-soundcloud-{t}-{d}"),
                sheet(base, size, Source::SoundCloud, Route::Library),
                signed_in(),
            ));
            out.push(running(
                format!("area-settings-{t}-{d}"),
                Scenario {
                    section: SettingsSection::SoundCloud,
                    soundcloud: Some(ScScene::Account),
                    ..base_scenario(base, size, Route::Settings)
                },
                signed_in(),
            ));
            out.push(running(
                format!("area-library-soundcloud-{t}-{d}"),
                Scenario {
                    soundcloud: Some(ScScene::Account),
                    sc_library: Some(ScLibraryScene::Tab(ScTab::Likes)),
                    area: Some(Source::SoundCloud),
                    ..base_scenario(base, size, Route::Library)
                },
                signed_in(),
            ));
            out.push(running(
                format!("area-library-youtube-{t}-{d}"),
                Scenario {
                    soundcloud: Some(ScScene::Account),
                    ..base_scenario(base, size, Route::Library)
                },
                signed_in(),
            ));
        }
    }
    // SoundCloud not offered: signed out, and off. One sentence instead of the control.
    out.push(running(
        "area-sheet-signed-out-dark-960x600".to_owned(),
        Scenario {
            soundcloud: Some(ScScene::Account),
            ..sheet(BaseMode::Dark, SMALL, Source::YouTube, Route::Home)
        },
        SourceAccountState::SignedOut,
    ));
    out.push(shot(
        "area-sheet-off-dark-960x600".to_owned(),
        Scenario {
            sheet: Sheet::SignIn,
            soundcloud: Some(ScScene::Off),
            ..base_scenario(BaseMode::Dark, SMALL, Route::Home)
        },
    ));
    out.push(shot(
        "area-settings-off-dark-960x600".to_owned(),
        Scenario {
            section: SettingsSection::SoundCloud,
            soundcloud: Some(ScScene::Off),
            ..base_scenario(BaseMode::Dark, SMALL, Route::Settings)
        },
    ));
}

/// The sheet with both accounts signed in, in the state `scene` gives the SoundCloud card.
fn both(name: String, base: BaseMode, size: (u32, u32), scene: ScScene) -> Shot {
    Shot {
        signed_in: true,
        ..running(
            name,
            Scenario {
                sheet: Sheet::SignedIn,
                soundcloud: Some(scene),
                ..base_scenario(base, size, Route::Home)
            },
            signed_in(),
        )
    }
}

/// The `Your accounts` sheet (PR 2): both accounts, the SoundCloud card in each state, and the
/// sheet in a window half the size (what 200% scale leaves of 960 x 600).
pub fn account_shots(out: &mut Vec<Shot>) {
    for base in [BaseMode::Dark, BaseMode::Light] {
        for size in [SMALL, LARGE] {
            out.push(both(
                format!("accounts-both-{}-{}", tag(base), dims(size)),
                base,
                size,
                ScScene::Account,
            ));
        }
    }
    out.push(both(
        "accounts-both-confirm-dark-960x1000".to_owned(),
        BaseMode::Dark,
        TALL,
        ScScene::ConfirmSignOut,
    ));
    out.push(both(
        "accounts-both-scale200-dark-480x300".to_owned(),
        BaseMode::Dark,
        (480, 300),
        ScScene::Account,
    ));
    out.push(running(
        "accounts-paste-dark-960x1000".to_owned(),
        Scenario {
            sheet: Sheet::SignIn,
            soundcloud: Some(ScScene::Paste),
            ..base_scenario(BaseMode::Dark, TALL, Route::Home)
        },
        SourceAccountState::SignedOut,
    ));
    out.push(running(
        "accounts-busy-dark-960x1000".to_owned(),
        Scenario {
            sheet: Sheet::SignIn,
            soundcloud: Some(ScScene::Busy),
            ..base_scenario(BaseMode::Dark, TALL, Route::Home)
        },
        SourceAccountState::SignedOut,
    ));
    out.push(running(
        "accounts-expired-dark-960x1000".to_owned(),
        Scenario {
            sheet: Sheet::SignIn,
            soundcloud: Some(ScScene::Account),
            ..base_scenario(BaseMode::Dark, TALL, Route::Home)
        },
        SourceAccountState::Expired,
    ));
}
