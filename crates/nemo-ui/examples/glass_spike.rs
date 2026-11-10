//! Glass player bar spike (Tier Low look, design.md sections 6, 7 and 14).
//!
//! Run: `cargo run -p nemo-ui --example glass_spike --features gui`
//!
//! Draws a generated cover-like gradient, a floating 88 px player bar inset 12 px, and a
//! readout of frame interval over the last second. The readout is the time between
//! redraws (`window::frames`), not GPU time. Release builds only give meaningful numbers.
#![allow(clippy::print_stdout, reason = "example")]

use std::collections::VecDeque;
use std::time::{Duration, Instant};

use iced::widget::{button, column, container, row, slider, stack, text};
use iced::{
    Background, Border, Color, Element, Fill, Gradient, Radians, Shadow, Subscription, Theme,
    Vector, border, gradient, window,
};
use nemo_ui::theme::{self, Rgba, color, contrast_ratio, glass, layout, shape, spacing};

/// Generated "cover" colors: (offset, hex). Deliberately bright and saturated.
const COVER_STOPS: [(f32, u32); 4] = [
    (0.0, 0xFF_B300),
    (0.35, 0xE9_3F8E),
    (0.7, 0x2F_9BFF),
    (1.0, 0x3B_E3A0),
];
/// Song length shown on the seek bar.
const TRACK_SECONDS: f32 = 215.0;

struct Spike {
    playing: bool,
    position: f32,
    last_frame: Option<Instant>,
    /// (time of frame, interval since previous frame)
    frames: VecDeque<(Instant, Duration)>,
    /// Tint alpha chosen so `on-surface` passes 4.5:1 over the brightest cover stop.
    tint_alpha: f32,
}

#[derive(Debug, Clone, Copy)]
enum Message {
    Frame(Instant),
    Toggle,
    Seek(f32),
}

fn to_iced(c: Rgba) -> Color {
    Color::from_rgba8(c.r, c.g, c.b, c.alpha())
}

/// Legibility rule from design.md section 6: raise tint alpha in 0.05 steps up to 0.90
/// until `on-surface` is >= 4.5:1 over tint + the worst (lightest) backdrop color.
fn legible_tint_alpha() -> f32 {
    let worst = COVER_STOPS
        .iter()
        .map(|&(_, hex)| Rgba::from_rgb_hex(hex))
        .max_by(|a, b| a.relative_luminance().total_cmp(&b.relative_luminance()))
        .unwrap_or(color::SURFACE);
    let mut alpha = glass::TINT_ALPHA;
    while alpha < glass::TINT_ALPHA_MAX {
        let bar = glass::TINT_BASE.with_alpha(alpha).over(worst);
        if contrast_ratio(color::ON_SURFACE, bar) >= 4.5 {
            break;
        }
        alpha += glass::TINT_ALPHA_STEP;
    }
    alpha.min(glass::TINT_ALPHA_MAX)
}

impl Spike {
    fn new() -> Self {
        Self {
            playing: true,
            position: 42.0,
            last_frame: None,
            frames: VecDeque::new(),
            tint_alpha: legible_tint_alpha(),
        }
    }

    fn update(&mut self, message: Message) {
        match message {
            Message::Frame(now) => {
                if let Some(prev) = self.last_frame {
                    let dt = now.saturating_duration_since(prev);
                    self.frames.push_back((now, dt));
                    if self.playing {
                        self.position = (self.position + dt.as_secs_f32()) % TRACK_SECONDS;
                    }
                }
                self.last_frame = Some(now);
                while let Some(&(t, _)) = self.frames.front() {
                    if now.saturating_duration_since(t) > Duration::from_secs(1) {
                        self.frames.pop_front();
                    } else {
                        break;
                    }
                }
            }
            Message::Toggle => self.playing = !self.playing,
            Message::Seek(p) => self.position = p,
        }
    }

    fn stats(&self) -> (f64, f64, usize) {
        let n = self.frames.len();
        if n == 0 {
            return (0.0, 0.0, 0);
        }
        let ms: Vec<f64> = self
            .frames
            .iter()
            .map(|(_, d)| d.as_secs_f64() * 1000.0)
            .collect();
        #[allow(clippy::cast_precision_loss, reason = "n is at most a few hundred")]
        let avg = ms.iter().sum::<f64>() / n as f64;
        let worst = ms.iter().copied().fold(0.0_f64, f64::max);
        (avg, worst, n)
    }

