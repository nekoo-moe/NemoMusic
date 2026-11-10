//! Views. [`root`] picks the three-pane shell or full-window Now Playing and stacks the overlays;
//! the content router in [`shell`] matches on [`Route`], so a new screen adds one arm.

pub mod account;
pub mod bar;
pub mod detail;
pub mod home;
pub mod kit;
pub mod library;
pub mod now_playing;
pub mod overlays;
pub mod pane;
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
        fill(page, color::SURFACE_CONTAINER_LOWEST)
            .width(Length::Fill)
            .height(Length::Fill)
            .into(),
    ];

    // Space below the notices, so they sit above the capsule and not under it.
    let bar_space = if now_playing || app.ui.snapshot.current.is_none() {
        gap::XL
    } else {
        // Capsule 72 + margin 16 + a little air.
        // Layout numbers are small, exact in f32.
        #[allow(clippy::cast_precision_loss)]
        let space = (layout::CAPSULE_HEIGHT + layout::CAPSULE_MARGIN + layout::PANE_GAP) as f32;
        space
    };

    if let Some(toast) = overlays::toast(app) {
        layers.push(
            container(toast)
                .width(Length::Fill)
                .height(Length::Fill)
                .align_y(Alignment::End)
                .center_x(Length::Fill)
                .padding(Padding {
                    bottom: bar_space + gap::XS,
                    ..Padding::ZERO
                })
                .into(),
        );
    }
    if app.ui.signal_open {
        let (top, bottom) = if now_playing {
            (56.0, 0.0)
        } else {
            (0.0, bar_space + gap::XS)
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
                    right: gap::XL,
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
    if app.account.open {
        layers.push(account::sheet(app));
    }
    if app.ui.tos_sheet_visible() {
        layers.push(overlays::tos_sheet(app));
    }
    stack(layers).into()
}
