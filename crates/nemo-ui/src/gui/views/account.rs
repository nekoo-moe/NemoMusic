//! The sign-in sheet (`design.md` section 11): where data lives, the Terms of Service notice,
//! "Sign in with Google", a masked "Paste cookie instead" fallback, plain-words errors, and the
//! signed-in card with Sign out and its confirmation. The cookie is a `Secret`; nothing here
//! prints it.

use iced::font::Weight;
use iced::widget::{Space, column, container, mouse_area, opaque, row, stack, text_input};
use iced::{Alignment, Background, Border, Color, Element, Length, Padding};
use nemo_core::account::{AccountInfo, AccountState};
use nemo_core::secret::Secret;
use nemo_core::track::Artwork;

use super::super::app::{AccountMsg, App, ArtShape, Message};
use super::super::assets::{Icon, font};
use super::super::fx::Fx;
use super::super::style::{col, gap, icon, icon_button, label, label_w, shadow, spinner};
use super::kit::{art, filled_button, paragraph, text_button, tonal_button, vspace};
use crate::copy;
use crate::state::Event;
use crate::theme::{color, glass, shape, type_scale};

/// Sheet width.
const WIDTH: f32 = 440.0;

/// The sheet over a scrim.
pub fn sheet(app: &App) -> Element<'_, Message> {
    let p = app.anim.sheet.value.clamp(0.0, 1.0);
    let (w, _) = app.window_px();
    // Window widths are small exact numbers.
    #[allow(clippy::cast_precision_loss)]
    let width = WIDTH.min((w as f32 - 2.0 * gap::LG).max(320.0));
    let inner = width - 2.0 * gap::XL;

    let state = app.backend.account.state();
    let body = match &state {
        AccountState::SignedIn(info) => signed_in(app, info, inner),
        _ => signed_out(app, matches!(state, AccountState::SigningIn), inner),
    };
    let title = match &state {
        AccountState::SignedIn(_) => "Your account",
        _ => copy::ACCOUNT_TITLE,
    };
    let header = row![
        label(title, type_scale::TITLE_LG, true, col(color::ON_SURFACE)),
        Space::new().width(Length::Fill),
        icon_button(
            Icon::Close,
            22.0,
            40.0,
            col(color::ON_SURFACE),
            Color::TRANSPARENT,
            Some(Message::Account(AccountMsg::Close)),
        ),
    ]
    .align_y(Alignment::Center);
    let card = container(column![header, vspace(gap::SM), body].width(Length::Fixed(inner)))
        .padding(gap::XL)
        .width(Length::Fixed(width))
        .style(|_| container::Style {
            background: Some(Background::Color(col(color::SURFACE_CONTAINER_HIGH))),
            border: Border {
                color: col(color::OUTLINE_VARIANT),
                width: 1.0,
                radius: shape::XL.into(),
            },
            shadow: shadow(glass::SHADOW_OFFSET_Y, glass::SHADOW_BLUR, glass::SHADOW),
            ..container::Style::default()
        });
    let card = Fx::new(card)
        .passive()
        .entrance(p, 16.0, col(color::SURFACE_CONTAINER_HIGH));
    let scrim = mouse_area(
        container(Space::new())
            .width(Length::Fill)
            .height(Length::Fill)
            .style(move |_| container::Style {
                background: Some(Background::Color(col(color::SCRIM.with_alpha(0.60 * p)))),
                ..container::Style::default()
            }),
    )
    .on_press(Message::Account(AccountMsg::Close));
    stack![
        scrim,
        container(opaque(card))
            .width(Length::Fill)
            .height(Length::Fill)
            .center_x(Length::Fill)
            .center_y(Length::Fill),
    ]
    .into()
}

