//! Views. [`root`] picks the three-pane shell or full-window Now Playing and stacks the overlays;
//! the content router in [`shell`] matches on [`Route`], so a new screen adds one arm.

pub mod account;
pub mod backdrop;
pub mod bar;
pub mod detail;
pub mod dev;
pub mod explore;
pub mod home;
pub mod kit;
pub mod library;
pub mod listening_area;
pub mod lyrics;
pub mod now_playing;
pub mod np_classic;
pub mod opt_in;
pub mod overlays;
pub mod pane;
pub mod panels;
pub mod queue;
pub mod sc_account;
pub mod search;
pub mod settings;
pub mod settings_lyrics;
pub mod setup;
pub mod shell;
pub mod splash;

use iced::widget::{container, stack};
use iced::{Alignment, Color, Element, Length, Padding};

use super::app::{App, Message};
use super::fx::Fx;
use super::style::{col, gap, label};
use crate::layout;
use crate::route::Route;
use crate::theme::{color, shape, type_scale};

/// How far the page behind the Now Playing sheet shrinks (design.md section 8).
const SHELL_BACK: f32 = 0.04;
/// Darkness of the scrim over the page at the end of the rise.
const SCRIM_ALPHA: f32 = 0.45;

/// The whole window.
pub fn root(app: &App) -> Element<'_, Message> {
    if app.mini {
        return panels::mini_player(app);
    }
    let (w, h) = app.window_px();
    let is_np = matches!(app.ui.route(), Route::NowPlaying);
    let np_p = app.anim.now_playing.value.clamp(0.0, 1.0);
    let page: Element<'_, Message> = if np_p <= 0.001 {
        shell::view(app, w, h)
    } else if np_p >= 0.999 {
        now_playing::view(app, w, h)
    } else {
        // The page steps back and dims while the sheet rises over it.
        #[allow(clippy::cast_precision_loss)]
        let h_px = h as f32; // window height is small, exact in f32
        let shell_el = Fx::new(shell::view(app, w, h))
            .passive()
            .scale(1.0 - SHELL_BACK * np_p);
        let scrim = container(iced::widget::Space::new())
            .width(Length::Fill)
            .height(Length::Fill)
            .style(move |_| container::Style {
                background: Some(
                    Color {
                        a: SCRIM_ALPHA * np_p,
                        ..col(color::SCRIM)
                    }
                    .into(),
                ),
                ..container::Style::default()
            });
        // Full travel: the sheet starts below the window, so it never "pops" in.
        let mut sheet =
            Fx::new(now_playing::view(app, w, h)).entrance(np_p, h_px, Color::TRANSPARENT);
        if !is_np {
            sheet = sheet.passive();
        }
        stack![shell_el, scrim, sheet]
            .width(Length::Fill)
            .height(Length::Fill)
            .into()
    };
    let now_playing = is_np;
    let mut layers: Vec<Element<'_, Message>> = vec![
        backdrop::window_field(app),
        container(page)
            .width(Length::Fill)
            .height(Length::Fill)
            .into(),
    ];

    // Space below the notices, so they sit above the capsule and not under it, and the right
    // edge the popovers opened from the capsule line up with (its inner right end).
    let capsule_shows = !now_playing && app.ui.snapshot.current.is_some();
    // Layout numbers are small, exact in f32.
    #[allow(clippy::cast_precision_loss)]
    let (bar_space, bar_right) = if capsule_shows {
        let side = (w.saturating_sub(layout::bar::width(w)) / 2 + layout::bar::PADDING) as f32;
        (
            (layout::CAPSULE_HEIGHT + layout::CAPSULE_MARGIN + layout::PANE_GAP) as f32,
            side,
        )
    } else {
        (gap::XL, gap::XL)
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
    if app.surfaces.signal.visible() {
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
                    right: bar_right,
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
            col(color::on_surface()),
        ))
        .padding(Padding::from([gap::XS, gap::SM]))
        .style(|_| container::Style {
            background: Some(col(color::surface_container_high()).into()),
            border: iced::Border {
                radius: shape::sm().into(),
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
    if let Some(track) = app.surfaces.menu_shown() {
        layers.push(overlays::track_menu(app, track));
    }
    if app.surfaces.tools.visible() {
        layers.push(
            container(panels::tools_panel(app))
                .width(Length::Fill)
                .height(Length::Fill)
                .align_x(Alignment::End)
                .align_y(Alignment::End)
                .padding(Padding {
                    right: bar_right,
                    bottom: bar_space + gap::XS,
                    ..Padding::ZERO
                })
                .into(),
        );
    }
    if app.surfaces.create.visible() {
        layers.push(panels::create_sheet(app));
    }
    if app.surfaces.shortcuts.visible() {
        layers.push(panels::shortcuts_sheet(app));
    }
    if app.surfaces.dev.visible() {
        layers.push(dev::panel(app));
    }
    if app.ui.setup.active() {
        layers.push(setup::view(app));
    }
    if app.account.open {
        layers.push(account::sheet(app));
    }
    if app.surfaces.tos.visible() {
        layers.push(overlays::tos_sheet(app));
    }
    stack(layers).into()
}
