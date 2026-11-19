//! The start-up splash: the brand intro (brand-intro spec, task 1) over the real start-up steps.
//!
//! The mark, the wordmark and the tagline are drawn from [`Frame`], a pure function of the time
//! since the first frame; the thin progress line and the name of the step running stay at the
//! bottom. It draws on the neutral baseline of the saved base mode and the idle accent, never on
//! a song colour. `opacity` fades every part of it together, so the fade into the app needs no
//! group opacity from the renderer.

use iced::widget::image::Handle;
use iced::widget::{Space, button, column, container, image, row, stack, text};
use iced::{Alignment, Background, Border, Color, Element, Length, Padding, Pixels};

use super::super::assets::{Icon, display_font};
use super::super::style::{animated, col, gap, icon, label};
use crate::palette::Palette;
use crate::splash::intro::{
    BLOOM_SIZE, Frame, GAP_MARK, GAP_TAGLINE, GLYPH_SIZE, MARK_SIZE, Reveal, SIGNATURE_H, TAGLINE,
    TAGLINE_BOX, WORD_BOX, WORDMARK,
};
use crate::theme::{Rgba, color, shape, type_scale};

/// Width of the progress line.
const LINE_W: f32 = 240.0;
/// Height of the progress line (the 3 px rail of `design.md` section 10).
const LINE_H: f32 = 3.0;
/// Height of the Close button.
const BUTTON_H: f32 = 40.0;
/// Space between the progress line and the bottom edge of the window.
const BOTTOM: f32 = 48.0;
/// How far the mark shrinks while the splash leaves (share of its size).
const EXIT_SHRINK: f32 = 0.1;

/// What the splash draws at one moment.
#[derive(Debug, Clone, Copy)]
pub struct SplashLook<'a> {
    /// Finished share of the start-up steps, 0 to 1 (already eased by the caller).
    pub fraction: f32,
    /// The step running, or the plain sentence of a failure.
    pub status: &'a str,
    /// Opacity of the whole splash, 0 to 1.
    pub opacity: f32,
    /// The idle accent (not a song colour).
    pub accent: Palette,
    /// The start failed: the status is a sentence and the Close button shows.
    pub failed: bool,
    /// Milliseconds since the first frame: which moment of the intro to draw.
    pub t_ms: u64,
    /// Reduce motion is on: draw the intro at rest.
    pub reduced: bool,
    /// The accent bloom, built once from [`crate::splash::intro::bloom_rgba`].
    pub bloom: &'a Handle,
}

fn tint(c: Rgba, opacity: f32) -> Color {
    let mut out = col(c);
    out.a *= opacity;
    out
}

/// The splash as a full-window layer. `on_close` is the message of the Close button, drawn only
/// when the start failed.
pub fn view<'a, M: Clone + 'a>(look: &SplashLook<'_>, on_close: M) -> Element<'a, M> {
    let o = look.opacity.clamp(0.0, 1.0);
    // A failed start shows the finished signature and moves nothing.
    let at_rest = look.failed || look.reduced;
    let mut frame = if at_rest {
        Frame::REST
    } else {
        Frame::at(look.t_ms, false)
    };
    if look.failed {
        frame.tagline = Reveal {
            opacity: 0.0,
            ..Reveal::REST
        };
    }
    frame.mark_scale *= 1.0 - EXIT_SHRINK * (1.0 - o);

    let bloom = bloom_layer(look.bloom, &frame, o);
    let signature = container(
        column![
            mark(look.accent, &frame, o),
            Space::new().height(GAP_MARK),
            wordmark(look.t_ms, at_rest, o),
            Space::new().height(GAP_TAGLINE),
            tagline(frame.tagline, o),
        ]
        .align_x(Alignment::Center),
    )
    .width(Length::Fill)
    .height(Length::Fill)
    .center_x(Length::Fill)
    .center_y(Length::Fill);
    let bottom = bottom_layer(look, &frame, o, on_close);
    let background = container(Space::new())
        .width(Length::Fill)
        .height(Length::Fill)
        .style(move |_| container::Style {
            background: Some(Background::Color(tint(color::surface(), o))),
            ..container::Style::default()
        });
    stack![background, bloom, signature, bottom].into()
}

/// The accent bloom, centred on the mark and growing and breathing with `frame`.
fn bloom_layer<'a, M: 'a>(handle: &Handle, frame: &Frame, o: f32) -> Element<'a, M> {
    let size = BLOOM_SIZE * frame.bloom_scale;
    // The signature is centred in the window; the mark is its top, so the bloom sits that far
    // above the middle.
    let lift = (SIGNATURE_H - MARK_SIZE) / 2.0;
    container(
        image(handle.clone())
            .width(Length::Fixed(size))
            .height(Length::Fixed(size))
            .opacity(frame.bloom_opacity * o),
    )
    .width(Length::Fill)
    .height(Length::Fill)
    .center_x(Length::Fill)
    .center_y(Length::Fill)
    .padding(Padding {
        bottom: 2.0 * lift,
        ..Padding::ZERO
    })
    .into()
}

