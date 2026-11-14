//! The Privacy and About sections.

use iced::widget::{column, container};
use iced::{Color, Element, Padding};

use crate::copy;
use crate::gui::app::Message;
use crate::gui::style::{col, gap, label};
use crate::gui::views::kit::{paragraph, text_button};
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

pub(super) fn about(width: f32) -> Vec<Element<'static, Message>> {
    let dim = col(color::on_surface_variant());
    let inner = width - 2.0 * gap::LG;
    let block = |title: &'static str, body: String, ink: Color| -> Element<'static, Message> {
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
    vec![
        block(
            "NemoMusic",
            format!("Version {}", env!("CARGO_PKG_VERSION")),
            dim,
        ),
        block("Licences", copy::ABOUT_LICENSE.to_owned(), dim),
        block("Credits", copy::ABOUT_CREDITS.join("\n"), dim),
        block("YouTube Music", copy::ABOUT_TOS.to_owned(), dim),
        container(text_button(
            "Review the first-run notice",
            Message::Ui(Event::TosReview),
        ))
        .padding(Padding::from([gap::XS, gap::SM]))
        .into(),
    ]
}
