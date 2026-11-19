//! Settings (`ui-reference.md` section 4): a 200 px section list on the left, grouped rows on the
//! right. Changes go through the reducer, which tells the app (`on_settings`).

use iced::widget::{Space, button, column, container, row, toggler};
use iced::{Alignment, Background, Color, Element, Length, Padding};

use super::super::app::{App, Message, SettingsSection};
use super::super::style::{animated, col, gap, label};
use super::kit::{page, page_title, vspace};
use crate::copy;
use crate::layout;
use crate::settings::SettingChange;
use crate::state::Event;
use crate::theme::{color, shape, type_scale};

mod appearance;
pub(super) mod control;
mod info;
mod playback;

use control::group;
use info::{about, privacy};
use playback::{audio_output, library, lyrics, playback};

/// Opacity of the content of a disabled control (`design.md` section 5).
const DISABLED_ALPHA: f32 = 0.38;
/// Width of the section list.
const MENU_W: f32 = 200.0;
/// Widest right column.
const RIGHT_MAX: f32 = 720.0;

/// The Settings screen.
pub fn view(app: &App, w: u32) -> Element<'_, Message> {
    let grid = layout::grid_in(w);
    // Content widths are small exact numbers.
    #[allow(clippy::cast_precision_loss)]
    let content_w = grid.content as f32;
    let right_w = (content_w - MENU_W - gap::XL).clamp(320.0, RIGHT_MAX);
    let section = app.settings_section;

    let mut menu = column![].spacing(gap::XS).width(Length::Fixed(MENU_W));
    for s in SettingsSection::ALL {
        let on = s == section;
        let idle = if on {
            col(color::surface_container_high())
        } else {
            Color::TRANSPARENT
        };
        menu = menu.push(animated(
            button(
                container(label(
                    s.label(),
                    type_scale::BODY_MD,
                    on,
                    col(if on {
                        color::on_surface()
                    } else {
                        color::on_surface_variant()
                    }),
                ))
                .height(Length::Fixed(40.0))
                .width(Length::Fill)
                .padding(Padding::from([0.0, gap::MD]))
                .align_y(Alignment::Center),
            )
            .padding(0)
            .width(Length::Fill)
            .on_press(Message::SettingsSection(s)),
            idle,
            shape::md(),
        ));
    }

    let body: Element<'_, Message> = match section {
        SettingsSection::Playback => group(playback(app, right_w), right_w),
        SettingsSection::AudioOutput => group(audio_output(app, right_w), right_w),
        SettingsSection::Lyrics => group(lyrics(app, right_w), right_w),
        SettingsSection::Library => group(library(app, right_w), right_w),
        // Appearance is its own cards under a live preview.
        SettingsSection::Appearance => appearance::page(app, right_w),
        SettingsSection::Privacy => group(privacy(right_w), right_w),
        SettingsSection::About => group(about(right_w), right_w),
    };
    let right =
        column![page_title(section.label()), vspace(gap::LG), body].width(Length::Fixed(right_w));
    page(
        grid.content,
        row![menu, Space::new().width(gap::XL), right].align_y(Alignment::Start),
    )
}

/// `On` / `Off` text next to a switch (state is never colour alone).
fn switch<'a>(on: bool, change: fn(bool) -> SettingChange) -> Element<'a, Message> {
    switch_with(on, move |v| Message::Ui(Event::Setting(change(v))))
}

/// Like [`switch`], with any message.
pub(super) fn switch_with<'a>(
    on: bool,
    msg: impl Fn(bool) -> Message + 'a,
) -> Element<'a, Message> {
    row![
        label(
            if on { "On" } else { "Off" },
            type_scale::BODY_MD,
            false,
            col(color::on_surface_variant()),
        ),
        Space::new().width(gap::MD),
        toggler(on)
            .on_toggle(msg)
            .size(24.0)
            .style(switch_style(on)),
    ]
    .align_y(Alignment::Center)
    .into()
}

/// The two song-colour switches (song-colour-theme spec): the song colours, and the heartbeat,
/// greyed out while the colours are off or motion is reduced. Shared by Settings and the setup
/// flow, with the same copy.
pub(super) fn song_colour_rows(app: &App, width: f32) -> Vec<Element<'_, Message>> {
    let s = &app.ui.settings;
    vec![
        control::setting_row(
            width,
            copy::SONG_COLOUR_LABEL,
            Some(copy::SONG_COLOUR_HELPER.to_owned()),
            switch(s.song_colour_theme, SettingChange::SongColourTheme),
        ),
        control::setting_row(
            width,
            copy::HEARTBEAT_LABEL,
            Some(copy::HEARTBEAT_HELPER.to_owned()),
            if s.heartbeat_available(app.ui.reduced_motion) {
                switch(s.song_heartbeat, SettingChange::SongHeartbeat)
            } else {
                switch_disabled(s.song_heartbeat)
            },
        ),
    ]
}

/// A switch that cannot be used now (greyed out at the disabled opacity of `design.md` section
/// 5). The state still shows as `On` or `Off` text, and nothing is sent when it is pressed.
pub(super) fn switch_disabled<'a>(on: bool) -> Element<'a, Message> {
    let dim = |c: crate::theme::Rgba| col(c.with_alpha(DISABLED_ALPHA));
    row![
        label(
            if on { "On" } else { "Off" },
            type_scale::BODY_MD,
            false,
            dim(color::on_surface_variant()),
        ),
        Space::new().width(gap::MD),
        toggler(on).size(24.0).style(move |_, _| toggler::Style {
            background: Background::Color(dim(color::on_surface())),
            background_border_width: 0.0,
            background_border_color: Color::TRANSPARENT,
            foreground: Background::Color(dim(color::surface())),
            foreground_border_width: 0.0,
            foreground_border_color: Color::TRANSPARENT,
            text_color: None,
            border_radius: None,
            padding_ratio: 0.2,
        }),
    ]
    .align_y(Alignment::Center)
    .into()
}

/// The style of every settings switch: `on-surface` track when on, a quiet track when off.
pub(super) fn switch_style(on: bool) -> impl Fn(&iced::Theme, toggler::Status) -> toggler::Style {
    move |_, status| {
        let hovered = matches!(status, toggler::Status::Hovered { .. });
        toggler::Style {
            background: Background::Color(if on {
                col(color::on_surface())
            } else if hovered {
                col(color::on_surface().with_alpha(0.30))
            } else {
                col(color::on_surface().with_alpha(0.24))
            }),
            background_border_width: 0.0,
            background_border_color: Color::TRANSPARENT,
            foreground: Background::Color(if on {
                col(color::surface())
            } else {
                col(color::on_surface_variant())
            }),
            foreground_border_width: 0.0,
            foreground_border_color: Color::TRANSPARENT,
            text_color: None,
            border_radius: None,
            padding_ratio: 0.2,
        }
    }
}