    #[allow(
        clippy::unused_self,
        reason = "signature required by iced::Application::subscription"
    )]
    fn subscription(&self) -> Subscription<Message> {
        window::frames().map(Message::Frame)
    }

    fn view(&self) -> Element<'_, Message> {
        stack![Self::cover(), self.bar_layer(), self.readout_layer()].into()
    }

    /// Generated backdrop: a diagonal four-stop gradient plus a translucent cross gradient.
    fn cover() -> Element<'static, Message> {
        let mut base = gradient::Linear::new(Radians(0.9));
        for (off, hex) in COVER_STOPS {
            base = base.add_stop(off, to_iced(Rgba::from_rgb_hex(hex)));
        }
        let cross = gradient::Linear::new(Radians(2.4))
            .add_stop(0.0, Color::from_rgba8(255, 255, 255, 0.0))
            .add_stop(0.5, Color::from_rgba8(120, 0, 200, 0.45))
            .add_stop(1.0, Color::from_rgba8(255, 255, 255, 0.0));
        stack![
            container("")
                .width(Fill)
                .height(Fill)
                .style(move |_| container::Style::default()
                    .background(Background::Gradient(Gradient::Linear(base)))),
            container("")
                .width(Fill)
                .height(Fill)
                .style(move |_| container::Style::default()
                    .background(Background::Gradient(Gradient::Linear(cross)))),
        ]
        .into()
    }

    fn bar_layer(&self) -> Element<'_, Message> {
        let label = if self.playing { "Pause" } else { "Play" };
        let on_surface = to_iced(color::ON_SURFACE);

        let play = button(
            text(label)
                .size(f32::from(theme::type_scale::BODY_MD.size))
                .color(on_surface),
        )
        .on_press(Message::Toggle)
        .padding([spacing::SM, spacing::LG])
        .style(move |_: &Theme, _| button::Style {
            background: Some(Background::Color(to_iced(
                color::ON_SURFACE.with_alpha(0.12),
            ))),
            text_color: on_surface,
            border: border::rounded(shape::FULL),
            shadow: Shadow::default(),
            snap: false,
        });

        let seek = slider(0.0..=TRACK_SECONDS, self.position, Message::Seek)
            .step(0.5_f32)
            .style(move |_: &Theme, _| slider::Style {
                rail: slider::Rail {
                    backgrounds: (
                        Background::Color(on_surface),
                        Background::Color(to_iced(color::ON_SURFACE.with_alpha(0.38))),
                    ),
                    width: 4.0,
                    border: border::rounded(shape::FULL),
                },
                handle: slider::Handle {
                    shape: slider::HandleShape::Circle { radius: 8.0 },
                    background: Background::Color(on_surface),
                    border_width: 0.0,
                    border_color: Color::TRANSPARENT,
                },
            });

        let time = text(format!(
            "{} / {}",
            fmt_time(self.position),
            fmt_time(TRACK_SECONDS)
        ))
        .size(f32::from(theme::type_scale::BODY_SM.size))
        .color(on_surface);

        let content = row![play, seek, time]
            .spacing(f32::from(spacing::LG))
            .align_y(iced::Center);

        let tint = to_iced(glass::TINT_BASE.with_alpha(self.tint_alpha));
        let edge = to_iced(glass::EDGE.with_alpha(glass::EDGE_ALPHA));
        let shadow_color = to_iced(glass::SHADOW);
        let bar = container(content)
            .padding([0, spacing::XL])
            .center_y(Fill)
            .width(Fill)
            .height(layout::PLAYER_BAR_HEIGHT)
            .style(move |_| container::Style {
                text_color: Some(on_surface),
                background: Some(Background::Color(tint)),
                border: Border {
                    color: edge,
                    width: 1.0,
                    radius: glass::RADIUS.into(),
                },
                shadow: Shadow {
                    color: shadow_color,
                    offset: Vector::new(0.0, glass::SHADOW_OFFSET_Y),
                    blur_radius: glass::SHADOW_BLUR,
                },
                snap: false,
            });

        container(bar)
            .padding(layout::PLAYER_BAR_INSET)
            .align_bottom(Fill)
            .width(Fill)
            .height(Fill)
            .into()
    }

    /// Readout sits on an opaque `surface-container-high` plate, so it is always readable.
    fn readout_layer(&self) -> Element<'_, Message> {
        let (avg, worst, n) = self.stats();
        let fps = if avg > 0.0 { 1000.0 / avg } else { 0.0 };
        let verdict = if n == 0 {
            "waiting for frames"
        } else if worst <= theme::motion::FRAME_BUDGET_MS {
            "within 16.6 ms budget"
        } else {
            "over 16.6 ms budget"
        };
        let plate = container(
            column![
                text(format!("avg {avg:.1} ms ({fps:.0} fps)"))
                    .size(f32::from(theme::type_scale::BODY_MD.size))
                    .color(to_iced(color::ON_SURFACE)),
                text(format!("worst {worst:.1} ms, {n} frames in last 1 s"))
                    .size(f32::from(theme::type_scale::BODY_MD.size))
                    .color(to_iced(color::ON_SURFACE)),
                text(verdict)
                    .size(f32::from(theme::type_scale::BODY_SM.size))
                    .color(to_iced(color::ON_SURFACE_VARIANT)),
            ]
            .spacing(f32::from(spacing::XS)),
        )
        .padding(f32::from(spacing::MD))
        .style(|_| {
            container::Style::default()
                .background(Background::Color(to_iced(color::SURFACE_CONTAINER_HIGH)))
                .border(border::rounded(shape::MD))
        });
        container(plate).padding(f32::from(spacing::XL)).into()
    }
}

fn fmt_time(seconds: f32) -> String {
    // Display only; fractional seconds are dropped on purpose.
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    let s = seconds.max(0.0) as u32;
    format!("{}:{:02}", s / 60, s % 60)
}

fn main() -> iced::Result {
    iced::application(Spike::new, Spike::update, Spike::view)
        .subscription(Spike::subscription)
        .theme(Theme::Dark)
        .title("NemoMusic glass spike")
        .window_size(layout::DEFAULT_WINDOW)
        .run()
}
