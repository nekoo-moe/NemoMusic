//! Shots of the first-run setup flow (startup-flow task 3): every step in dark and light at the
//! minimum window, plus the notice ticked, signed in and a failed sign-in.

use nemo_ui::gui::headless::Scenario;
use nemo_ui::scheme::BaseMode;
use nemo_ui::settings::UiSettings;
use nemo_ui::setup::SetupStep;

use super::{Shot, shot};

/// The minimum window (the narrow stepper) and a wide window (the brand panel).
const SIZE: (u32, u32) = (960, 600);
const WIDE: (u32, u32) = (1366, 697);
/// The narrowest window that still has the brand panel, at the minimum height.
const EDGE: (u32, u32) = (1100, 600);

fn scenario(base: BaseMode, accepted: bool, step: SetupStep) -> Scenario {
    sized(SIZE, base, accepted, step)
}

fn sized(size: (u32, u32), base: BaseMode, accepted: bool, step: SetupStep) -> Scenario {
    Scenario {
        size,
        settings: UiSettings {
            base,
            tos_acknowledged: accepted,
            ..UiSettings::default()
        },
        setup: Some(step),
        // A first run: nothing has played, so no song tints the window.
        stopped: Some(false),
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
    wide_shots(out);
    motion_shots(out);
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

/// The brand panel: every step in dark at 1366 x 697, the Look step in light, and the two
/// steps with the most controls at the narrowest wide window.
fn wide_shots(out: &mut Vec<Shot>) {
    let name = |step: SetupStep, tag: &str, size: (u32, u32)| {
        format!(
            "setup-{}-{tag}-{}x{}",
            step.title().to_lowercase().replace(' ', "-"),
            size.0,
            size.1
        )
    };
    for step in SetupStep::ALL {
        out.push(shot(
            name(step, "dark", WIDE),
            sized(WIDE, BaseMode::Dark, false, step),
        ));
    }
    out.push(shot(
        name(SetupStep::Look, "light", WIDE),
        sized(WIDE, BaseMode::Light, false, SetupStep::Look),
    ));
    for step in [SetupStep::Look, SetupStep::Sources] {
        out.push(shot(
            name(step, "dark", EDGE),
            sized(EDGE, BaseMode::Dark, step == SetupStep::Sources, step),
        ));
    }
}

/// Key frames of the step change: the Look step 0, 80, 160 and 320 ms in (the block slides, the
/// rows rise 40 ms apart, the stepper marker morphs), and the Done step 60, 140 and 220 ms in
/// (the mark settles). Wide, so the brand panel and its stepper show.
fn motion_shots(out: &mut Vec<Shot>) {
    let frames = [
        (SetupStep::Look, "look", [0_u32, 80, 160, 320].as_slice()),
        (SetupStep::Done, "done", [60, 140, 220].as_slice()),
    ];
    for (step, tag, times) in frames {
        for ms in times {
            out.push(shot(
                format!("setup-motion-{tag}-t{ms}-dark-1366x697"),
                Scenario {
                    full_motion: true,
                    setup_ms: Some(*ms),
                    ..sized(WIDE, BaseMode::Dark, false, step)
                },
            ));
        }
    }
}
