//! Tests for the opt-in row. The headless ones build a renderer: run them with
//! `ICED_TEST_BACKEND=tiny-skia` and `--ignored`.

use iced::{Pixels, Rectangle, Size};
use iced_test::Simulator;
use iced_test::selector::Candidate;

use super::*;
use crate::gui::app::SettingsSection;
use crate::gui::assets;

const INFO: OptInInfo = OptInInfo {
    id: "fake",
    title: "Fake scrobbling",
    sends: "The songs you play, with the time you played them.",
    hosts: &["ws.example.org", "api.example.org"],
    risk: "Your plays show on your public profile.",
    version: 1,
    default_on: false,
};

fn actions(delete: bool) -> OptInActions {
    OptInActions {
        ask: Message::SettingsSection(SettingsSection::Playback),
        turn_on: Message::SettingsSection(SettingsSection::AudioOutput),
        cancel: Message::SettingsSection(SettingsSection::Lyrics),
        turn_off: Message::SettingsSection(SettingsSection::Library),
        delete_now: delete.then_some(Message::SettingsSection(SettingsSection::Privacy)),
    }
}

fn state(phase: OptInPhase) -> OptInRow<'static> {
    OptInRow {
        info: &INFO,
        phase,
        status: Some("Connected"),
        stays: Some("Your scrobble queue stays on this computer."),
    }
}

#[test]
fn the_panel_buttons_come_only_while_confirming() {
    use OptInControl::{Cancel, DeleteNow, Switch, TurnOn};
    let all = actions(true);
    assert_eq!(controls(&state(OptInPhase::Off), &all), [Switch, DeleteNow]);
    assert_eq!(controls(&state(OptInPhase::Off), &actions(false)), [Switch]);
    assert_eq!(
        controls(&state(OptInPhase::Confirming), &all),
        [Switch, TurnOn, Cancel]
    );
    assert_eq!(controls(&state(OptInPhase::On), &all), [Switch]);
}

#[test]
fn the_hosts_line_names_every_host_or_says_none() {
    assert_eq!(hosts_text(&INFO), "ws.example.org, api.example.org");
    let local = OptInInfo { hosts: &[], ..INFO };
    assert_eq!(hosts_text(&local), copy::OPT_IN_NO_HOSTS);
}

fn simulator(phase: OptInPhase, delete: bool, width: f32) -> Simulator<'static, Message> {
    let settings = iced::Settings {
        fonts: vec![
            assets::FONT_REGULAR.into(),
            assets::FONT_MEDIUM.into(),
            assets::FONT_SEMIBOLD.into(),
        ],
        default_font: assets::font(false),
        default_text_size: Pixels(14.0),
        ..iced::Settings::default()
    };
    let view = opt_in_row(state(phase), actions(delete), width);
    Simulator::with_size(settings, Size::new(width, 900.0), view)
}

/// Every focusable control in traversal (Tab) order, and every text with its bounds.
fn scan(sim: &mut Simulator<'_, Message>) -> (Vec<Rectangle>, Vec<(String, Rectangle)>) {
    let mut focusable = Vec::new();
    let mut texts = Vec::new();
    // The selector never matches, so it visits the whole tree; the error is expected.
    let _ = sim.find(|candidate: Candidate<'_>| -> Option<()> {
        match candidate {
            Candidate::Focusable { bounds, .. } => focusable.push(bounds),
            Candidate::Text {
                content, bounds, ..
            } => texts.push((content.to_owned(), bounds)),
            _ => {}
        }
        None
    });
    (focusable, texts)
}

/// Name each focusable control by the text inside it; the switch holds none.
fn named(focusable: &[Rectangle], texts: &[(String, Rectangle)]) -> Vec<OptInControl> {
    focusable
        .iter()
        .map(|b| {
            let inside = texts
                .iter()
                .find(|(_, t)| b.contains(t.center()))
                .map(|(s, _)| s.as_str());
            match inside {
                Some(copy::OPT_IN_TURN_ON) => OptInControl::TurnOn,
                Some(copy::OPT_IN_CANCEL) => OptInControl::Cancel,
                Some(copy::OPT_IN_DELETE_NOW) => OptInControl::DeleteNow,
                _ => OptInControl::Switch,
            }
        })
        .collect()
}

