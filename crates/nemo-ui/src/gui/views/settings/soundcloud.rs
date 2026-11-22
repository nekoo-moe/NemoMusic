//! The SoundCloud section (soundcloud-account spec): the account card (while SoundCloud is on),
//! then the opt-in row.
//!
//! The row is the shared `OptInRow` (what is sent, to which hosts, the risk, "Turn on" in a panel
//! that has no pre-selected button). The card shows only while SoundCloud is on and running: sign
//! in with SoundCloud's own page or with a pasted token (masked, never echoed), the signed-in
//! name and avatar, and Sign out with its confirmation. Plain words everywhere: the access is
//! unofficial, the token stays on this computer, the audio is lossy.

use iced::widget::{Space, column, container, row};
use iced::{Alignment, Element, Length, Padding};
use nemo_core::source_account::SourceAccountState;

use crate::copy::soundcloud as text;
use crate::gui::app::{App, Message, ScMsg};
use crate::gui::assets::Icon;
use crate::gui::style::{col, gap};
use crate::gui::views::kit::{filled_button, paragraph, text_button, tile};
use crate::gui::views::listening_area;
use crate::gui::views::opt_in::{Extras, OptInActions, OptInPhase, OptInRow, opt_in_row_with};
use crate::gui::views::sc_account::{self, Place};
use crate::gui::views::settings::appearance::restart;
use crate::theme::{color, type_scale};

/// The whole section: the opt-in row and, when it applies, the account card.
pub(super) fn section(app: &App, width: f32) -> Element<'_, Message> {
    let mut page = column![].spacing(gap::LG).width(Length::Fixed(width));
    // The listening area is chosen here and in the account sheet, nowhere else.
    if let Some(area) = area_row(app, width) {
        page = page.push(group(vec![area], width));
    }
    // The account comes first while SoundCloud is on: the switch row below it lists what is
    // sent, to which hosts and the risk, which is long.
    if app.sc_open() {
        page = page.push(group(vec![account_rows(app, width)], width));
    }
    page = page.push(group(vec![switch_row(app, width)], width));
    page.into()
}

/// The listening-area card: the same control as the account sheet. None in a build without
/// SoundCloud.
fn area_row(app: &App, width: f32) -> Option<Element<'_, Message>> {
    let control = listening_area::control(app, width - 2.0 * gap::LG)?;
    Some(
        container(control)
            .padding(Padding::from([gap::MD, gap::LG]))
            .width(Length::Fixed(width))
            .into(),
    )
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
fn status(app: &App, lang: crate::settings::Language) -> &'static str {
    if !app.sc_running() {
        return if app.sc_restart_offered() {
            crate::i18n::t(
                lang,
                text::NEEDS_RESTART,
                "SoundCloud sẽ khởi chạy sau khi khởi động lại. Âm nhạc sẽ tạm dừng trong giây lát.",
            )
        } else {
            crate::i18n::t(
                lang,
                text::STARTS_AFTER_RESTART,
                "SoundCloud sẽ khởi chạy vào lần tiếp theo NemoMusic mở.",
            )
        };
    }
    match app.sc_state() {
        Some(SourceAccountState::SignedIn(_)) => crate::i18n::t(
            lang,
            text::STATUS_SIGNED_IN,
            "Bật. Bạn đã đăng nhập. Tắt SoundCloud sẽ đăng xuất và xóa token của bạn khỏi máy tính này.",
        ),
        _ => crate::i18n::t(
            lang,
            text::STATUS_SIGNED_OUT,
            "Bật. Bạn chưa đăng nhập. Tắt SoundCloud sẽ xóa mọi thông tin đăng nhập đã lưu khỏi máy tính này.",
        ),
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
    let lang = app.ui.settings.language;
    let Some(info) = app.sc_info() else {
        return container(paragraph(
            crate::i18n::t(
                lang,
                text::NOT_IN_THIS_BUILD,
                "SoundCloud không được hỗ trợ trong bản dựng này.",
            ),
            type_scale::BODY_MD,
            col(color::on_surface_variant()),
            width - 2.0 * gap::LG,
        ))
        .padding(Padding::from([gap::MD, gap::LG]))
        .into();
    };
    let phase = phase(app);
    let stays_text = app.sc.error.as_deref().unwrap_or(crate::i18n::t(
        lang,
        text::STAYS,
        "Tắt SoundCloud sẽ đăng xuất và xóa token của bạn khỏi máy tính này.",
    ));
    let row = OptInRow {
        info,
        phase,
        status: Some(status(app, lang)),
        // Off: what turning it off did. A failed removal is told here, where the switch is.
        stays: Some(stays_text),
        lang,
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
    let lang = app.ui.settings.language;
    let inner = width - 2.0 * gap::LG;
    let confirm_body = crate::i18n::t(
        lang,
        text::OFF_CONFIRM,
        "Tắt SoundCloud sẽ đăng xuất và xóa token cũng như dữ liệu SoundCloud mà NemoMusic đã tải trên máy tính này. Thao tác này không đăng xuất soundcloud.com trên trình duyệt của bạn. Bạn có thể bật lại và đăng nhập sau.",
    );
    let confirm_btn = crate::i18n::t(lang, text::OFF_CONFIRM_BUTTON, "Tắt và đăng xuất");
    let cancel_btn = crate::i18n::t(lang, "Cancel", "Hủy");
    column![
        paragraph(
            confirm_body,
            type_scale::BODY_MD,
            col(color::on_surface()),
            inner,
        ),
        row![
            filled_button(
                confirm_btn,
                Icon::Logout,
                app.accent(),
                Message::Sc(ScMsg::ConfirmOff),
            ),
            Space::new().width(gap::SM),
            text_button(cancel_btn, Message::Sc(ScMsg::CancelOff)),
        ]
        .align_y(Alignment::Center),
    ]
    .spacing(gap::MD)
    .into()
}

/// The account card, or the reason there is none.
fn account_rows(app: &App, width: f32) -> Element<'_, Message> {
    let inner = width - 2.0 * gap::LG;
    container(sc_account::body(app, inner, Place::Settings))
        .padding(Padding::from([gap::MD, gap::LG]))
        .width(Length::Fixed(width))
        .into()
}

#[cfg(test)]
mod tests;
