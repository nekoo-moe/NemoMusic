//! The System tab: version, OS, build profile, the config keys that are set, the hosts contacted
//! this run, uptime, the audio output and the frame times.

use iced::widget::column;
use iced::{Element, Length};

use super::parts::{Ink, UNKNOWN, card, empty, title};
use crate::gui::app::{App, Message};
use crate::gui::style::gap;

/// The body of the tab.
pub(super) fn view(app: &App) -> Element<'_, Message> {
    let width = super::inner_width(app);
    let sys = &app.dev.snap.system;
    let frames = app
        .frame_stats
        .readout()
        .map_or_else(|| "press F3 to measure".to_owned(), |r| r.text());
    let mut list = column![title("This run")]
        .spacing(gap::SM)
        .width(Length::Fill);
    list = list.push(card(
        "App",
        vec![
            ("Version".to_owned(), sys.version.clone(), Ink::Plain),
            ("System".to_owned(), sys.os.clone(), Ink::Plain),
            ("Build".to_owned(), sys.profile.clone(), Ink::Plain),
            (
                "Uptime".to_owned(),
                format!("{} s", sys.uptime_s),
                Ink::Plain,
            ),
            (
                "Audio output".to_owned(),
                sys.audio.clone().unwrap_or_else(|| UNKNOWN.to_owned()),
                Ink::Plain,
            ),
            ("Frame times".to_owned(), frames, Ink::Plain),
        ],
        width,
    ));
    list = list.push(title("Settings that are set"));
    if sys.config.is_empty() {
        list = list.push(empty("Every setting has its default."));
    } else {
        let lines = sys
            .config
            .iter()
            .map(|c| {
                (
                    c.key.clone(),
                    c.value
                        .clone()
                        .unwrap_or_else(|| "(set, value hidden)".to_owned()),
                    Ink::Plain,
                )
            })
            .collect();
        list = list.push(card("Config", lines, width));
    }
    list = list.push(title("Hosts contacted"));
    if sys.egress.is_empty() {
        list = list.push(empty("No requests yet."));
    } else {
        let lines = sys
            .egress
            .iter()
            .map(|e| (e.host.clone(), format!("{} requests", e.count), Ink::Plain))
            .collect();
        list = list.push(card("Egress", lines, width));
    }
    list.into()
}
