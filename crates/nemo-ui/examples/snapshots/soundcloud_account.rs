//! Shots of the SoundCloud page in Settings and the SoundCloud card of the setup flow
//! (soundcloud-account, PR 2): dark and light, 960 x 600 and 1366 x 697, with every state of the
//! account card. `Shot.soundcloud` says SoundCloud runs in the backend, with the account in that
//! state; without it the services did not start (the switch is off, or `Restart now` shows).

use nemo_core::source_account::SourceAccountState;
use nemo_ui::gui::app::SettingsSection;
use nemo_ui::gui::headless::Scenario;
use nemo_ui::gui::headless::soundcloud::ScScene;
use nemo_ui::mock::MockScAccount;
use nemo_ui::route::Route;
use nemo_ui::scheme::BaseMode;
use nemo_ui::settings::UiSettings;
use nemo_ui::setup::SetupStep;

use super::{Shot, shot};

const SMALL: (u32, u32) = (960, 600);
const LARGE: (u32, u32) = (1366, 697);

fn page(base: BaseMode, size: (u32, u32), scene: ScScene) -> Scenario {
    Scenario {
        route: Route::Settings,
        section: SettingsSection::SoundCloud,
        size,
        settings: UiSettings {
            base,
            tos_acknowledged: true,
            ..UiSettings::default()
        },
        soundcloud: Some(scene),
        ..Scenario::default()
    }
}

fn setup(base: BaseMode, scene: ScScene) -> Scenario {
    Scenario {
        size: SMALL,
        settings: UiSettings {
            base,
            tos_acknowledged: true,
            ..UiSettings::default()
        },
        setup: Some(SetupStep::Sources),
        stopped: Some(false),
        soundcloud: Some(scene),
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

fn tag(base: BaseMode) -> &'static str {
    if base == BaseMode::Light {
        "light"
    } else {
        "dark"
    }
}

fn signed_in() -> SourceAccountState {
    SourceAccountState::SignedIn(MockScAccount::info())
}

/// Every shot of this spec.
pub(super) fn shots(out: &mut Vec<Shot>) {
    for base in [BaseMode::Dark, BaseMode::Light] {
        let t = tag(base);
        for (size, dims) in [(SMALL, "960x600"), (LARGE, "1366x697")] {
            let n = |stem: &str| format!("soundcloud-{stem}-{t}-{dims}");
            if base == BaseMode::Dark {
                out.push(shot(n("off"), page(base, size, ScScene::Off)));
            }
            out.push(running(
                n("signed-out"),
                page(base, size, ScScene::Account),
                SourceAccountState::SignedOut,
            ));
            out.push(running(
                n("signed-in"),
                page(base, size, ScScene::Account),
                signed_in(),
            ));
        }
        if base == BaseMode::Dark {
            let n = |stem: &str| format!("soundcloud-{stem}-{t}-960x600");
            out.push(shot(
                n("confirming"),
                page(base, SMALL, ScScene::Confirming),
            ));
            out.push(shot(n("restart"), page(base, SMALL, ScScene::Restart)));
            out.push(running(
                n("paste"),
                page(base, SMALL, ScScene::Paste),
                SourceAccountState::SignedOut,
            ));
            out.push(running(
                n("busy"),
                page(base, SMALL, ScScene::Busy),
                SourceAccountState::SigningIn,
            ));
            out.push(running(
                n("error"),
                page(base, SMALL, ScScene::Error),
                SourceAccountState::SignedOut,
            ));
            out.push(running(
                n("expired"),
                page(base, SMALL, ScScene::Account),
                SourceAccountState::Expired,
            ));
            out.push(running(
                n("confirm-sign-out"),
                page(base, SMALL, ScScene::ConfirmSignOut),
                signed_in(),
            ));
            out.push(running(
                n("signed-out-notice"),
                page(base, SMALL, ScScene::SignedOutNotice),
                SourceAccountState::SignedOut,
            ));
        }
        // The setup flow's Sources step.
        out.push(shot(
            format!("setup-sources-soundcloud-off-{t}-960x600"),
            setup(base, ScScene::Off),
        ));
        out.push(running(
            format!("setup-sources-soundcloud-sign-in-{t}-960x600"),
            setup(base, ScScene::Account),
            SourceAccountState::SignedOut,
        ));
    }
    out.push(shot(
        "setup-sources-soundcloud-confirming-dark-960x600".to_owned(),
        setup(BaseMode::Dark, ScScene::Confirming),
    ));
    out.push(running(
        "setup-sources-soundcloud-signed-in-dark-960x600".to_owned(),
        setup(BaseMode::Dark, ScScene::Account),
        signed_in(),
    ));
    out.push(shot(
        "setup-sources-soundcloud-restart-dark-960x600".to_owned(),
        setup(BaseMode::Dark, ScScene::Restart),
    ));
}