/// The mark: a disc in the accent with the note rising into it.
fn mark<'a, M: 'a>(accent: Palette, frame: &Frame, o: f32) -> Element<'a, M> {
    let scale = frame.mark_scale.max(0.0);
    let side = MARK_SIZE * scale;
    let fill = tint(accent.primary, o * frame.mark_opacity);
    let ink = tint(accent.on_primary, o * frame.glyph.opacity);
    let disc = container(
        container(icon(Icon::Note, GLYPH_SIZE * scale, ink))
            .width(Length::Fill)
            .height(Length::Fill)
            .center_x(Length::Fill)
            .center_y(Length::Fill)
            .padding(Padding {
                top: 2.0 * frame.glyph.rise_px * scale,
                ..Padding::ZERO
            }),
    )
    .width(Length::Fixed(side))
    .height(Length::Fixed(side))
    .style(move |_| container::Style {
        background: Some(Background::Color(fill)),
        border: Border {
            radius: shape::FULL.into(),
            ..Border::default()
        },
        ..container::Style::default()
    });
    // The slot keeps its size while the disc scales, so nothing around it moves.
    container(disc)
        .width(Length::Fixed(MARK_SIZE))
        .height(Length::Fixed(MARK_SIZE))
        .center_x(Length::Fixed(MARK_SIZE))
        .center_y(Length::Fixed(MARK_SIZE))
        .into()
}

/// The wordmark in `font.display` 600, one letter at a time.
fn wordmark<'a, M: 'a>(t_ms: u64, at_rest: bool, o: f32) -> Element<'a, M> {
    let style = type_scale::DISPLAY_MD;
    let mut letters = row![];
    for (i, ch) in WORDMARK.chars().enumerate() {
        let r = Frame::letter(i, t_ms, at_rest);
        let glyph = text(ch.to_string())
            .size(Pixels(f32::from(style.size)))
            .line_height(text::LineHeight::Absolute(Pixels(f32::from(style.line))))
            .font(display_font(true))
            .color(tint(color::on_surface(), o * r.opacity))
            .wrapping(text::Wrapping::None);
        letters = letters.push(rise_box(glyph.into(), WORD_BOX, r.rise_px));
    }
    letters.into()
}

/// The line under the wordmark.
fn tagline<'a, M: 'a>(r: Reveal, o: f32) -> Element<'a, M> {
    let line = label(
        TAGLINE,
        type_scale::BODY_MD,
        false,
        tint(color::on_surface_variant(), o * r.opacity),
    );
    rise_box(line.into(), TAGLINE_BOX, r.rise_px)
}

/// A box of fixed `height` with `content` `rise` px below its top, so a rising item never moves
/// what is around it.
fn rise_box<'a, M: 'a>(content: Element<'a, M>, height: f32, rise: f32) -> Element<'a, M> {
    container(column![Space::new().height(rise), content])
        .height(Length::Fixed(height))
        .into()
}

/// The progress line with the step running, or the failure sentence with its Close button.
fn bottom_layer<'a, M: Clone + 'a>(
    look: &SplashLook<'_>,
    frame: &Frame,
    o: f32,
    on_close: M,
) -> Element<'a, M> {
    let mut body = column![].align_x(Alignment::Center).spacing(gap::SM);
    if look.failed {
        body = body.push(
            label(
                look.status.to_owned(),
                type_scale::BODY_MD,
                false,
                tint(color::on_surface(), o),
            )
            .wrapping(iced::widget::text::Wrapping::Word)
            .width(Length::Fixed(LINE_W + 2.0 * gap::XXL))
            .align_x(iced::alignment::Horizontal::Center),
        );
        body = body.push(close_button(look.accent, o, on_close));
    } else {
        let a = o * frame.rail_opacity;
        body = body.push(progress_line(look.fraction, look.accent.primary, a));
        body = body.push(label(
            look.status.to_owned(),
            type_scale::LABEL_SM,
            false,
            tint(color::on_surface_variant(), a),
        ));
    }
    container(body)
        .width(Length::Fill)
        .height(Length::Fill)
        .center_x(Length::Fill)
        .align_y(Alignment::End)
        .padding(Padding {
            bottom: BOTTOM,
            ..Padding::ZERO
        })
        .into()
}

/// The thin line: a rail with the finished part filled in the accent.
fn progress_line<'a, M: 'a>(fraction: f32, fill: Rgba, o: f32) -> Element<'a, M> {
    let filled = LINE_W * fraction.clamp(0.0, 1.0);
    let rail = container(
        container(iced::widget::Space::new())
            .width(Length::Fixed(filled))
            .height(Length::Fixed(LINE_H))
            .style(move |_| container::Style {
                background: Some(Background::Color(tint(fill, o))),
                border: Border {
                    radius: shape::FULL.into(),
                    ..Border::default()
                },
                ..container::Style::default()
            }),
    )
    .width(Length::Fixed(LINE_W))
    .height(Length::Fixed(LINE_H))
    .style(move |_| container::Style {
        background: Some(Background::Color(tint(
            color::on_surface().with_alpha(0.12),
            o,
        ))),
        border: Border {
            radius: shape::FULL.into(),
            ..Border::default()
        },
        ..container::Style::default()
    });
    container(rail)
        .padding(Padding::from([gap::XS, 0.0]))
        .into()
}

/// A tonal Close button for the failed start.
fn close_button<'a, M: Clone + 'a>(accent: Palette, o: f32, on_close: M) -> Element<'a, M> {
    let ink = tint(accent.on_primary_container, o);
    let b = button(
        container(label("Close", type_scale::BODY_MD, true, ink))
            .padding(Padding::from([0.0, gap::XL]))
            .height(Length::Fixed(BUTTON_H))
            .align_y(Alignment::Center),
    )
    .padding(0)
    .on_press(on_close);
    animated(b, tint(accent.primary_container, o), shape::FULL)
}
