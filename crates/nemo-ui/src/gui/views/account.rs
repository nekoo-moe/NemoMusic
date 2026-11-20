//! The sign-in sheet (`design.md` section 11): where data lives, the Terms of Service notice,
//! "Sign in with Google", a masked "Paste cookie instead" fallback, plain-words errors, and the
//! signed-in card with Sign out and its confirmation. The cookie is a `Secret`; nothing here
//! prints it.

use iced::font::Weight;
use iced::widget::{Space, column, container, mouse_area, opaque, row, stack, text_input};
use iced::{Alignment, Background, Border, Color, Element, Length, Padding, mouse};
use nemo_core::account::{AccountInfo, AccountState};
use nemo_core::secret::Secret;
use nemo_core::track::Artwork;

use super::super::app::{AccountMsg, App, ArtShape, Message, ScMsg, SettingsSection};
use super::super::assets::{Icon, font};
use super::super::fx::Fx;
use super::super::style::{col, gap, icon, icon_button, label, label_w, scroll, shadow, spinner};
use super::kit::{art, filled_button, paragraph, text_button, tile, tonal_button, vspace};
use super::sc_account::{self, Place};
use crate::copy;
use crate::copy::soundcloud as sc_text;
use crate::state::Event;
use crate::theme::{color, glass, shape, type_scale};

/// Sheet width (floating landscape rectangle).
const WIDTH: f32 = 600.0;
/// Room kept between the sheet and the window edge, top and bottom.
const MARGIN: f32 = gap::LG;
/// Room the scroll bar keeps at the right of the cards, so it never covers a word.
const RAIL: f32 = gap::MD;
/// The sheet never gets shorter than this, so a tiny window still shows a scroll area.
const MIN_HEIGHT: f32 = 240.0;

/// A card of the sheet: its heading and what it shows, on `surface-container` inside the sheet.
fn account_card<'a>(
    title: &'static str,
    body: Element<'a, Message>,
    width: f32,
) -> Element<'a, Message> {
    tile(
        container(
            column![
                label(title, type_scale::TITLE_SM, true, col(color::on_surface())),
                body
            ]
            .spacing(gap::MD),
        )
        .padding(gap::LG)
        .width(Length::Fixed(width)),
        color::surface_container(),
    )
}

/// The YouTube Music card: what the sheet showed before the sheet held two accounts.
fn youtube_card(app: &App, width: f32) -> Element<'_, Message> {
    let inner = width - 2.0 * gap::LG;
    let state = app.backend.account.state();
    let body = match &state {
        AccountState::SignedIn(info) => signed_in(app, info, inner),
        _ => signed_out(app, matches!(state, AccountState::SigningIn), inner),
    };
    account_card("YouTube Music", body, width)
}

/// The SoundCloud card: the same pieces as the Settings card, or one sentence and the way to
/// The SoundCloud card: a collapsible dropdown card with a development warning and sign-in controls.
fn soundcloud_card(app: &App, width: f32) -> Element<'_, Message> {
    let inner = width - 2.0 * gap::LG;
    let open = app.sc.dropdown_open;
    let chevron = if open {
        Icon::ChevronDown
    } else {
        Icon::ChevronRight
    };

    let title_row = mouse_area(
        row![
            label(
                sc_text::TITLE,
                type_scale::TITLE_SM,
                true,
                col(color::on_surface())
            ),
            Space::new().width(gap::SM),
            container(
                row![
                    icon(Icon::Info, 12.0, col(color::warning())),
                    Space::new().width(4.0),
                    label(
                        "Development",
                        type_scale::LABEL_SM,
                        false,
                        col(color::warning())
                    ),
                ]
                .align_y(Alignment::Center),
            )
            .padding(Padding::from([2.0, 8.0]))
            .style(|_| container::Style {
                background: Some(Background::Color(col(color::warning().with_alpha(0.12)))),
                border: Border {
                    color: col(color::warning().with_alpha(0.30)),
                    width: 1.0,
                    radius: shape::sm().into(),
                },
                ..container::Style::default()
            }),
            Space::new().width(Length::Fill),
            icon(chevron, 18.0, col(color::on_surface_variant())),
        ]
        .align_y(Alignment::Center),
    )
    .interaction(mouse::Interaction::Pointer)
    .on_press(Message::Sc(ScMsg::ToggleDropdown));

    let content: Element<'_, Message> = if open {
        let body: Element<'_, Message> = if app.sc_open() {
            sc_account::body(app, inner, Place::Sheet)
        } else {
            column![
                sc_account::warning_box(inner),
                paragraph(
                    sc_text::CARD_OFF,
                    type_scale::BODY_MD,
                    col(color::on_surface_variant()),
                    inner
                ),
                text_button(
                    sc_text::CARD_TURN_ON,
                    Message::SettingsSectionGo(SettingsSection::SoundCloud)
                ),
            ]
            .spacing(gap::SM)
            .into()
        };
        column![title_row, body].spacing(gap::MD).into()
    } else {
        column![title_row].into()
    };

    tile(
        container(content)
            .padding(gap::LG)
            .width(Length::Fixed(width)),
        color::surface_container(),
    )
}