#[test]
#[ignore = "builds a headless renderer: run with ICED_TEST_BACKEND=tiny-skia"]
fn tab_reaches_every_control_in_drawing_order() {
    for (phase, delete) in [
        (OptInPhase::Off, true),
        (OptInPhase::Confirming, false),
        (OptInPhase::On, false),
    ] {
        let mut sim = simulator(phase, delete, 640.0);
        let (focusable, texts) = scan(&mut sim);
        assert_eq!(
            named(&focusable, &texts),
            controls(&state(phase), &actions(delete)),
            "{phase:?}"
        );
        for b in &focusable {
            assert!(b.height >= 32.0, "{phase:?}: target {b:?} under 32 px");
        }
        // Neither panel button, nor any other control, starts focused.
        assert!(sim.find(iced_test::selector::is_focused()).is_err());
    }
}

#[test]
#[ignore = "builds a headless renderer: run with ICED_TEST_BACKEND=tiny-skia"]
fn every_phase_shows_what_is_sent_where_and_the_risk() {
    for phase in [OptInPhase::Off, OptInPhase::Confirming, OptInPhase::On] {
        let mut sim = simulator(phase, true, 640.0);
        let (_, texts) = scan(&mut sim);
        let shown = |s: &str| texts.iter().any(|(t, _)| t == s);
        for s in [
            copy::OPT_IN_SENDS,
            copy::OPT_IN_TO,
            copy::OPT_IN_RISK,
            INFO.sends,
            INFO.risk,
            "ws.example.org, api.example.org",
        ] {
            assert!(shown(s), "{phase:?}: {s}");
        }
        assert_eq!(shown(copy::OPT_IN_TURN_ON), phase == OptInPhase::Confirming);
        assert_eq!(shown("Connected"), phase == OptInPhase::On);
        assert_eq!(
            shown("Your scrobble queue stays on this computer."),
            phase == OptInPhase::Off
        );
        assert!(shown(if phase == OptInPhase::On { "On" } else { "Off" }));
    }
}

#[test]
#[ignore = "builds a headless renderer: run with ICED_TEST_BACKEND=tiny-skia"]
fn each_control_sends_the_callers_message() {
    let clicked = |phase, text: Option<&str>| -> Vec<Message> {
        let mut sim = simulator(phase, true, 640.0);
        if let Some(t) = text {
            sim.click(t).expect("button");
        } else {
            let (focusable, _) = scan(&mut sim);
            let switch = focusable.first().expect("switch").center();
            // `click(Point)` would pick the outermost container under the point; press the
            // point itself instead.
            sim.point_at(switch);
            let _ = sim.simulate(iced_test::simulator::click());
        }
        sim.into_messages().collect()
    };
    let section = |m: &[Message]| match m {
        [Message::SettingsSection(s)] => Some(*s),
        _ => None,
    };
    let confirming = OptInPhase::Confirming;
    assert_eq!(
        section(&clicked(OptInPhase::Off, None)),
        Some(SettingsSection::Playback)
    );
    assert_eq!(
        section(&clicked(OptInPhase::On, None)),
        Some(SettingsSection::Library)
    );
    assert_eq!(
        section(&clicked(confirming, Some(copy::OPT_IN_TURN_ON))),
        Some(SettingsSection::AudioOutput)
    );
    assert_eq!(
        section(&clicked(confirming, Some(copy::OPT_IN_CANCEL))),
        Some(SettingsSection::Lyrics)
    );
    assert_eq!(
        section(&clicked(OptInPhase::Off, Some(copy::OPT_IN_DELETE_NOW))),
        Some(SettingsSection::Privacy)
    );
}

#[test]
#[ignore = "builds a headless renderer: run with ICED_TEST_BACKEND=tiny-skia"]
fn texts_wrap_inside_a_narrow_row() {
    let width = 320.0;
    for phase in [OptInPhase::Off, OptInPhase::Confirming, OptInPhase::On] {
        let mut sim = simulator(phase, true, width);
        let (_, texts) = scan(&mut sim);
        for (text, b) in &texts {
            assert!(
                b.x >= 0.0 && b.x + b.width <= width + 0.5,
                "{phase:?}: {text}"
            );
        }
    }
}
