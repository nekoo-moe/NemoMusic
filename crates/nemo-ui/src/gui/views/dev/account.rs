//! The Account tab: sign-in diagnostics per site. Facts only: signed in or not, a token present
//! yes or no, its size class, the cookie names and the last sign-in stage. Never a value.

use iced::widget::column;
use iced::{Element, Length};

use super::parts::{Ink, card, empty, title};
use crate::gui::app::dev::report::{len_class, when, yes_no};
use crate::gui::app::{App, Message};
use crate::gui::style::gap;

/// The body of the tab.
pub(super) fn view(app: &App) -> Element<'_, Message> {
    let width = super::inner_width(app);
    let snap = &app.dev.snap;
    let mut list = column![title("Sign-in")]
        .spacing(gap::SM)
        .width(Length::Fill);
    if snap.accounts.is_empty() {
        list = list.push(empty("No account facts in this build."));
    }
    for a in &snap.accounts {
        let cookies = if a.cookie_names.is_empty() {
            "-".to_owned()
        } else {
            a.cookie_names.join(", ")
        };
        let outcome = a.last_outcome.as_deref().unwrap_or("-");
        let bad = matches!(
            outcome,
            "rejected" | "failed" | "network" | "unavailable" | "storage" | "blocked"
        );
        list = list.push(card(
            &a.site,
            vec![
                (
                    "Signed in".to_owned(),
                    yes_no(a.signed_in).to_owned(),
                    Ink::Plain,
                ),
                (
                    "Token stored".to_owned(),
                    yes_no(a.token_present).to_owned(),
                    Ink::Plain,
                ),
                (
                    "Token size".to_owned(),
                    len_class(a.token_len).to_owned(),
                    Ink::Plain,
                ),
                ("Cookie names".to_owned(), cookies, Ink::Plain),
                (
                    "Last stage".to_owned(),
                    a.last_stage.clone().unwrap_or_else(|| "-".to_owned()),
                    Ink::Plain,
                ),
                (
                    "Last outcome".to_owned(),
                    outcome.to_owned(),
                    if bad { Ink::Bad } else { Ink::Plain },
                ),
                ("Last stage at".to_owned(), when(a.last_at_ms), Ink::Plain),
            ],
            width,
        ));
    }
    list.into()
}
