//! The SoundCloud section (soundcloud-account spec): the account card (while SoundCloud is on),
//! then the opt-in row.
//!
//! The row is the shared `OptInRow` (what is sent, to which hosts, the risk, "Turn on" in a panel
//! that has no pre-selected button). The card shows only while SoundCloud is on and running: sign
//! in with SoundCloud's own page or with a pasted token (masked, never echoed), the signed-in
//! name and avatar, and Sign out with its confirmation. Plain words everywhere: the access is
//! unofficial, the token stays on this computer, the audio is lossy.

use iced::font::Weight;
use iced::widget::{Space, column, container, row};
use iced::{Alignment, Element, Length, Padding};
use nemo_core::source_account::{SourceAccountInfo, SourceAccountState};
use nemo_core::track::Artwork;

use crate::copy::soundcloud as text;
use crate::gui::app::{App, ArtShape, Message, ScMsg};
use crate::gui::assets::Icon;
use crate::gui::style::{col, gap, label, label_w, spinner};
use crate::gui::views::account::{SecretField, notice_box, secret_field};
use crate::gui::views::kit::{art, filled_button, paragraph, text_button, tile, tonal_button};
use crate::gui::views::listening_area;
use crate::gui::views::opt_in::{Extras, OptInActions, OptInPhase, OptInRow, opt_in_row_with};
use crate::gui::views::settings::appearance::restart;
use crate::theme::{color, type_scale};

/// The whole section: the opt-in row and, when it applies, the account card.
pub(super) fn section(app: &App, width: f32) -> Element<'_, Message> {
    let mut page = column![].spacing(gap::LG).width(Length::Fixed(width));
    // The listening area is chosen here and in the account sheet, nowhere else.
    page = page.push(group(vec![area_row(app, width)], width));
    // The account comes first while SoundCloud is on: the switch row below it lists what is
    // sent, to which hosts and the risk, which is long.
    if app.sc_open() {
        page = page.push(group(vec![account_rows(app, width)], width));
    }
    page = page.push(group(vec![switch_row(app, width)], width));
    page.into()
}

/// The listening-area card: the same control as the account sheet.
fn area_row(app: &App, width: f32) -> Element<'_, Message> {
    container(listening_area::control(app, width - 2.0 * gap::LG))
        .padding(Padding::from([gap::MD, gap::LG]))
        .width(Length::Fixed(width))
        .into()
}

fn group(rows: Vec<Element<'_, Message>>, width: f32) -> Element<'_, Message> {
    tile(
        column(rows)
            .width(Length::Fixed(width))
            .padding(Padding::from([gap::SM, 0.0])),
        color::surface_container(),
    )
}

/// The phase of the switch, from the app state.
fn phase(app: &App) -> OptInPhase {
    if app.sc.confirming {
        OptInPhase::Confirming
    } else if app.sc_open() {
        OptInPhase::On
    } else {
        OptInPhase::Off
    }
}

/// One status line under an open switch.
fn status(app: &App) -> &'static str {
    if !app.sc_running() {
        return if app.sc_restart_offered() {
            text::NEEDS_RESTART
        } else {
            text::STARTS_AFTER_RESTART
        };
    }
    match app.sc_state() {
        Some(SourceAccountState::SignedIn(_)) => text::STATUS_SIGNED_IN,
        _ => text::STATUS_SIGNED_OUT,
    }
}

/// The messages of the opt-in row's controls. Turning off signs out, so there is no separate
/// "Delete now".
fn actions() -> OptInActions {
    OptInActions {
        ask: Message::Sc(ScMsg::Ask),
        turn_on: Message::Sc(ScMsg::TurnOn),
        cancel: Message::Sc(ScMsg::Cancel),
        turn_off: Message::Sc(ScMsg::TurnOff),
        delete_now: None,
    }
}

fn switch_row(app: &App, width: f32) -> Element<'_, Message> {
    let Some(info) = app.sc_info() else {
        return container(paragraph(
            text::NOT_IN_THIS_BUILD,
            type_scale::BODY_MD,
            col(color::on_surface_variant()),
            width - 2.0 * gap::LG,
        ))
        .padding(Padding::from([gap::MD, gap::LG]))
        .into();
    };
    let phase = phase(app);
    let row = OptInRow {
        info,
        phase,
        status: Some(status(app)),
        // Off: what turning it off did. A failed removal is told here, where the switch is.
        stays: Some(app.sc.error.as_deref().unwrap_or(text::STAYS)),
    };
    // The same `Restart now` pill as the Window glass card sits under the switch, and its place
    // only exists while it shows or fades.
    let pill = (phase == OptInPhase::On && app.surfaces.sc_restart.visible()).then(|| {
        (
            restart::slot(app, &app.surfaces.sc_restart, restart::PILL_W),
            restart::PILL_W,
        )
    });
    let extras = Extras {
        under_switch: pill,
        under_header: app.sc.confirm_off.then(|| off_confirm(app, width)),
    };
    opt_in_row_with(row, actions(), width, extras)
}

