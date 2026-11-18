//! The sources step: a YouTube Music card (sign in, or use it without an account), a SoundCloud
//! card (public tracks, no sign-in) and a local music card. Each card changes only itself; the
//! bottom bar moves on.

use iced::widget::{Space, column, container, row, toggler};
use iced::{Alignment, Element, Length};

use super::super::kit::{paragraph, text_button, tile, tonal_button, vspace};
use super::super::settings::switch_style;
use super::steps::heading;
use crate::copy::{self, setup as text};
use crate::gui::app::{AccountMsg, App, Message};
use crate::gui::assets::Icon;
use crate::gui::style::{col, fit, gap, icon, label, spinner};
use crate::setup::SetupEvent;
use crate::state::Event;
use crate::theme::{color, type_scale};
use nemo_core::account::AccountState;

pub(super) fn view(app: &App, width: f32) -> Element<'_, Message> {
    let inner = width - 2.0 * gap::LG;
    column![
        heading(text::SOURCES_TITLE, Some(text::SOURCES_HELP), width),
        vspace(gap::LG),
        card(youtube(app, inner), width),
        vspace(gap::MD),
        card(soundcloud(inner), width),
        vspace(gap::MD),
        card(local(app, inner), width),
    ]
    .width(Length::Fixed(width))
    .into()
}

fn card(content: Element<'_, Message>, width: f32) -> Element<'_, Message> {
    tile(
        container(content)
            .padding(gap::LG)
            .width(Length::Fixed(width)),
        color::surface_container(),
    )
}

fn title(text: &str) -> Element<'_, Message> {
    label(text, type_scale::TITLE_SM, true, col(color::on_surface())).into()
}

fn dim(text: &str, width: f32) -> Element<'_, Message> {
    paragraph(
        text,
        type_scale::BODY_SM,
        col(color::on_surface_variant()),
        width,
    )
}

/// The YouTube Music card. Everything it shows comes from the account service as plain
/// sentences: no token, cookie or id.
fn youtube(app: &App, inner: f32) -> Element<'_, Message> {
    let mut c = column![title("YouTube Music"), dim(text::YT_BODY, inner)]
        .spacing(gap::SM)
        .width(Length::Fixed(inner));
    let state = app.backend.account.state();
    if let AccountState::SignedIn(info) = state {
        return c
            .push(status_line(
                Icon::CheckCircle,
                format!("Signed in as {}", info.name),
            ))
            .into();
    }
    if !app.ui.settings.tos_acknowledged {
        return c
            .push(dim(text::YT_NEEDS_NOTICE, inner))
            .push(text_button(
                "Back to the notice",
                Message::Ui(Event::Setup(SetupEvent::Back)),
            ))
            .into();
    }
    if app.account_busy() {
        return c
            .push(
                row![
                    spinner(24.0, col(app.accent().primary), app.spin),
                    Space::new().width(gap::MD),
                    paragraph(
                        copy::ACCOUNT_WAITING,
                        type_scale::BODY_MD,
                        col(color::on_surface()),
                        inner - 24.0 - gap::MD,
                    ),
                ]
                .align_y(Alignment::Center),
            )
            .into();
    }
    if let Some(error) = &app.account.error {
        c = c
            .push(status_line(Icon::Info, error.clone()))
            .push(dim(text::YT_FAILED_WAY, inner));
    }
    if app.ui.setup.without_account() {
        c = c.push(status_line(
            Icon::Check,
            text::YT_WITHOUT_ACCOUNT.to_owned(),
        ));
    }
    let mut actions = row![tonal_button(
        "Sign in with Google",
        Some(Icon::Login),
        Some(Message::Account(AccountMsg::SignIn)),
    )]
    .spacing(gap::SM)
    .align_y(Alignment::Center);
    if !app.ui.setup.without_account() {
        actions = actions.push(text_button(
            "Use without an account",
            Message::Ui(Event::Setup(SetupEvent::UseWithoutAccount)),
        ));
    }
    c = c.push(actions);
    if app.account.error.is_some() {
        // Its own row: the three buttons do not fit side by side at the minimum window.
        c = c.push(text_button(
            "More ways to sign in",
            Message::Account(AccountMsg::Open),
        ));
    }
    c.into()
}

/// The SoundCloud card. There is no SoundCloud sign-in and, in this version, no SoundCloud
/// catalog behind the switch, so the switch is shown off and cannot be turned on.
fn soundcloud<'a>(inner: f32) -> Element<'a, Message> {
    let switch = row![
        label(
            "Off",
            type_scale::BODY_MD,
            false,
            col(color::on_surface_variant())
        ),
        Space::new().width(gap::MD),
        toggler(false).size(24.0).style(switch_style(false)),
    ]
    .align_y(Alignment::Center);
    column![
        row![
            title("SoundCloud"),
            Space::new().width(Length::Fill),
            switch
        ]
        .align_y(Alignment::Center),
        label(
            text::SC_BODY,
            type_scale::BODY_MD,
            false,
            col(color::on_surface())
        ),
        dim(text::SC_NOT_YET, inner),
    ]
    .spacing(gap::SM)
    .width(Length::Fixed(inner))
    .into()
}

/// The local music card: the folders already listed, `Add a folder` when the app has a picker,
/// and `Skip`.
fn local(app: &App, inner: f32) -> Element<'_, Message> {
    let mut c = column![title("Local music"), dim(text::LOCAL_BODY, inner)]
        .spacing(gap::SM)
        .width(Length::Fixed(inner));
    for folder in app.music_folders.iter().take(3) {
        c = c.push(label(
            fit(folder, inner, type_scale::BODY_MD.size),
            type_scale::BODY_MD,
            false,
            col(color::on_surface()),
        ));
    }
    if !app.has_add_folder() {
        c = c.push(dim(text::LOCAL_NO_PICKER, inner));
    }
    let mut actions = row![].spacing(gap::SM).align_y(Alignment::Center);
    if app.has_add_folder() {
        actions = actions.push(tonal_button("Add a folder", None, Some(Message::AddFolder)));
    }
    if app.ui.setup.folders_skipped() {
        c = c.push(status_line(Icon::Check, text::LOCAL_SKIPPED.to_owned()));
    } else {
        actions = actions.push(text_button(
            "Skip",
            Message::Ui(Event::Setup(SetupEvent::SkipFolders)),
        ));
    }
    c.push(actions).into()
}

/// An icon and one sentence.
fn status_line<'a>(glyph: Icon, text: String) -> Element<'a, Message> {
    row![
        icon(glyph, 20.0, col(color::on_surface_variant())),
        Space::new().width(gap::SM),
        label(text, type_scale::BODY_MD, true, col(color::on_surface())),
    ]
    .align_y(Alignment::Center)
    .into()
}
