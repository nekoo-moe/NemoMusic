//! The steps of the setup flow that need no outside service: Welcome, Look, Playback and Done.

use iced::widget::{Space, column, container, row};
use iced::{Alignment, Element, Length};

use super::super::kit::{paragraph, vspace};
use super::super::settings::control::{segmented, setting_row};
use super::super::settings::{song_colour_rows, switch_with};
use super::panel::display;
use crate::copy::{self, setup as text};
use crate::gui::app::{App, Message};
use crate::gui::assets::Icon;
use crate::gui::style::{col, gap, icon, label};
use crate::scheme::{AccentIntensity, BaseMode, CornerStyle};
use crate::settings::{NowPlayingStyle, SettingChange};
use crate::setup::summary;
use crate::state::Event;
use crate::theme::{color, type_scale};

/// Body width from which the Look step has two columns.
const TWO_COLUMNS_AT: f32 = 740.0;
use nemo_core::account::AccountState;

/// A step's headline in `font.display` with one sentence under it.
pub(super) fn heading<'a>(
    title: &'a str,
    help: Option<&'a str>,
    width: f32,
) -> Element<'a, Message> {
    let mut c = column![
        display(title, type_scale::HEADLINE_MD, col(color::on_surface())).width(Length::Fill)
    ]
    .spacing(gap::XS)
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

/// Two columns side by side, each half of `width` less the gap between them.
pub(super) fn two_columns<'a>(
    left: Vec<Element<'a, Message>>,
    right: Vec<Element<'a, Message>>,
    width: f32,
) -> Element<'a, Message> {
    let half = ((width - gap::LG) / 2.0).floor();
    row![
        column(left).width(Length::Fixed(half)),
        Space::new().width(gap::LG),
        column(right).width(Length::Fixed(half)),
    ]
    .align_y(Alignment::Start)
    .into()
}

/// Whether a step has room for two columns at this body width.
pub(super) fn has_two_columns(width: f32) -> bool {
    width >= TWO_COLUMNS_AT
}

/// Welcome: what NemoMusic is, then two short facts: it runs on this computer, and how long the
/// setup takes.
pub(super) fn welcome(app: &App, width: f32) -> Element<'_, Message> {
    let accent = app.accent();
    let fact = |glyph: Icon, title: &'static str, line: &'static str| {
        row![
            icon(glyph, 24.0, col(accent.primary)),
            Space::new().width(gap::MD),
            column![
                label(title, type_scale::BODY_MD, true, col(color::on_surface())),
                paragraph(
                    line,
                    type_scale::BODY_MD,
                    col(color::on_surface_variant()),
                    width - 24.0 - gap::MD,
                ),
            ]
            .spacing(gap::XS),
        ]
        .align_y(Alignment::Start)
    };
    column![
        heading(text::WELCOME_TITLE, Some(text::WELCOME[0]), width),
        vspace(gap::XL),
        fact(Icon::Lock, text::WELCOME_LOCAL_TITLE, text::WELCOME[1]),
        vspace(gap::LG),
        fact(Icon::Timer, text::WELCOME_TIME_TITLE, text::WELCOME[2]),
    ]
    .width(Length::Fixed(width))
    .into()
}

/// Look: the song colours and heartbeat, the surface, accent strength, corners, Now Playing style
/// and reduced motion. Every pick goes through `Event::Setting`, so it applies at once.
pub(super) fn look(app: &App, width: f32) -> Element<'_, Message> {
    let two = has_two_columns(width);
    let col_w = if two {
        ((width - gap::LG) / 2.0).floor()
    } else {
        width
    };
    // The song colours come first: they are the look that changes with every song.
    let mut switches = song_colour_rows(app, col_w);
    switches.push(reduce_motion_row(app, col_w));
    let choices = look_choices(app, col_w);
    let body = if two {
        two_columns(switches, choices, width)
    } else {
        column(switches.into_iter().chain(choices)).into()
    };
    column![
        heading(text::LOOK_TITLE, Some(text::LOOK_HELP), width),
        vspace(gap::MD),
        body,
    ]
    .width(Length::Fixed(width))
    .into()
}

fn reduce_motion_row(app: &App, width: f32) -> Element<'_, Message> {
    setting_row(
        width,
        "Reduce motion",
        None,
        switch_with(app.ui.settings.reduce_motion, |on| {
            Message::Ui(Event::Setting(SettingChange::ReduceMotion(on)))
        }),
    )
}

/// A pick-one row: the name, then the options under it. Tighter than the Settings row, so the
/// four of them fit one column at 960 x 600.
fn choice_row<'a>(
    width: f32,
    title: &'a str,
    control: Element<'a, Message>,
) -> Element<'a, Message> {
    container(
        column![
            label(title, type_scale::BODY_MD, false, col(color::on_surface())),
            control
        ]
        .spacing(gap::XS),
    )
    .width(Length::Fixed(width))
    .padding(iced::Padding::from([gap::SM, gap::LG]))
    .into()
}

/// The four pick-one rows of the Look step.
fn look_choices(app: &App, width: f32) -> Vec<Element<'_, Message>> {
    let s = &app.ui.settings;
    vec![
        choice_row(
            width,
            "Surface",
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
        choice_row(
            width,
            "Accent intensity",
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
        choice_row(
            width,
            "Corners",
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
        choice_row(
            width,
            "Now Playing",
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
    ]
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
        vspace(gap::MD),
        column(rows),
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
        vspace(gap::MD),
        column(rows),
        vspace(gap::MD),
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
