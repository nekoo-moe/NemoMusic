//! The queue, shown as a tab of the right pane (opaque, never glass): now playing, next up,
//! remove, play next and clear.

use iced::widget::{Space, button, column, container, row};
use iced::{Alignment, Color, Element, Length, Padding};
use nemo_core::track::Track;

use super::super::app::{App, ArtShape, Message, ROWS_STEP};
use super::super::assets::Icon;
use super::super::style::{animated, col, fit, gap, icon_button, label, scroll, tip};
use super::kit::{art, text_button, vspace};
use crate::format;
use crate::state::{Event, QueueOp};
use crate::theme::{color, shape, type_scale};

/// The queue tab for a pane `width` px wide: a header with Clear, then the rows.
pub fn tab(app: &App, width: f32) -> Element<'_, Message> {
    let snap = &app.ui.snapshot;
    let dim = col(color::ON_SURFACE_VARIANT);
    let can_clear = snap.queue.len() > 1;
    // Row text width: pane less padding, art 48, gap 12 and the two small buttons.
    let text_w = (width - 24.0 - 48.0 - gap::MD - 2.0 * 32.0 - gap::SM).max(80.0);

    let mut body = column![].spacing(0);
    if let Some(cur) = snap.queue_index.and_then(|i| snap.queue.get(i)) {
        body = body
            .push(section_label("Now playing"))
            .push(item(app, cur, 0, true, text_w));
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
            body = body.push(item(app, t, start + offset, false, text_w));
        }
    }
    let earlier = snap.queue_index.unwrap_or(0).min(snap.queue.len());
    if earlier > 0 {
        body = body.push(section_label("Earlier"));
        for (i, t) in snap.queue.iter().enumerate().take(earlier).take(ROWS_STEP) {
            body = body.push(item(app, t, i, false, text_w));
        }
    }

    let header = row![
        label(
            format!("{} in queue", snap.queue.len()),
            type_scale::BODY_SM,
            false,
            dim
        ),
        Space::new().width(Length::Fill),
        if can_clear {
            text_button("Clear", Message::Ui(Event::Queue(QueueOp::Clear)))
        } else {
            Space::new().into()
        },
    ]
    .align_y(Alignment::Center)
    .height(Length::Fixed(40.0))
    .padding(Padding::from([0.0, gap::SM]));
    column![
        header,
        scroll(container(body).padding(Padding {
            bottom: super::kit::PAGE_BOTTOM,
            ..Padding::ZERO
        }))
        .height(Length::Fill)
    ]
    .into()
}

fn section_label(text: &str) -> Element<'_, Message> {
    container(label(
        text,
        type_scale::LABEL_MD,
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
    text_w: f32,
) -> Element<'a, Message> {
    let dim = col(color::ON_SURFACE_VARIANT);
    let accent = app.accent();
    let line = row![
        art(app, &t.artwork, 48, ArtShape::Thumb),
        Space::new().width(gap::MD),
        column![
            label(
                fit(&t.title, text_w, type_scale::BODY_MD.size),
                type_scale::BODY_MD,
                true,
                if current {
                    col(accent.primary)
                } else {
                    col(color::ON_SURFACE)
                },
            ),
            label(
                fit(
                    &format::artists(&t.artists),
                    text_w,
                    type_scale::BODY_SM.size
                ),
                type_scale::BODY_SM,
                false,
                dim,
            ),
        ]
        .width(Length::Fixed(text_w)),
    ]
    .align_y(Alignment::Center);
    let main = button(
        container(line)
            .height(Length::Fixed(60.0))
            .padding(Padding::from([0.0, gap::SM]))
            .align_y(Alignment::Center),
    )
    .padding(0)
    .width(Length::Fill)
    .on_press(Message::Ui(Event::Queue(QueueOp::Jump(index))));
    let idle = if current {
        col(color::SURFACE_CONTAINER_HIGH)
    } else {
        Color::TRANSPARENT
    };
    let mut r = row![animated(main, idle, shape::LG)].align_y(Alignment::Center);
    if !current {
        r = r
            .push(tip(
                icon_button(
                    Icon::Next,
                    18.0,
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
                    18.0,
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
