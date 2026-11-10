//! The queue panel (right, 320 px, opaque `surface-container`, never glass): now playing, next
//! up, remove, play next and clear.

use iced::widget::{Space, button, column, container, row, scrollable};
use iced::{Alignment, Color, Element, Length, Padding};
use nemo_core::track::Track;

use super::super::app::{App, ArtShape, Message, ROWS_STEP};
use super::super::assets::Icon;
use super::super::style::{button_style, col, fill, fit, gap, icon_button, label, tip};
use super::kit::{art, text_button, vspace};
use crate::format;
use crate::layout;
use crate::state::{Event, QueueOp};
use crate::theme::{color, shape, type_scale};

/// Width available to the title column of a row.
const TEXT_W: f32 = 168.0;

/// The panel.
pub fn panel(app: &App) -> Element<'_, Message> {
    let snap = &app.ui.snapshot;
    let dim = col(color::ON_SURFACE_VARIANT);
    let can_clear = snap.queue.len() > 1;

    let header = row![
        label("Queue", type_scale::TITLE_MD, true, col(color::ON_SURFACE)),
        Space::new().width(Length::Fill),
        if can_clear {
            text_button("Clear", Message::Ui(Event::Queue(QueueOp::Clear)))
        } else {
            Space::new().into()
        },
        icon_button(
            Icon::Close,
            20.0,
            40.0,
            col(color::ON_SURFACE),
            Color::TRANSPARENT,
            Some(Message::Ui(Event::ToggleQueue)),
        ),
    ]
    .align_y(Alignment::Center)
    .height(Length::Fixed(56.0))
    .padding(Padding::from([0.0, gap::LG]));

    let mut body = column![].spacing(0).padding(Padding::from([0.0, gap::SM]));
    if let Some(cur) = snap.queue_index.and_then(|i| snap.queue.get(i)) {
        body = body
            .push(section_label("Now playing"))
            .push(item(app, cur, 0, true, 56.0));
    } else {
        body = body.push(section_label("Nothing is playing"));
    }

    let start = snap.queue_index.map_or(0, |i| i + 1);
    let next = snap.queue.get(start..).unwrap_or_default();
    if next.is_empty() {
        if !snap.queue.is_empty() {
            body = body.push(vspace(gap::SM)).push(
                container(label(
                    "Nothing queued after this song.",
                    type_scale::BODY_SM,
                    false,
                    dim,
                ))
                .padding(Padding::from([gap::SM, gap::MD])),
            );
        }
    } else {
        body = body.push(section_label("Next up"));
        for (offset, t) in next.iter().enumerate().take(ROWS_STEP) {
            body = body.push(item(app, t, start + offset, false, 48.0));
        }
    }
    let earlier = snap.queue_index.unwrap_or(0).min(snap.queue.len());
    if earlier > 0 {
        body = body.push(section_label("Earlier"));
        for (i, t) in snap.queue.iter().enumerate().take(earlier).take(ROWS_STEP) {
            body = body.push(item(app, t, i, false, 48.0));
        }
    }

    // Opaque `surface-container`, not glass.
    fill(
        column![header, scrollable(body).height(Length::Fill)],
        color::SURFACE_CONTAINER,
    )
    .width(Length::Fixed(f32::from(
        u16::try_from(layout::QUEUE_WIDTH).unwrap_or(320),
    )))
    .height(Length::Fill)
    .into()
}

fn section_label(text: &str) -> Element<'_, Message> {
    container(label(
        text,
        type_scale::LABEL_SM,
        false,
        col(color::ON_SURFACE_VARIANT),
    ))
    .padding(Padding {
        top: gap::MD,
        bottom: gap::XS,
        left: gap::SM,
        right: gap::SM,
    })
    .into()
}

/// One queue row: cover, title and artist; the current one has no buttons.
fn item<'a>(
    app: &'a App,
    t: &'a Track,
    index: usize,
    current: bool,
    height: f32,
) -> Element<'a, Message> {
    let dim = col(color::ON_SURFACE_VARIANT);
    let main = button(
        container(
            row![
                art(app, &t.artwork, 40, ArtShape::Row),
                Space::new().width(gap::MD),
                column![
                    label(
                        fit(&t.title, TEXT_W, type_scale::BODY_MD.size),
                        type_scale::BODY_MD,
                        true,
                        col(color::ON_SURFACE),
                    ),
                    label(
                        fit(
                            &format::artists(&t.artists),
                            TEXT_W,
                            type_scale::BODY_SM.size
                        ),
                        type_scale::BODY_SM,
                        false,
                        dim,
                    ),
                ]
                .width(Length::Fixed(TEXT_W)),
            ]
            .align_y(Alignment::Center),
        )
        .height(Length::Fixed(height))
        .padding(Padding::from([0.0, gap::SM]))
        .align_y(Alignment::Center),
    )
    .padding(0)
    .style(button_style(
        if current {
            col(color::SURFACE_CONTAINER_HIGH)
        } else {
            Color::TRANSPARENT
        },
        shape::MD,
    ))
    .width(Length::Fill)
    .on_press(Message::Ui(Event::Queue(QueueOp::Jump(index))));
    let mut r = row![main].align_y(Alignment::Center);
    if !current {
        r = r
            .push(tip(
                icon_button(
                    Icon::Next,
                    16.0,
                    32.0,
                    dim,
                    Color::TRANSPARENT,
                    Some(Message::Ui(Event::Queue(QueueOp::PlayNext(index)))),
                ),
                "Play next",
            ))
            .push(tip(
                icon_button(
                    Icon::Close,
                    16.0,
                    32.0,
                    dim,
                    Color::TRANSPARENT,
                    Some(Message::Ui(Event::Queue(QueueOp::Remove(index)))),
                ),
                "Remove",
            ));
    }
    r.into()
}
