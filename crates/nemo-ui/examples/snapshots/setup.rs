//! Shots of the first-run setup flow (startup-flow task 3): every step in dark and light at the
//! minimum window, plus the notice ticked, signed in and a failed sign-in.

use nemo_ui::gui::headless::Scenario;
use nemo_ui::scheme::BaseMode;
use nemo_ui::settings::UiSettings;
use nemo_ui::setup::SetupStep;

use super::{Shot, shot};

const SIZE: (u32, u32) = (960, 600);

fn scenario(base: BaseMode, accepted: bool, step: SetupStep) -> Scenario {
    Scenario {
        size: SIZE,
        settings: UiSettings {
            base,
            tos_acknowledged: accepted,
            ..UiSettings::default()
        },
        setup: Some(step),
        ..Scenario::default()
    }
}

/// Every setup shot.
pub(super) fn shots(out: &mut Vec<Shot>) {
    for (tag, base) in [("dark", BaseMode::Dark), ("light", BaseMode::Light)] {
        for step in SetupStep::ALL {
            let name = format!(
                "setup-{}-{tag}-960x600",
                step.title().to_lowercase().replace(' ', "-")
            );
            // The notice is open until the user answers it; the later steps follow an answer.
            out.push(shot(name, scenario(base, false, step)));
        }
    }
    out.push(shot(
        "setup-privacy-and-risk-ticked-dark-960x600".to_owned(),
        Scenario {
            setup_ticked: true,
            ..scenario(BaseMode::Dark, false, SetupStep::Notice)
        },
    ));
    out.push(shot(
        "setup-sources-accepted-dark-960x600".to_owned(),
        scenario(BaseMode::Dark, true, SetupStep::Sources),
    ));
    let mut signed_in = shot(
        "setup-sources-signed-in-dark-960x600".to_owned(),
        scenario(BaseMode::Dark, true, SetupStep::Sources),
    );
    signed_in.signed_in = true;
    out.push(signed_in);
    out.push(shot(
        "setup-sources-error-dark-960x600".to_owned(),
        Scenario {
            account_error: Some(
                "Google did not accept that sign-in window. Try again, or paste a cookie."
                    .to_owned(),
            ),
            ..scenario(BaseMode::Dark, true, SetupStep::Sources)
        },
    ));
}
