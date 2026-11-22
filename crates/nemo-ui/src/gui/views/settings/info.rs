//! The Privacy and About sections.

use iced::widget::{column, container, mouse_area};
use iced::{Color, Element, Padding};

use super::control::{segmented, setting_row};
use crate::copy;
use crate::gui::app::dev::DevMsg;
use crate::gui::app::{App, Message};
use crate::gui::assets::Icon;
use crate::gui::style::{col, gap, label};
use crate::gui::views::kit::{paragraph, text_button};
use crate::i18n;
use crate::settings::{Language, SettingChange};
use crate::setup::SetupEvent;
use crate::state::Event;
use crate::theme::{color, type_scale};

pub(super) fn privacy(width: f32) -> Vec<Element<'static, Message>> {
    let dim = col(color::on_surface_variant());
    let inner = width - 2.0 * gap::LG;
    let mut rows: Vec<Element<'static, Message>> = vec![
        container(paragraph(
            copy::PRIVACY_INTRO,
            type_scale::BODY_MD,
            col(color::on_surface()),
            inner,
        ))
        .padding(Padding::from([gap::MD, gap::LG]))
        .into(),
    ];
    for (host, why) in copy::EGRESS {
        rows.push(
            container(
                column![
                    paragraph(host, type_scale::BODY_MD, col(color::on_surface()), inner),
                    paragraph(why, type_scale::BODY_SM, dim, inner),
                ]
                .spacing(gap::XS),
            )
            .padding(Padding::from([gap::SM, gap::LG]))
            .into(),
        );
    }
    rows.push(
        container(paragraph(
            copy::PRIVACY_DATA,
            type_scale::BODY_SM,
            dim,
            inner,
        ))
        .padding(Padding::from([gap::MD, gap::LG]))
        .into(),
    );
    rows
}

const LANGUAGES: [(Language, &str, Option<Icon>); 2] = [
    (Language::English, "English", None),
    (Language::Vietnamese, "Tiếng Việt", None),
];

pub(super) fn about(app: &App, width: f32) -> Vec<Element<'_, Message>> {
    let dim = col(color::on_surface_variant());
    let inner = width - 2.0 * gap::LG;
    let lang = app.ui.settings.language;
    let text = i18n::about_text(lang);

    let block = |title: &'static str, body: String, ink: Color| -> Element<'_, Message> {
        container(
            column![
                label(title, type_scale::BODY_MD, true, col(color::on_surface())),
                paragraph(body, type_scale::BODY_SM, ink, inner),
            ]
            .spacing(gap::XS),
        )
        .padding(Padding::from([gap::MD, gap::LG]))
        .into()
    };
    // Ten clicks on the title and the version open the developer panel (dev-tools spec). The
    // text looks and behaves like any other: no hover, no cursor, no hint, no count.
    let version = mouse_area(block(
        "NemoMusic",
        format!("{} {}", text.version_label, env!("CARGO_PKG_VERSION")),
        dim,
    ))
    .on_press(Message::Dev(DevMsg::Click));

    vec![
        setting_row(
            width,
            text.language_title,
            Some(text.language_help.to_string()),
            segmented(app, &LANGUAGES, lang, SettingChange::Language),
        ),
        setting_row(
            width,
            text.updates_title,
            Some(text.updates_help.to_string()),
            crate::gui::views::settings::switch(
                app,
                app.ui.settings.check_for_updates,
                SettingChange::CheckForUpdates,
            ),
        ),
        version.into(),
        block(text.licenses_title, text.licenses_body.to_owned(), dim),
        block(text.credits_title, copy::ABOUT_CREDITS.join("\n"), dim),
        block(text.tos_title, text.tos_body.to_owned(), dim),
        container(text_button(
            text.review_notice,
            Message::Ui(Event::TosReview),
        ))
        .padding(Padding::from([gap::XS, gap::SM]))
        .into(),
        container(
            column![
                label(
                    text.setup_title,
                    type_scale::BODY_MD,
                    true,
                    col(color::on_surface())
                ),
                paragraph(text.setup_help, type_scale::BODY_SM, dim, inner),
                text_button(
                    text.setup_button,
                    Message::Ui(Event::Setup(SetupEvent::Open)),
                ),
            ]
            .spacing(gap::XS),
        )
        .padding(Padding::from([gap::MD, gap::LG]))
        .into(),
    ]
}