/// The sheet over a scrim: `Your accounts`, the `Listening area` control, then one card for each
/// service. It scrolls inside its card when the window is too short for all of it.
pub fn sheet(app: &App) -> Element<'_, Message> {
    let p = app.anim.sheet.value.clamp(0.0, 1.0);
    let (w, h) = app.window_px();
    // Window sizes are small exact numbers.
    #[allow(clippy::cast_precision_loss)]
    let (width, max_h) = (
        WIDTH.min((w as f32 - 2.0 * gap::LG).max(320.0)),
        (h as f32 - 2.0 * MARGIN).max(MIN_HEIGHT),
    );
    let inner = width - 2.0 * gap::XL;

    let header = row![
        label(
            copy::ACCOUNTS_TITLE,
            type_scale::TITLE_LG,
            true,
            col(color::on_surface())
        ),
        Space::new().width(Length::Fill),
        icon_button(
            Icon::Close,
            22.0,
            40.0,
            col(color::on_surface()),
            Color::TRANSPARENT,
            Some(Message::Account(AccountMsg::Close)),
        ),
    ]
    .align_y(Alignment::Center);
    let wide = inner - RAIL;

    let mut content = column![].spacing(gap::MD).width(Length::Fixed(inner));
    if let Some(area) = super::listening_area::control(app, wide) {
        content = content.push(area);
    }
    content = content.push(youtube_card(app, wide));
    if app.sc_info().is_some() {
        content = content.push(soundcloud_card(app, wide));
    }
    let card =
        container(column![header, vspace(gap::SM), scroll(content)].width(Length::Fixed(inner)))
            .padding(gap::XL)
            .width(Length::Fixed(width))
            .max_height(max_h)
            .style(|_| container::Style {
                background: Some(Background::Color(col(color::surface_container_high()))),
                border: Border {
                    color: col(color::outline_variant()),
                    width: 1.0,
                    radius: shape::xl().into(),
                },
                shadow: shadow(glass::SHADOW_OFFSET_Y, glass::SHADOW_BLUR, glass::SHADOW),
                ..container::Style::default()
            });
    let card = Fx::new(card)
        .passive()
        .entrance(p, 16.0, col(color::surface_container_high()));
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

pub(in crate::gui::views) fn notice_box<'a>(
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
                col(color::on_surface()),
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
            radius: shape::lg().into(),
            ..Border::default()
        },
        ..container::Style::default()
    })
    .into()
}

