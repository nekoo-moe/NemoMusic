//! The SoundCloud account pieces both places draw: the SoundCloud card of the Settings page and
//! the SoundCloud card of the `Your accounts` sheet (source-workspaces spec). One set of
//! components and one set of messages (`ScMsg`), so the two places always show the same state.
//!
//! The card shows what the account state says: a note while the services have not started, the
//! sign-in controls while signed out (SoundCloud's own page, or a pasted token in a masked
//! field), and the name, avatar and `Sign out` with its confirmation while signed in. It never
//! shows a token, a cookie or an id.

use iced::font::Weight;
use iced::widget::{Space, column, row};
use iced::{Alignment, Element, Length};
use nemo_core::source_account::{SourceAccountInfo, SourceAccountState};
use nemo_core::track::Artwork;

use crate::copy::soundcloud as text;
use crate::gui::app::{App, ArtShape, Message, ScMsg};
use crate::gui::assets::Icon;
use crate::gui::style::{col, gap, label, label_w, spinner};
use crate::gui::views::account::{SecretField, notice_box, secret_field};
use crate::gui::views::kit::{art, filled_button, paragraph, text_button, tonal_button};
use crate::theme::{color, type_scale};

/// Where the card is drawn. The sheet has its own heading above the card, so it leaves out the
/// title lines the Settings page needs.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(in crate::gui::views) enum Place {
    /// The SoundCloud section of Settings.
    Settings,
    /// The `Your accounts` sheet.
    Sheet,
}

/// The card for the account of a running SoundCloud, or the note that says why there is none.
pub(in crate::gui::views) fn body(app: &App, width: f32, place: Place) -> Element<'_, Message> {
    let note = |t: &'static str| -> Element<'_, Message> {
        paragraph(
            t,
            type_scale::BODY_MD,
            col(color::on_surface_variant()),
            width,
        )
    };
    if !app.sc_running() {
        return column![note(text::STARTS_AFTER_RESTART)]
            .spacing(gap::MD)
            .into();
    }
    match app.backend.soundcloud_account.as_ref().map(|a| a.state()) {
        None => note(text::NO_STORE),
        Some(SourceAccountState::SignedIn(info)) => signed_in(app, &info, width, place),
        Some(state) => signed_out(app, &state, width, place),
    }
}

/// Notice box with SoundCloud development warning.
pub(in crate::gui::views) fn warning_box<'a>(
    lang: crate::settings::Language,
    width: f32,
) -> Element<'a, Message> {
    notice_box(
        Icon::Info,
        col(color::warning()),
        col(color::warning().with_alpha(0.12)),
        crate::i18n::t(
            lang,
            "SoundCloud development is currently moving at a snail's pace, so we honestly don't recommend using it because it doesn't work bro (╯°□°)╯︵ ┻━┻ 🐢",
            "SoundCloud đang được phát triển chậm như rùa bò, nên thật lòng chúng tôi không khuyến khích bạn sử dụng vì nó không chạy đâu bro (╯°□°)╯︵ ┻━┻ 🐢",
        ).to_owned(),
        width,
    )
}

fn signed_out<'a>(
    app: &'a App,
    state: &SourceAccountState,
    width: f32,
    place: Place,
) -> Element<'a, Message> {
    let sc = &app.sc;
    let busy = sc.busy || matches!(state, SourceAccountState::SigningIn);
    let dim = col(color::on_surface_variant());
    let lang = app.ui.settings.language;
    let mut c = column![].spacing(gap::MD).width(Length::Fixed(width));
    if place == Place::Settings {
        c = c.push(label(
            text::TITLE,
            type_scale::TITLE_SM,
            true,
            col(color::on_surface()),
        ));
    }
    c = c.push(warning_box(lang, width));
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
    let lang = app.ui.settings.language;
    let line = if app.sc.rechecked {
        text::STILL_NOTHING
    } else {
        text::STILL_WAITING
    };
    column![
        paragraph(line, type_scale::BODY_MD, col(color::on_surface()), width),
        tonal_button(
            crate::i18n::t(lang, text::CHECK_AGAIN, "Kiểm tra lại"),
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
    let lang = app.ui.settings.language;
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
                    crate::i18n::t(lang, text::CHECK_TOKEN, "Kiểm tra mã"),
                    Some(Icon::Key),
                    (can_type && typed).then_some(Message::Sc(ScMsg::PasteSubmit)),
                ),
                text_button(
                    crate::i18n::t(lang, text::USE_WINDOW, "Sử dụng cửa sổ đăng nhập"),
                    Message::Sc(ScMsg::PasteMode(false))
                ),
            ]
            .spacing(gap::SM)
            .align_y(Alignment::Center)
            .wrap()
            .vertical_spacing(gap::SM),
        ]
        .spacing(gap::MD)
        .into();
    }
    row![
        tonal_button(
            crate::i18n::t(lang, text::SIGN_IN, "Đăng nhập với SoundCloud"),
            Some(Icon::Login),
            can.then_some(Message::Sc(ScMsg::SignIn)),
        ),
        text_button(
            crate::i18n::t(lang, text::PASTE, "Dán mã token để thay thế"),
            Message::Sc(ScMsg::PasteMode(true))
        ),
    ]
    .spacing(gap::SM)
    .align_y(Alignment::Center)
    .wrap()
    .vertical_spacing(gap::SM)
    .into()
}

fn signed_in<'a>(
    app: &'a App,
    info: &SourceAccountInfo,
    width: f32,
    place: Place,
) -> Element<'a, Message> {
    let sc = &app.sc;
    let dim = col(color::on_surface_variant());
    let lang = app.ui.settings.language;
    let face = art(
        app,
        &Artwork::Remote(info.avatar.clone()),
        64,
        ArtShape::Circle,
    );
    let mut who = column![label_w(
        info.name.clone(),
        type_scale::TITLE_MD,
        Weight::Semibold,
        col(color::on_surface()),
    )]
    .spacing(gap::XS);
    if place == Place::Settings {
        who = who.push(label(text::TITLE, type_scale::BODY_MD, false, dim));
    }
    let mut c = column![
        row![face, Space::new().width(gap::LG), who].align_y(Alignment::Center),
        warning_box(lang, width),
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
                        crate::i18n::t(lang, "Sign out", "Đăng xuất"),
                        Icon::Logout,
                        app.accent(),
                        Message::Sc(ScMsg::SignOut)
                    ),
                    Space::new().width(gap::SM),
                    text_button(
                        crate::i18n::t(lang, "Cancel", "Hủy"),
                        Message::Sc(ScMsg::CancelSignOut)
                    ),
                ]
                .align_y(Alignment::Center),
            );
    } else if sc.busy {
        c = c.push(
            row![
                spinner(24.0, col(app.accent().primary), app.spin),
                Space::new().width(gap::MD),
                label(
                    crate::i18n::t(lang, text::SIGNING_OUT, "Đang đăng xuất\u{2026}"),
                    type_scale::BODY_MD,
                    false,
                    col(color::on_surface())
                ),
            ]
            .align_y(Alignment::Center),
        );
    } else {
        c = c.push(tonal_button(
            crate::i18n::t(lang, "Sign out", "Đăng xuất"),
            Some(Icon::Logout),
            Some(Message::Sc(ScMsg::AskSignOut)),
        ));
    }
    c.into()
}
