//! The steps of the setup flow that need no outside service: Welcome, Look, Playback and Done.

use iced::widget::{Space, column, container, row};
use iced::{Alignment, Element, Length};

use super::super::kit::{paragraph, vspace};
use super::super::settings::control::{group, segmented, setting_row, stacked_row};
use super::super::settings::switch_with;
use crate::copy::{self, setup as text};
use crate::gui::app::{App, Message};
use crate::gui::assets::Icon;
use crate::gui::style::{col, gap, icon, label};
use crate::scheme::{AccentIntensity, BaseMode, CornerStyle};
use crate::settings::{NowPlayingStyle, SettingChange};
use crate::setup::summary;
use crate::state::Event;
use crate::theme::{color, shape, type_scale};
use nemo_core::account::AccountState;

/// Side of the mark tile on the first step.
const MARK: f32 = 64.0;

/// A step title with an optional line under it.
pub(super) fn heading<'a>(
    title: &'a str,
    help: Option<&'a str>,
    width: f32,
) -> Element<'a, Message> {
    let mut c = column![label(
        title,
        type_scale::HEADLINE_MD,
        true,
        col(color::on_surface())
    )]
    .spacing(gap::SM)
    .width(Length::Fixed(width));
    if let Some(h) = help {
        c = c.push(paragraph(
            h,
            type_scale::BODY_MD,
            col(color::on_surface_variant()),
            width,
        ));
    }
    c.into()
}

/// Welcome: what NemoMusic is, and that it runs on this computer.
pub(super) fn welcome(app: &App, width: f32) -> Element<'_, Message> {
    let accent = app.accent();
    let mark = container(icon(Icon::Note, 32.0, col(accent.on_primary_container)))
        .width(Length::Fixed(MARK))
        .height(Length::Fixed(MARK))
        .center_x(Length::Fixed(MARK))
        .center_y(Length::Fixed(MARK))
        .style(move |_| container::Style {
            background: Some(col(accent.primary_container).into()),
            border: iced::Border {
                radius: shape::xl().into(),
                ..iced::Border::default()
            },
            ..container::Style::default()
        });
    let mut c = column![
        mark,
        vspace(gap::LG),
        heading(text::WELCOME_TITLE, None, width)
    ]
    .width(Length::Fixed(width));
    for line in text::WELCOME {
        c = c.push(vspace(gap::MD)).push(paragraph(
            line,
            type_scale::BODY_MD,
            col(color::on_surface()),
            width,
        ));
    }
    c.into()
}

/// Look: the surface, accent strength, corners, Now Playing style and reduced motion. Every
/// pick goes through `Event::Setting`, so it applies at once.
pub(super) fn look(app: &App, width: f32) -> Element<'_, Message> {
    let s = &app.ui.settings;
    let rows = vec![
        stacked_row(
            width,
            "Surface",
            None,
            segmented(
                app,
                &[
                    (BaseMode::Dark, "Dark", Some(Icon::DarkMode)),
                    (BaseMode::Amoled, "AMOLED black", Some(Icon::Tune)),
                    (BaseMode::Light, "Light", Some(Icon::LightMode)),
                ],
                s.base,
                SettingChange::Base,
            ),
        ),
        stacked_row(
            width,
            "Accent intensity",
            None,
            segmented(
                app,
                &[
                    (AccentIntensity::Soft, "Soft", None),
                    (AccentIntensity::Standard, "Standard", None),
                    (AccentIntensity::Vivid, "Vivid", None),
                ],
                s.accent_intensity,
                SettingChange::AccentIntensity,
            ),
        ),
        stacked_row(
            width,
            "Corners",
            None,
            segmented(
                app,
                &[
                    (CornerStyle::Sharp, "Sharp", None),
                    (CornerStyle::Soft, "Soft", None),
                    (CornerStyle::Round, "Round", None),
                ],
                s.corner_style,
                SettingChange::CornerStyle,
            ),
        ),
        stacked_row(
            width,
            "Now Playing",
            None,
            segmented(
                app,
                &[
                    (NowPlayingStyle::Classic, "Classic", None),
                    (NowPlayingStyle::M3Expressive, "Material 3 Expressive", None),
                ],
                s.now_playing_style,
                SettingChange::NowPlayingStyle,
            ),
        ),
        setting_row(
            width,
            "Reduce motion",
            None,
            switch_with(s.reduce_motion, |on| {
                Message::Ui(Event::Setting(SettingChange::ReduceMotion(on)))
            }),
        ),
    ];
    column![
        heading(text::LOOK_TITLE, Some(text::LOOK_HELP), width),
        vspace(gap::LG),
        group(rows, width),
    ]
    .width(Length::Fixed(width))
    .into()
}

/// Playback: ad blocking and SponsorBlock, with the wording of `design.md` section 12 and
/// their stored values shown as `On` or `Off`.
pub(super) fn playback(app: &App, width: f32) -> Element<'_, Message> {
    let s = &app.ui.settings;
    let rows = vec![
        setting_row(
            width,
            copy::SPONSORBLOCK_LABEL,
            Some(format!(
                "{}. {}",
                copy::SPONSORBLOCK_SOURCE,
                copy::SPONSORBLOCK_LICENSE
            )),
            switch_with(s.sponsorblock, |on| {
                Message::Ui(Event::Setting(SettingChange::SponsorBlock(on)))
            }),
        ),
        setting_row(
            width,
            copy::ADBLOCK_LABEL,
            Some(copy::ADBLOCK_HELPER.to_owned()),
            switch_with(s.block_ads, |on| {
                Message::Ui(Event::Setting(SettingChange::BlockAds(on)))
            }),
        ),
    ];
    column![
        heading(text::PLAYBACK_TITLE, Some(text::PLAYBACK_HELP), width),
        vspace(gap::LG),
        group(rows, width),
    ]
    .width(Length::Fixed(width))
    .into()
}

/// Done: one line per choice, then `More options in Settings`.
pub(super) fn done(app: &App, width: f32) -> Element<'_, Message> {
    let name = match app.backend.account.state() {
        AccountState::SignedIn(info) => Some(info.name),
        _ => None,
    };
    let lines = summary(
        &app.ui.settings,
        &app.ui.setup,
        name.as_deref(),
        app.music_folders.len(),
    );
    let mut rows: Vec<Element<'_, Message>> = Vec::new();
    for (what, how) in lines {
        rows.push(
            container(
                row![
                    label(
                        what,
                        type_scale::BODY_MD,
                        false,
                        col(color::on_surface_variant())
                    ),
                    Space::new().width(Length::Fill),
                    label(how, type_scale::BODY_MD, true, col(color::on_surface())),
                ]
                .align_y(Alignment::Center),
            )
            .padding(iced::Padding::from([gap::SM, gap::LG]))
            .into(),
        );
    }
    column![
        heading(text::DONE_TITLE, None, width),
        vspace(gap::LG),
        group(rows, width),
        vspace(gap::LG),
        paragraph(
            text::DONE_MORE,
            type_scale::BODY_SM,
            col(color::on_surface_variant()),
            width,
        ),
    ]
    .width(Length::Fixed(width))
    .into()
}