fn signed_out(app: &App, service_busy: bool, width: f32) -> Element<'_, Message> {
    let acc = &app.account;
    let busy = acc.busy || service_busy;
    let dim = col(color::on_surface_variant());
    let acknowledged = app.ui.settings.tos_acknowledged;
    let mut c = column![].spacing(gap::MD).width(Length::Fixed(width));

    if let Some(done) = &acc.notice {
        c = c.push(notice_box(
            Icon::CheckCircle,
            col(color::success()),
            col(color::success().with_alpha(0.12)),
            done.clone(),
            width,
        ));
    }
    c = c.push(paragraph(
        copy::ACCOUNT_LOCAL,
        type_scale::BODY_MD,
        col(color::on_surface()),
        width,
    ));
    c = c.push(notice_box(
        Icon::Info,
        col(color::warning()),
        col(color::warning().with_alpha(0.10)),
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
            col(color::error()),
            col(color::error().with_alpha(0.12)),
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
                    col(color::on_surface()),
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
                text_button(
                    copy::ACCOUNT_USE_WINDOW,
                    Message::Account(AccountMsg::CookieMode(false))
                ),
            ]
            .spacing(gap::SM)
            .align_y(Alignment::Center)
            .wrap()
            .vertical_spacing(gap::SM),
        );
    } else {
        c = c.push(
            row![
                filled_button_neutral(can),
                text_button(
                    copy::ACCOUNT_PASTE,
                    Message::Account(AccountMsg::CookieMode(true))
                ),
            ]
            .spacing(gap::SM)
            .align_y(Alignment::Center)
            .wrap()
            .vertical_spacing(gap::SM),
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
    secret_field(
        app,
        SecretField {
            placeholder: copy::ACCOUNT_COOKIE_PLACEHOLDER,
            value: &app.account.cookie,
            width,
            enabled,
        },
        |s| Message::Account(AccountMsg::CookieInput(s)),
        Message::Account(AccountMsg::CookieSubmit),
    )
}

/// What a masked field shows and whether it takes input.
#[derive(Clone, Copy)]
pub(in crate::gui::views) struct SecretField<'a> {
    /// Hint while the field is empty.
    pub placeholder: &'a str,
    /// What was typed. Shown as dots only.
    pub value: &'a Secret<String>,
    /// Field width.
    pub width: f32,
    /// `false` while a call runs: the field then ignores input.
    pub enabled: bool,
}

/// A masked field for a secret (a cookie or a token): dots only, never echoed.
pub(in crate::gui::views) fn secret_field<'a>(
    app: &App,
    field: SecretField<'a>,
    on_input: fn(Secret<String>) -> Message,
    on_submit: Message,
) -> Element<'a, Message> {
    let ring = col(app.accent().primary);
    let mut input = text_input(field.placeholder, field.value.expose())
        .secure(true)
        .size(f32::from(type_scale::BODY_MD.size))
        .font(font(false))
        .width(Length::Fixed(field.width))
        .padding(Padding::from([gap::MD, gap::LG]))
        .style(move |_, status| {
            let focused = matches!(status, text_input::Status::Focused { .. });
            text_input::Style {
                background: Background::Color(col(color::surface_container_highest())),
                border: Border {
                    width: if focused { 2.0 } else { 0.0 },
                    radius: shape::lg().into(),
                    color: ring,
                },
                icon: col(color::on_surface_variant()),
                placeholder: col(color::on_surface_variant()),
                value: col(color::on_surface()),
                selection: col(color::PRIMARY_FALLBACK.with_alpha(0.4)),
            }
        });
    if field.enabled {
        input = input
            .on_input(move |s| on_input(Secret::new(s)))
            .on_submit(on_submit);
    }
    input.into()
}

fn signed_in<'a>(app: &'a App, info: &AccountInfo, width: f32) -> Element<'a, Message> {
    let dim = col(color::on_surface_variant());
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
        col(color::on_surface())
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
            col(color::error()),
            col(color::error().with_alpha(0.12)),
            e.clone(),
            width,
        ));
    }
    if acc.confirm_sign_out {
        c = c.push(paragraph(
            copy::ACCOUNT_SIGN_OUT_CONFIRM,
            type_scale::BODY_MD,
            col(color::on_surface()),
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
                    col(color::on_surface())
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

#[cfg(test)]
mod tests;