fn notice_box<'a>(
    glyph: Icon,
    ink: Color,
    tint: Color,
    text: String,
    width: f32,
) -> Element<'a, Message> {
    container(
        row![
            icon(glyph, 20.0, ink),
            Space::new().width(gap::MD),
            paragraph(
                text,
                type_scale::BODY_SM,
                col(color::ON_SURFACE),
                width - 20.0 - gap::MD - 2.0 * gap::MD
            ),
        ]
        .align_y(Alignment::Start),
    )
    .padding(gap::MD)
    .width(Length::Fixed(width))
    .style(move |_| container::Style {
        background: Some(Background::Color(tint)),
        border: Border {
            radius: shape::LG.into(),
            ..Border::default()
        },
        ..container::Style::default()
    })
    .into()
}

fn signed_out(app: &App, service_busy: bool, width: f32) -> Element<'_, Message> {
    let acc = &app.account;
    let busy = acc.busy || service_busy;
    let dim = col(color::ON_SURFACE_VARIANT);
    let acknowledged = app.ui.settings.tos_acknowledged;
    let mut c = column![].spacing(gap::MD).width(Length::Fixed(width));

    if let Some(done) = &acc.notice {
        c = c.push(notice_box(
            Icon::CheckCircle,
            col(color::SUCCESS),
            col(color::SUCCESS.with_alpha(0.12)),
            done.clone(),
            width,
        ));
    }
    c = c.push(paragraph(
        copy::ACCOUNT_LOCAL,
        type_scale::BODY_MD,
        col(color::ON_SURFACE),
        width,
    ));
    c = c.push(notice_box(
        Icon::Info,
        col(color::WARNING),
        col(color::WARNING.with_alpha(0.10)),
        copy::ACCOUNT_TOS.to_owned(),
        width,
    ));
    if !acknowledged {
        c = c.push(
            row![
                label(copy::ACCOUNT_NEEDS_NOTICE, type_scale::BODY_SM, false, dim),
                Space::new().width(Length::Fill),
                text_button("Review notice", Message::Ui(Event::TosReview)),
            ]
            .align_y(Alignment::Center),
        );
    }
    if let Some(e) = &acc.error {
        c = c.push(notice_box(
            Icon::Info,
            col(color::ERROR),
            col(color::ERROR.with_alpha(0.12)),
            e.clone(),
            width,
        ));
    }
    if busy {
        let waiting = if acc.cookie_mode {
            copy::ACCOUNT_CHECKING
        } else {
            copy::ACCOUNT_WAITING
        };
        c = c.push(
            row![
                spinner(24.0, col(app.accent().primary), app.spin),
                Space::new().width(gap::MD),
                paragraph(
                    waiting,
                    type_scale::BODY_MD,
                    col(color::ON_SURFACE),
                    width - 24.0 - gap::MD
                ),
            ]
            .align_y(Alignment::Center),
        );
    }

    let can = acknowledged && !busy;
    if acc.cookie_mode {
        c = c.push(cookie_field(app, width, can));
        c = c.push(paragraph(
            copy::ACCOUNT_COOKIE_HELP,
            type_scale::BODY_SM,
            dim,
            width,
        ));
        c = c.push(
            row![
                tonal_button(
                    "Check cookie",
                    Some(Icon::Key),
                    (can && !acc.cookie.expose().trim().is_empty())
                        .then_some(Message::Account(AccountMsg::CookieSubmit)),
                ),
                Space::new().width(gap::SM),
                text_button(
                    copy::ACCOUNT_USE_WINDOW,
                    Message::Account(AccountMsg::CookieMode(false))
                ),
            ]
            .align_y(Alignment::Center),
        );
    } else {
        c = c.push(
            row![
                filled_button_neutral(can),
                Space::new().width(gap::SM),
                text_button(
                    copy::ACCOUNT_PASTE,
                    Message::Account(AccountMsg::CookieMode(true))
                ),
            ]
            .align_y(Alignment::Center),
        );
    }
    c.into()
}

