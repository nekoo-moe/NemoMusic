//! The sources step: a YouTube Music card (sign in, or use it without an account), a SoundCloud
//! card (turn it on, then sign in) and a local music card. Each card changes only itself; the
//! bottom bar moves on.

use iced::widget::{Space, column, container, row};
use iced::{Alignment, Element, Length};

use super::super::kit::{paragraph, text_button, tile, tonal_button, vspace};
use super::super::opt_in::{OptInActions, OptInPhase, OptInRow, opt_in_row};
use super::reveal::rise;
use super::steps::{has_two_columns, heading, two_columns};
use crate::copy::{self, setup as text};
use crate::gui::app::{AccountMsg, App, Message, ScMsg};
use crate::gui::assets::Icon;
use crate::gui::style::{col, fit, gap, icon, label, spinner};
use crate::setup::SetupEvent;
use crate::state::Event;
use crate::theme::{color, type_scale};
use nemo_core::account::AccountState;
use nemo_core::source_account::SourceAccountState;

pub(super) fn view(app: &App, width: f32) -> Element<'_, Message> {
    let two = has_two_columns(width);
    let col_w = if two {
        ((width - gap::LG) / 2.0).floor()
    } else {
        width
    };
    let inner = col_w - 2.0 * gap::MD;
    let head = heading(text::SOURCES_TITLE, Some(text::SOURCES_HELP), width);
    let yt = card(youtube(app, inner), col_w);
    let sc = card(soundcloud(app, inner), col_w);
    let local = card(local(app, inner), col_w);
    let (yt, sc, local) = (rise(app, 0, yt), rise(app, 1, sc), rise(app, 2, local));
    let body: Element<'_, Message> = if two {
        // YouTube Music is the tall card; the other two share the second column.
        two_columns(vec![yt], vec![sc, vspace(gap::SM), local], width)
    } else {
        column![yt, vspace(gap::SM), sc, vspace(gap::SM), local].into()
    };
    column![head, vspace(gap::MD), body]
        .width(Length::Fixed(width))
        .into()
}

fn card(content: Element<'_, Message>, width: f32) -> Element<'_, Message> {
    tile(
        container(content)
            .padding(gap::MD)
            .width(Length::Fixed(width)),
        color::surface_container_high(),
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
                inner,
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
            .push(status_line(Icon::Info, error.clone(), inner))
            .push(dim(text::YT_FAILED_WAY, inner));
    }
    if app.ui.setup.without_account() {
        c = c.push(status_line(
            Icon::Check,
            text::YT_WITHOUT_ACCOUNT.to_owned(),
            inner,
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

/// The SoundCloud card: what it is, then the switch's own confirmation, or sign-in once it runs.
/// The panel and the sign-in are the ones of Settings; this card only starts them.
fn soundcloud(app: &App, inner: f32) -> Element<'_, Message> {
    let Some(info) = app.sc_info() else {
        return column![
            title(text::SC_TITLE),
            dim(copy::soundcloud::NOT_IN_THIS_BUILD, inner)
        ]
        .spacing(gap::SM)
        .into();
    };
    if app.sc.confirming {
        return opt_in_row(
            OptInRow {
                info,
                phase: OptInPhase::Confirming,
                status: None,
                stays: None,
            },
            OptInActions {
                ask: Message::Sc(ScMsg::Ask),
                turn_on: Message::Sc(ScMsg::TurnOn),
                cancel: Message::Sc(ScMsg::Cancel),
                turn_off: Message::Sc(ScMsg::TurnOff),
                delete_now: None,
            },
            inner + 2.0 * gap::MD,
        );
    }
    let mut c = column![
        title(text::SC_TITLE),
        dim(copy::soundcloud::SETUP_BODY, inner)
    ]
    .spacing(gap::SM)
    .width(Length::Fixed(inner));
    if !app.sc_open() {
        return c
            .push(tonal_button(
                copy::soundcloud::SETUP_TURN_ON,
                None,
                Some(Message::Sc(ScMsg::Ask)),
            ))
            .into();
    }
    match app.sc_state() {
        _ if !app.sc_running() => c.push(status_line(
            Icon::Check,
            copy::soundcloud::SETUP_AFTER_RESTART.to_owned(),
            inner,
        )),
        Some(SourceAccountState::SignedIn(info)) => c.push(status_line(
            Icon::CheckCircle,
            format!("Signed in as {}", info.name),
            inner,
        )),
        Some(_) => {
            if let Some(error) = &app.sc.error {
                c = c.push(status_line(Icon::Info, error.clone(), inner));
            }
            c.push(tonal_button(
                copy::soundcloud::SIGN_IN,
                Some(Icon::Login),
                (!app.sc.busy).then_some(Message::Sc(ScMsg::SignIn)),
            ))
        }
        None => c.push(dim(copy::soundcloud::NO_STORE, inner)),
    }
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
        c = c.push(status_line(
            Icon::Check,
            text::LOCAL_SKIPPED.to_owned(),
            inner,
        ));
    } else {
        actions = actions.push(text_button(
            "Skip",
            Message::Ui(Event::Setup(SetupEvent::SkipFolders)),
        ));
    }
    c.push(actions).into()
}

/// An icon and one sentence that wraps inside `width`.
fn status_line<'a>(glyph: Icon, text: String, width: f32) -> Element<'a, Message> {
    row![
        icon(glyph, 20.0, col(color::on_surface_variant())),
        Space::new().width(gap::SM),
        label(text, type_scale::BODY_MD, true, col(color::on_surface()))
            .wrapping(iced::widget::text::Wrapping::Word)
            .width(Length::Fixed(width - 20.0 - gap::SM)),
    ]
    .align_y(Alignment::Start)
    .into()
}
