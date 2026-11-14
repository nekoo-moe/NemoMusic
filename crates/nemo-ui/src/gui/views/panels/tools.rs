//! The tools panel: sleep timer, playback speed, lyrics source and window modes.

use std::time::Instant;

use iced::font::Weight;
use iced::widget::{Space, button, column, container, opaque, row};
use iced::{Alignment, Color, Element, Length, Padding};

use crate::explore::{SPEEDS, SleepChoice, timer_left};
use crate::gui::app::{App, Message};
use crate::gui::assets::Icon;
use crate::gui::fx::fx;
use crate::gui::style::{
    animated, button_outlined, col, gap, icon, icon_button, label, label_w, scroll,
};
use crate::gui::views::kit::{paragraph, text_button, vspace};
use crate::gui::views::overlays::card_style;
use crate::layout;
use crate::theme::{color, shape, type_scale};

/// A thin section label.
fn heading(text: &str) -> Element<'_, Message> {
    label_w(
        text,
        type_scale::LABEL_MD,
        Weight::Medium,
        col(color::on_surface_variant()),
    )
    .into()
}

/// A full-width action row: glyph, text, optional right-hand hint.
fn action<'a>(
    glyph: Icon,
    text: &'a str,
    hint: Option<&'a str>,
    message: Message,
) -> Element<'a, Message> {
    let ink = col(color::on_surface());
    let mut r = row![
        icon(glyph, 20.0, ink),
        Space::new().width(gap::MD),
        label(text, type_scale::BODY_MD, false, ink),
        Space::new().width(Length::Fill),
    ]
    .align_y(Alignment::Center)
    .padding(Padding::from([0.0, gap::MD]))
    .height(Length::Fixed(44.0));
    if let Some(h) = hint {
        r = r.push(label(
            h,
            type_scale::LABEL_MD,
            false,
            col(color::on_surface_variant()),
        ));
    }
    animated(
        button(r).padding(0).width(Length::Fill).on_press(message),
        Color::TRANSPARENT,
        shape::md(),
    )
}