/// The question that comes with turning the switch off while signed in. It sits right under
/// the switch, inside the row, so it is seen where the switch was pressed.
fn off_confirm(app: &App, width: f32) -> Element<'_, Message> {
    let inner = width - 2.0 * gap::LG;
    column![
        paragraph(
            text::OFF_CONFIRM,
            type_scale::BODY_MD,
            col(color::on_surface()),
            inner,
        ),
        row![
            filled_button(
                text::OFF_CONFIRM_BUTTON,
                Icon::Logout,
                app.accent(),
                Message::Sc(ScMsg::ConfirmOff),
            ),
            Space::new().width(gap::SM),
            text_button("Cancel", Message::Sc(ScMsg::CancelOff)),
        ]
        .align_y(Alignment::Center),
    ]
    .spacing(gap::MD)
    .into()
}

/// The account card, or the reason there is none.
fn account_rows(app: &App, width: f32) -> Element<'_, Message> {
    let inner = width - 2.0 * gap::LG;
    let note = |t: &'static str| -> Element<'_, Message> {
        paragraph(
            t,
            type_scale::BODY_MD,
            col(color::on_surface_variant()),
            inner,
        )
    };
    let body: Element<'_, Message> = if app.sc_running() {
        match app.backend.soundcloud_account.as_ref().map(|a| a.state()) {
            None => note(text::NO_STORE),
            Some(SourceAccountState::SignedIn(info)) => signed_in(app, &info, inner),
            Some(state) => signed_out(app, &state, inner),
        }
    } else {
        let c = column![note(text::STARTS_AFTER_RESTART)].spacing(gap::MD);
        c.into()
    };
    container(body)
        .padding(Padding::from([gap::MD, gap::LG]))
        .width(Length::Fixed(width))
        .into()
}

fn signed_out<'a>(app: &'a App, state: &SourceAccountState, width: f32) -> Element<'a, Message> {
    let sc = &app.sc;
    let busy = sc.busy || matches!(state, SourceAccountState::SigningIn);
    let dim = col(color::on_surface_variant());
    let mut c = column![label(
        text::TITLE,
        type_scale::TITLE_SM,
        true,
        col(color::on_surface())
    )]
    .spacing(gap::MD)
    .width(Length::Fixed(width));
    if let Some(done) = &sc.notice {
        c = c.push(notice_box(
            Icon::CheckCircle,
            col(color::success()),
            col(color::success().with_alpha(0.12)),
            done.clone(),
            width,
        ));
    }
    if matches!(state, SourceAccountState::Expired) {
        c = c.push(notice_box(
            Icon::Info,
            col(color::warning()),
            col(color::warning().with_alpha(0.10)),
            text::EXPIRED.to_owned(),
            width,
        ));
    }
    c = c
        .push(paragraph(
            text::SIGNED_OUT_BODY,
            type_scale::BODY_MD,
            col(color::on_surface()),
            width,
        ))
        .push(paragraph(text::LOCAL, type_scale::BODY_SM, dim, width))
        .push(paragraph(text::HONEST, type_scale::BODY_SM, dim, width));
    if let Some(e) = &sc.error {
        c = c.push(notice_box(
            Icon::Info,
            col(color::error()),
            col(color::error().with_alpha(0.12)),
            e.clone(),
            width,
        ));
    }
    if busy {
        let waiting = if sc.paste && !sc.window {
            text::CHECKING
        } else {
            text::WAITING
        };
        c = c.push(
            row![
                spinner(24.0, col(app.accent().primary), app.spin),
                Space::new().width(gap::MD),
                paragraph(
                    waiting,
                    type_scale::BODY_MD,
                    col(color::on_surface()),
                    width - 24.0 - gap::MD,
                ),
            ]
            .align_y(Alignment::Center),
        );
    }
    if busy && app.sc_sign_in_hint() {
        c = c.push(hint_box(app, width));
    }
    // A token can be typed while the window waits: it takes over from the window.
    let can_type = !busy || sc.window;
    c.push(sign_in_controls(app, width, !busy, can_type)).into()
}

