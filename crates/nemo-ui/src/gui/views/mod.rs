//! Views. [`root`] picks the shell or full-window Now Playing; the content router in
//! [`shell::content`] matches on [`Route`], so new screens add one arm.

pub mod bar;
pub mod home;
pub mod now_playing;
pub mod shell;

use iced::widget::{container, stack};
use iced::{Alignment, Element, Length, Padding};

use super::app::{App, Message};
use super::style::{col, fill, gap, label};
use crate::route::Route;
use crate::theme::{color, shape, type_scale};

/// The whole window.
pub fn root(app: &App) -> Element<'_, Message> {
    let (w, h) = app.window_px();
    let page = match app.ui.route() {
        Route::NowPlaying => now_playing::view(app, w, h),
        _ => shell::view(app, w, h),
    };
    let mut layers: Vec<Element<'_, Message>> = vec![
        fill(page, color::SURFACE)
            .width(Length::Fill)
            .height(Length::Fill)
            .into(),
    ];
    if app.readout {
        let text = app
            .frame_stats
            .readout()
            .map_or_else(|| "measuring\u{2026}".to_owned(), |r| r.text());
        let plate = container(label(
            text,
            type_scale::LABEL_SM,
            false,
            col(color::ON_SURFACE),
        ))
        .padding(Padding::from([gap::XS, gap::SM]))
        .style(|_| container::Style {
            background: Some(col(color::SURFACE_CONTAINER_HIGH).into()),
            border: iced::Border {
                radius: shape::SM.into(),
                ..iced::Border::default()
            },
            ..container::Style::default()
        });
        layers.push(
            container(plate)
                .padding(gap::SM)
                .align_x(Alignment::Start)
                .align_y(Alignment::Start)
                .into(),
        );
    }
    stack(layers).into()
}
