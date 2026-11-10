//! Views. [`root`] picks the shell or full-window Now Playing and stacks the overlays; the
//! content router in [`shell`] matches on [`Route`], so a new screen adds one arm.

pub mod bar;
pub mod detail;
pub mod home;
pub mod kit;
pub mod library;
pub mod now_playing;
pub mod overlays;
pub mod queue;
pub mod search;
pub mod settings;
pub mod shell;

use iced::widget::{container, stack};
use iced::{Alignment, Element, Length, Padding};

use super::app::{App, Message};
use super::style::{col, fill, gap, label};
use crate::layout;
use crate::route::Route;
use crate::theme::{color, shape, type_scale};

/// The whole window.
pub fn root(app: &App) -> Element<'_, Message> {
    let (w, h) = app.window_px();
    let now_playing = matches!(app.ui.route(), Route::NowPlaying);
    let page = if now_playing {
        now_playing::view(app, w, h)
    } else {
        shell::view(app, w, h)
    };
    let mut layers: Vec<Element<'_, Message>> = vec![
        fill(page, color::SURFACE)
            .width(Length::Fill)
            .height(Length::Fill)
            .into(),
    ];

    // Insets of the main column, so notices sit above the bar and not under the panels.
    let (left, right, bar_space) = if now_playing {
        (0, 0, gap::XL)
    } else {
        let queue = if layout::queue_pushes_content(w, app.ui.queue_open) {
            layout::QUEUE_WIDTH
        } else {
            0
        };
        let bottom = if app.ui.snapshot.current.is_some() {
            // Bar 88 + inset 12 above and below.
            f32::from(u16::try_from(layout::CONTENT_BOTTOM_PADDING).unwrap_or(112))
        } else {
            gap::XL
        };
        (layout::sidebar_width(w), queue, bottom)
    };
    // Pixel sizes are small, exact in f32.
    #[allow(clippy::cast_precision_loss)]
    let (left, right) = (left as f32, right as f32);

    if let Some(toast) = overlays::toast(app) {
        layers.push(
            container(toast)
                .width(Length::Fill)
                .height(Length::Fill)
                .align_y(Alignment::End)
                .center_x(Length::Fill)
                .padding(Padding {
                    top: 0.0,
                    right,
                    bottom: bar_space + gap::XS,
                    left,
                })
                .into(),
        );
    }
    if app.ui.signal_open {
        let (top, bottom) = if now_playing {
            (56.0, 0.0)
        } else {
            (0.0, bar_space - gap::XS)
        };
        layers.push(
            container(overlays::signal_popover(app))
                .width(Length::Fill)
                .height(Length::Fill)
                .align_x(Alignment::End)
                .align_y(if now_playing {
                    Alignment::Start
                } else {
                    Alignment::End
                })
                .padding(Padding {
                    top,
                    right: right + gap::LG,
                    bottom,
                    left: 0.0,
                })
                .into(),
        );
    }
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
    if let Some(track) = &app.ui.menu {
        layers.push(overlays::track_menu(app, track));
    }
    if app.ui.tos_sheet_visible() {
        layers.push(overlays::tos_sheet(app));
    }
    stack(layers).into()
}