/// The tools panel, opened from the capsule (or `S`): sleep timer, playback speed, shortcuts and
/// window modes.
pub fn tools_panel(app: &App) -> Element<'_, Message> {
    let accent = app.accent();
    let dim = col(color::on_surface_variant());
    let w = 360.0;

    // Sleep timer.
    let mut sleep_row = row![].spacing(gap::SM);
    let mut sleep_col = column![].spacing(gap::SM);
    for (i, c) in SleepChoice::ALL.iter().enumerate() {
        let on = app.sleep.as_ref().is_some_and(|s| s.choice == *c);
        let text = c.label();
        // Chips are owned strings; leak-free by building the label inline.
        sleep_row = sleep_row.push(owned_chip(text, on, accent, Message::Sleep(Some(*c))));
        if i == 2 {
            sleep_col = sleep_col.push(sleep_row);
            sleep_row = row![].spacing(gap::SM);
        }
    }
    sleep_col = sleep_col.push(sleep_row);
    let sleep_status: Element<'_, Message> = match &app.sleep {
        Some(s) => {
            let text = match s.deadline {
                Some(d) => format!(
                    "Pauses in {}",
                    timer_left(d.saturating_duration_since(Instant::now()))
                ),
                None => "Pauses when this song ends".to_owned(),
            };
            row![
                label(text, type_scale::BODY_SM, false, col(color::on_surface())),
                Space::new().width(Length::Fill),
                text_button("Turn off", Message::Sleep(None)),
            ]
            .align_y(Alignment::Center)
            .into()
        }
        None => label(
            "Off. Playback pauses by itself when it ends.",
            type_scale::BODY_SM,
            false,
            dim,
        )
        .into(),
    };

    // Playback speed: off by default; chips work only when the parent wired the engine.
    let wired = app.speed_wired();
    let mut speed_row = row![].spacing(gap::SM);
    for s in SPEEDS {
        let on = app.speed_percent == s;
        let text = if s == 100 {
            "Off".to_owned()
        } else {
            format!("{}x", f64::from(s) / 100.0)
        };
        speed_row = speed_row.push(if wired {
            owned_chip(text, on, accent, Message::Speed(s))
        } else {
            disabled_chip(text, on)
        });
    }
    let speed_note = if !wired {
        "Off. A different speed resamples the audio and ends bit-perfect output, so it needs engine support first."
    } else if app.speed_percent == 100 {
        "Off keeps the audio bit-perfect."
    } else {
        "Speed is on: the audio is resampled and is not bit-perfect."
    };

    // Room for the scrollbar at the right.
    let inner = w - 2.0 * gap::LG - gap::MD;
    let body = column![
        row![
            label(
                "Tools",
                type_scale::TITLE_SM,
                true,
                col(color::on_surface())
            ),
            Space::new().width(Length::Fill),
            icon_button(
                Icon::Close,
                18.0,
                32.0,
                dim,
                Color::TRANSPARENT,
                Some(Message::Tools(false)),
            ),
        ]
        .align_y(Alignment::Center),
        vspace(gap::SM),
    ]
    .width(Length::Fixed(inner));
    let lyrics: Element<'_, Message> =
        match crate::gui::views::settings_lyrics::tools_section(app, inner) {
            Some(section) => column![section, vspace(gap::LG)].into(),
            None => Space::new().into(),
        };
    let body = column![
        body,
        lyrics,
        heading("Sleep timer"),
        vspace(gap::SM),
        sleep_col,
        vspace(gap::SM),
        sleep_status,
        vspace(gap::LG),
        heading("Playback speed"),
        vspace(gap::SM),
        speed_row,
        vspace(gap::SM),
        paragraph(speed_note, type_scale::BODY_SM, dim, inner),
        vspace(gap::LG),
        action(
            Icon::FullScreen,
            "Full-screen Now Playing",
            Some("F"),
            Message::ToggleFullScreen
        ),
        action(Icon::Mini, "Mini player", Some("M"), Message::ToggleMini),
        action(
            Icon::Keyboard,
            "Keyboard shortcuts",
            Some("?"),
            Message::Shortcuts(true)
        ),
    ]
    .width(Length::Fixed(inner));
    // The panel never grows past the window: it scrolls above the capsule instead.
    let (_, h) = app.window_px();
    // Window and layout numbers are small, exact in f32.
    #[allow(clippy::cast_precision_loss)]
    let room = h as f32
        - (layout::CAPSULE_HEIGHT + layout::CAPSULE_MARGIN + layout::PANE_GAP) as f32
        - gap::XS
        - gap::XL;
    opaque(
        container(scroll(container(body).padding(Padding {
            right: gap::MD,
            ..Padding::ZERO
        })))
        .padding(gap::LG)
        .width(Length::Fixed(w))
        .max_height(room.max(240.0))
        .style(card_style(shape::xl())),
    )
}

fn owned_chip<'a>(
    text: String,
    on: bool,
    accent: crate::palette::Palette,
    message: Message,
) -> Element<'a, Message> {
    let (fill, ink, outline) = if on {
        (
            col(accent.secondary_container),
            col(accent.on_secondary_container),
            None,
        )
    } else {
        (
            Color::TRANSPARENT,
            col(color::on_surface()),
            Some(col(color::outline())),
        )
    };
    let b = button(
        container(label_w(text, type_scale::LABEL_MD, Weight::Medium, ink))
            .padding(Padding::from([0.0, gap::MD]))
            .height(Length::Fixed(32.0))
            .align_y(Alignment::Center),
    )
    .padding(0)
    .on_press(message);
    fx(b.style(button_outlined(fill, outline, shape::FULL)))
        .layer(ink)
        .radius(shape::FULL)
        .into()
}

fn disabled_chip<'a>(text: String, on: bool) -> Element<'a, Message> {
    let ink = col(color::on_surface().with_alpha(0.38));
    let outline = col(color::outline_variant());
    let fill = if on {
        col(color::on_surface().with_alpha(0.08))
    } else {
        Color::TRANSPARENT
    };
    let b = button(
        container(label_w(text, type_scale::LABEL_MD, Weight::Medium, ink))
            .padding(Padding::from([0.0, gap::MD]))
            .height(Length::Fixed(32.0))
            .align_y(Alignment::Center),
    )
    .padding(0);
    fx(b.style(button_outlined(fill, Some(outline), shape::FULL)))
        .passive()
        .radius(shape::FULL)
        .into()
}