/// "Sign in with Google": neutral filled tonal, no logo.
fn filled_button_neutral<'a>(enabled: bool) -> Element<'a, Message> {
    tonal_button(
        copy::ACCOUNT_SIGN_IN,
        Some(Icon::Login),
        enabled.then_some(Message::Account(AccountMsg::SignIn)),
    )
}

/// The masked cookie field.
fn cookie_field(app: &App, width: f32, enabled: bool) -> Element<'_, Message> {
    let ring = col(app.accent().primary);
    let mut input = text_input(
        copy::ACCOUNT_COOKIE_PLACEHOLDER,
        app.account.cookie.expose(),
    )
    .secure(true)
    .size(f32::from(type_scale::BODY_MD.size))
    .font(font(false))
    .width(Length::Fixed(width))
    .padding(Padding::from([gap::MD, gap::LG]))
    .style(move |_, status| {
        let focused = matches!(status, text_input::Status::Focused { .. });
        text_input::Style {
            background: Background::Color(col(color::SURFACE_CONTAINER_HIGHEST)),
            border: Border {
                width: if focused { 2.0 } else { 0.0 },
                radius: shape::LG.into(),
                color: ring,
            },
            icon: col(color::ON_SURFACE_VARIANT),
            placeholder: col(color::ON_SURFACE_VARIANT),
            value: col(color::ON_SURFACE),
            selection: col(color::PRIMARY_FALLBACK.with_alpha(0.4)),
        }
    });
    if enabled {
        input = input
            .on_input(|s| Message::Account(AccountMsg::CookieInput(Secret::new(s))))
            .on_submit(Message::Account(AccountMsg::CookieSubmit));
    }
    input.into()
}

fn signed_in<'a>(app: &'a App, info: &AccountInfo, width: f32) -> Element<'a, Message> {
    let dim = col(color::ON_SURFACE_VARIANT);
    let acc = &app.account;
    let face: Element<'a, Message> = art(
        app,
        &Artwork::Remote(info.avatar.clone()),
        64,
        ArtShape::Circle,
    );
    let mut who = column![label_w(
        info.name.clone(),
        type_scale::TITLE_MD,
        Weight::Semibold,
        col(color::ON_SURFACE)
    )]
    .spacing(gap::XS);
    if let Some(h) = &info.handle {
        who = who.push(label(h.clone(), type_scale::BODY_MD, false, dim));
    }
    let mut c = column![
        row![face, Space::new().width(gap::LG), who].align_y(Alignment::Center),
        paragraph(copy::ACCOUNT_LOCAL, type_scale::BODY_SM, dim, width),
    ]
    .spacing(gap::LG)
    .width(Length::Fixed(width));
    if let Some(e) = &acc.error {
        c = c.push(notice_box(
            Icon::Info,
            col(color::ERROR),
            col(color::ERROR.with_alpha(0.12)),
            e.clone(),
            width,
        ));
    }
    if acc.confirm_sign_out {
        c = c.push(paragraph(
            copy::ACCOUNT_SIGN_OUT_CONFIRM,
            type_scale::BODY_MD,
            col(color::ON_SURFACE),
            width,
        ));
        c = c.push(
            row![
                filled_button(
                    "Sign out",
                    Icon::Logout,
                    app.accent(),
                    Message::Account(AccountMsg::SignOut)
                ),
                Space::new().width(gap::SM),
                text_button("Cancel", Message::Account(AccountMsg::CancelSignOut)),
            ]
            .align_y(Alignment::Center),
        );
    } else if acc.busy {
        c = c.push(
            row![
                spinner(24.0, col(app.accent().primary), app.spin),
                Space::new().width(gap::MD),
                label(
                    "Signing out\u{2026}",
                    type_scale::BODY_MD,
                    false,
                    col(color::ON_SURFACE)
                ),
            ]
            .align_y(Alignment::Center),
        );
    } else {
        c = c.push(tonal_button(
            "Sign out",
            Some(Icon::Logout),
            Some(Message::Account(AccountMsg::AskSignOut)),
        ));
    }
    c.into()
}