/// The window found no session for a while: one plain sentence and the way to look again.
fn hint_box(app: &App, width: f32) -> Element<'_, Message> {
    let line = if app.sc.rechecked {
        text::STILL_NOTHING
    } else {
        text::STILL_WAITING
    };
    column![
        paragraph(line, type_scale::BODY_MD, col(color::on_surface()), width),
        tonal_button(
            text::CHECK_AGAIN,
            Some(Icon::Refresh),
            Some(Message::Sc(ScMsg::Recheck)),
        ),
    ]
    .spacing(gap::SM)
    .into()
}

/// The sign-in button and the way to the masked field, or the field and its help.
fn sign_in_controls(app: &App, width: f32, can: bool, can_type: bool) -> Element<'_, Message> {
    let sc = &app.sc;
    if sc.paste {
        let typed = !sc.token.expose().trim().is_empty();
        return column![
            secret_field(
                app,
                SecretField {
                    placeholder: text::PLACEHOLDER,
                    value: &sc.token,
                    width,
                    enabled: can_type,
                },
                |s| Message::Sc(ScMsg::PasteInput(s)),
                Message::Sc(ScMsg::PasteSubmit),
            ),
            paragraph(
                text::PASTE_HELP,
                type_scale::BODY_SM,
                col(color::on_surface_variant()),
                width,
            ),
            row![
                tonal_button(
                    text::CHECK_TOKEN,
                    Some(Icon::Key),
                    (can_type && typed).then_some(Message::Sc(ScMsg::PasteSubmit)),
                ),
                Space::new().width(gap::SM),
                text_button(text::USE_WINDOW, Message::Sc(ScMsg::PasteMode(false))),
            ]
            .align_y(Alignment::Center),
        ]
        .spacing(gap::MD)
        .into();
    }
    row![
        tonal_button(
            text::SIGN_IN,
            Some(Icon::Login),
            can.then_some(Message::Sc(ScMsg::SignIn)),
        ),
        Space::new().width(gap::SM),
        text_button(text::PASTE, Message::Sc(ScMsg::PasteMode(true))),
    ]
    .align_y(Alignment::Center)
    .into()
}

fn signed_in<'a>(app: &'a App, info: &SourceAccountInfo, width: f32) -> Element<'a, Message> {
    let sc = &app.sc;
    let dim = col(color::on_surface_variant());
    let face = art(
        app,
        &Artwork::Remote(info.avatar.clone()),
        64,
        ArtShape::Circle,
    );
    let who = column![
        label_w(
            info.name.clone(),
            type_scale::TITLE_MD,
            Weight::Semibold,
            col(color::on_surface()),
        ),
        label(text::TITLE, type_scale::BODY_MD, false, dim),
    ]
    .spacing(gap::XS);
    let mut c = column![
        row![face, Space::new().width(gap::LG), who].align_y(Alignment::Center),
        paragraph(text::LOCAL, type_scale::BODY_SM, dim, width),
    ]
    .spacing(gap::LG)
    .width(Length::Fixed(width));
    if let Some(e) = &sc.error {
        c = c.push(notice_box(
            Icon::Info,
            col(color::error()),
            col(color::error().with_alpha(0.12)),
            e.clone(),
            width,
        ));
    }
    if sc.confirm_sign_out {
        c = c
            .push(paragraph(
                text::SIGN_OUT_CONFIRM,
                type_scale::BODY_MD,
                col(color::on_surface()),
                width,
            ))
            .push(
                row![
                    filled_button(
                        "Sign out",
                        Icon::Logout,
                        app.accent(),
                        Message::Sc(ScMsg::SignOut)
                    ),
                    Space::new().width(gap::SM),
                    text_button("Cancel", Message::Sc(ScMsg::CancelSignOut)),
                ]
                .align_y(Alignment::Center),
            );
    } else if sc.busy {
        c = c.push(
            row![
                spinner(24.0, col(app.accent().primary), app.spin),
                Space::new().width(gap::MD),
                label(
                    text::SIGNING_OUT,
                    type_scale::BODY_MD,
                    false,
                    col(color::on_surface())
                ),
            ]
            .align_y(Alignment::Center),
        );
    } else {
        c = c.push(tonal_button(
            "Sign out",
            Some(Icon::Logout),
            Some(Message::Sc(ScMsg::AskSignOut)),
        ));
    }
    c.into()
}

#[cfg(test)]
mod tests;
