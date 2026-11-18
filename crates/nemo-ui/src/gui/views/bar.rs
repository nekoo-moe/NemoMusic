//! The floating player capsule (`player-bar-redesign` spec) and the transport controls the other
//! screens share.

use iced::font::Weight;
use iced::widget::{Space, button, container, row, slider, stack};
use iced::{Alignment, Border, Element, Length, Padding};

use super::super::app::{App, Message};
use super::super::fx::fx;
use super::super::style::{col, fit, gap, label, label_w, slider_style};
use crate::format;
use crate::layout::bar as zones;
use crate::layout::{PLAYER_BAR_HEIGHT, PLAYER_BAR_INSET};
use crate::palette::Palette;
use crate::state::Event;
use crate::theme::{color, shape, type_scale};

mod centre;
mod control;
mod cover;
mod left;
mod play;
mod right;
mod surface;

pub use control::{like_button, play_button, repeat_button, transport};
pub use cover::cover_art;

/// A layout number in px. Layout numbers are small, exact in f32.
#[allow(clippy::cast_precision_loss)]
const fn px(n: u32) -> f32 {
    n as f32
}

/// Text for the quality badge, from the real signal path (`AAC 129 kbps · Lossy · Resample`).
/// `Unknown` when there is none; `Buffering` while the player waits for data.
#[must_use]
pub fn badge_text(app: &App) -> String {
    if app.ui.is_buffering() {
        return "Buffering\u{2026}".to_owned();
    }
    crate::signal::badge_label(app.ui.snapshot.signal_path.as_ref())
}

/// The capsule for a window `width` px wide: centred, 88 px high, 12 px above the window
/// bottom. The element is as wide as the window and holds the capsule, so the shell only has to
/// put it at the bottom.
#[must_use]
pub fn view(app: &App, width: f32) -> Element<'_, Message> {
    // A window width is a small positive number.
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    let bar_w = zones::width(width.max(0.0) as u32);
    let z = zones::zones(bar_w);
    let (bar, left_w, centre_w, right_w) = (px(bar_w), px(z.left), px(z.centre), px(z.right));
    let accent = app.accent();
    let content = row![
        container(left::view(app, left_w))
            .width(Length::Fixed(left_w))
            .align_y(Alignment::Center),
        container(centre::view(app, accent, centre_w))
            .width(Length::Fixed(centre_w))
            .align_x(Alignment::Center),
        container(right::view(app, z.step))
            .width(Length::Fixed(right_w))
            .align_y(Alignment::Center),
    ]
    .spacing(px(zones::ZONE_GAP))
    .align_y(Alignment::Center)
    .padding(Padding::from([0.0, px(zones::PADDING)]))
    .width(Length::Fixed(bar))
    .height(Length::Fixed(px(PLAYER_BAR_HEIGHT)));
    let capsule = stack![surface::body(app, bar), content, surface::glint(app, bar)]
        .width(Length::Fixed(bar))
        .height(Length::Fixed(px(PLAYER_BAR_HEIGHT)));
    container(capsule)
        .width(Length::Fill)
        .align_x(Alignment::Center)
        .padding(Padding {
            bottom: px(PLAYER_BAR_INSET),
            ..Padding::ZERO
        })
        .into()
}

/// `0:42 ---o--- 3:12`. Times use tabular figures and a fixed width, so nothing shifts as they
/// tick.
pub fn seek_row(app: &App, accent: Palette) -> Element<'_, Message> {
    let duration = app.ui.snapshot.duration;
    let frac = match (app.seek_drag, duration) {
        (Some(f), _) => f,
        (None, Some(d)) if !d.is_zero() => {
            (app.ui.position.as_secs_f32() / d.as_secs_f32()).clamp(0.0, 1.0)
        }
        _ => 0.0,
    };
    let shown = match (app.seek_drag, duration) {
        (Some(f), Some(d)) => d.mul_f32(f.clamp(0.0, 1.0)),
        _ => app.ui.position,
    };
    let dim = col(color::on_surface_variant());
    let time = |text: String, right: bool| {
        container(label(text, type_scale::LABEL_MD, false, dim))
            .width(Length::Fixed(40.0))
            .align_x(if right {
                Alignment::Start
            } else {
                Alignment::End
            })
    };
    let bar = slider(0.0..=1.0, frac, Message::SeekDrag)
        .step(0.001_f32)
        .on_release(Message::SeekRelease)
        .height(16.0)
        .style(slider_style(
            col(accent.primary),
            col(color::on_surface().with_alpha(0.20)),
            col(accent.primary),
            6.0,
        ));
    row![
        time(format::clock(shown), false),
        Space::new().width(gap::SM),
        bar,
        Space::new().width(gap::SM),
        time(
            duration.map_or_else(|| "-:--".to_owned(), format::clock),
            true
        ),
    ]
    .align_y(Alignment::Center)
    .into()
}

/// Quality badge pill: the real format, quality word and processing, never a guess. Pressing it
/// opens the signal-path popover; lossy never gets a success colour.
pub fn badge(app: &App, max_w: f32) -> Element<'_, Message> {
    let text = fit(&badge_text(app), max_w, type_scale::LABEL_SM.size);
    let b = button(
        container(label_w(
            text,
            type_scale::LABEL_SM,
            Weight::Medium,
            col(color::on_surface_variant()),
        ))
        .padding(Padding::from([4.0, gap::MD])),
    )
    .padding(0)
    .on_press(Message::Ui(Event::ToggleSignalPath));
    fx(b.style(|_, _| button::Style {
        background: None,
        text_color: col(color::on_surface()),
        border: Border {
            color: col(color::outline()),
            width: 1.0,
            radius: shape::FULL.into(),
        },
        shadow: iced::Shadow::default(),
        snap: false,
    }))
    .radius(shape::FULL)
    .into()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::gui::headless::{Scenario, build_app};
    use crate::mock::mock_backend;

    #[test]
    fn a_stream_never_says_lossless_and_buffering_says_so() {
        let mut app = build_app(&mock_backend(), &Scenario::default());
        let text = badge_text(&app);
        assert!(!text.to_lowercase().contains("lossless"), "{text}");
        assert!(text.contains("Lossy") || text == "Unknown", "{text}");
        app.ui.snapshot.state = nemo_core::player::PlayState::Buffering;
        assert_eq!(badge_text(&app), "Buffering\u{2026}");
    }
}
