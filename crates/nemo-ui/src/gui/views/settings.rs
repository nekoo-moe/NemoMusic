//! Settings (`ui-reference.md` section 4): a 200 px section list on the left, grouped rows on the
//! right. Changes go through the reducer, which tells the app (`on_settings`).

use iced::widget::{Space, button, column, container, row, slider, toggler};
use iced::{Alignment, Background, Color, Element, Length, Padding};

use super::super::app::{App, Message, SettingsSection};
use super::super::style::{button_style, col, gap, label, slider_style};
use super::kit::{page, page_title, paragraph, text_button, tile, tonal_button, vspace};
use crate::copy;
use crate::format;
use crate::layout;
use crate::settings::{GlassTier, SettingChange};
use crate::signal;
use crate::state::Event;
use crate::theme::{color, shape, type_scale};

/// Width of the section list.
const MENU_W: f32 = 200.0;
/// Widest right column.
const RIGHT_MAX: f32 = 720.0;

/// The Settings screen.
pub fn view(app: &App, w: u32) -> Element<'_, Message> {
    let grid = layout::grid(w, app.ui.queue_open);
    // Content widths are small exact numbers.
    #[allow(clippy::cast_precision_loss)]
    let content_w = grid.content as f32;
    let right_w = (content_w - MENU_W - gap::XL).clamp(320.0, RIGHT_MAX);
    let section = app.settings_section;

    let mut menu = column![].spacing(gap::XS).width(Length::Fixed(MENU_W));
    for s in SettingsSection::ALL {
        let on = s == section;
        menu = menu.push(
            button(
                container(label(
                    s.label(),
                    type_scale::BODY_MD,
                    on,
                    col(if on {
                        color::ON_SURFACE
                    } else {
                        color::ON_SURFACE_VARIANT
                    }),
                ))
                .height(Length::Fixed(40.0))
                .width(Length::Fill)
                .padding(Padding::from([0.0, gap::MD]))
                .align_y(Alignment::Center),
            )
            .padding(0)
            .width(Length::Fill)
            .style(button_style(
                if on {
                    col(color::SURFACE_CONTAINER_HIGH)
                } else {
                    Color::TRANSPARENT
                },
                shape::MD,
            ))
            .on_press(Message::SettingsSection(s)),
        );
    }

    let rows: Vec<Element<'_, Message>> = match section {
        SettingsSection::Playback => playback(app, right_w),
        SettingsSection::AudioOutput => audio_output(app, right_w),
        SettingsSection::Lyrics => lyrics(app, right_w),
        SettingsSection::Library => library(app, right_w),
        SettingsSection::Appearance => appearance(app, right_w),
        SettingsSection::Privacy => privacy(right_w),
        SettingsSection::About => about(right_w),
    };
    let right = column![
        page_title(section.label()),
        vspace(gap::LG),
        group(rows, right_w)
    ]
    .width(Length::Fixed(right_w));
    page(
        grid.content,
        row![menu, Space::new().width(gap::XL), right].align_y(Alignment::Start),
    )
}

/// A group card: rows stacked on `surface-container`, radius `md`.
fn group(rows: Vec<Element<'_, Message>>, width: f32) -> Element<'_, Message> {
    tile(
        column(rows)
            .width(Length::Fixed(width))
            .padding(Padding::from([gap::SM, 0.0])),
        color::SURFACE_CONTAINER,
    )
}

/// A row: label and helper on the left, a control on the right; at least 56 px high.
fn setting_row<'a>(
    width: f32,
    title: &'a str,
    helper: Option<String>,
    control: Element<'a, Message>,
) -> Element<'a, Message> {
    let control_w = 200.0;
    let text_w = (width - control_w - 2.0 * gap::LG - gap::LG).max(160.0);
    let mut text = column![label(
        title,
        type_scale::BODY_MD,
        false,
        col(color::ON_SURFACE)
    )]
    .spacing(gap::XS)
    .width(Length::Fixed(text_w));
    if let Some(h) = helper {
        text = text.push(paragraph(
            h,
            type_scale::BODY_SM,
            col(color::ON_SURFACE_VARIANT),
            text_w,
        ));
    }
    // The 32 px spacer plus 2 x 12 px padding makes every row at least 56 px high.
    row![
        Space::new().height(32.0),
        text,
        Space::new().width(Length::Fill),
        container(control)
            .width(Length::Fixed(control_w))
            .align_x(Alignment::End)
    ]
    .align_y(Alignment::Center)
    .padding(Padding::from([gap::MD, gap::LG]))
    .into()
}

/// `On` / `Off` text next to a switch (state is never colour alone).
fn switch<'a>(on: bool, change: fn(bool) -> SettingChange) -> Element<'a, Message> {
    row![
        label(
            if on { "On" } else { "Off" },
            type_scale::BODY_MD,
            false,
            col(color::ON_SURFACE_VARIANT),
        ),
        Space::new().width(gap::MD),
        toggler(on)
            .on_toggle(move |v| Message::Ui(Event::Setting(change(v))))
            .size(24.0)
            .style(move |_, status| {
                let hovered = matches!(status, toggler::Status::Hovered { .. });
                toggler::Style {
                    background: Background::Color(if on {
                        col(color::ON_SURFACE)
                    } else if hovered {
                        col(color::ON_SURFACE.with_alpha(0.30))
                    } else {
                        col(color::ON_SURFACE.with_alpha(0.24))
                    }),
                    background_border_width: 0.0,
                    background_border_color: Color::TRANSPARENT,
                    foreground: Background::Color(if on {
                        col(color::SURFACE)
                    } else {
                        col(color::ON_SURFACE_VARIANT)
                    }),
                    foreground_border_width: 0.0,
                    foreground_border_color: Color::TRANSPARENT,
                    text_color: None,
                    border_radius: None,
                    padding_ratio: 0.2,
                }
            }),
    ]
    .align_y(Alignment::Center)
    .into()
}

fn playback(app: &App, width: f32) -> Vec<Element<'_, Message>> {
    let s = &app.ui.settings;
    let volume = app.ui.snapshot.volume_percent;
    vec![
        setting_row(
            width,
            copy::SPONSORBLOCK_LABEL,
            Some(format!(
                "{}. {}",
                copy::SPONSORBLOCK_SOURCE,
                copy::SPONSORBLOCK_LICENSE
            )),
            switch(s.sponsorblock, SettingChange::SponsorBlock),
        ),
        setting_row(
            width,
            copy::ADBLOCK_LABEL,
            Some(copy::ADBLOCK_HELPER.to_owned()),
            switch(s.block_ads, SettingChange::BlockAds),
        ),
        setting_row(
            width,
            "Volume",
            Some("At 100% NemoMusic applies no gain at all.".to_owned()),
            row![
                slider(0..=100_u8, volume, |v| Message::Ui(Event::Setting(
                    SettingChange::Volume(v)
                )))
                .height(16.0)
                .width(Length::Fixed(120.0))
                .style(slider_style(
                    col(color::ON_SURFACE),
                    col(color::ON_SURFACE.with_alpha(0.24)),
                    col(color::ON_SURFACE),
                    6.0,
                )),
                Space::new().width(gap::MD),
                container(label(
                    format!("{volume}%"),
                    type_scale::BODY_MD,
                    false,
                    col(color::ON_SURFACE_VARIANT),
                ))
                .width(Length::Fixed(44.0))
                .align_x(Alignment::End),
            ]
            .align_y(Alignment::Center)
            .into(),
        ),
    ]
}

fn audio_output(app: &App, width: f32) -> Vec<Element<'_, Message>> {
    let dim = col(color::ON_SURFACE_VARIANT);
    let Some(path) = app.ui.snapshot.signal_path.as_ref() else {
        return vec![setting_row(
            width,
            "Signal path",
            Some("Nothing is playing. The path appears when a song starts.".to_owned()),
            Space::new().into(),
        )];
    };
    let mut rows: Vec<Element<'_, Message>> = vec![setting_row(
        width,
        "Quality",
        Some("Read from the player. Nothing here can be changed.".to_owned()),
        label(
            signal::badge_label(Some(path)),
            type_scale::BODY_SM,
            false,
            dim,
        )
        .into(),
    )];
    for line in signal::lines(path) {
        rows.push(
            row![
                container(label(
                    line.label,
                    type_scale::BODY_MD,
                    false,
                    col(color::ON_SURFACE)
                ))
                .width(Length::Fixed(96.0)),
                paragraph(
                    line.value,
                    type_scale::BODY_MD,
                    dim,
                    (width - 96.0 - 2.0 * gap::LG).max(160.0)
                ),
            ]
            .padding(Padding::from([gap::MD, gap::LG]))
            .into(),
        );
    }
    rows
}

fn lyrics(app: &App, width: f32) -> Vec<Element<'_, Message>> {
    let offset = if app.ui.snapshot.current.is_some() {
        format::offset_seconds(app.ui.lyrics_offset_ms)
    } else {
        "\u{2014}".to_owned()
    };
    vec![
        setting_row(
            width,
            "Timing offset",
            Some(
                "Set per song on Now Playing, from -10.0 to +10.0 s in 50 ms steps. Press [ and ] while a song plays. Shown here: the current song."
                    .to_owned(),
            ),
            label(offset, type_scale::BODY_MD, false, col(color::ON_SURFACE)).into(),
        ),
        setting_row(
            width,
            "Providers",
            Some(
                "LRCLIB and YouTube Music. The provider name is shown with every lyric."
                    .to_owned(),
            ),
            Space::new().into(),
        ),
    ]
}

fn library(app: &App, width: f32) -> Vec<Element<'_, Message>> {
    let dim = col(color::ON_SURFACE_VARIANT);
    let mut rows: Vec<Element<'_, Message>> = Vec::new();
    if app.music_folders.is_empty() {
        rows.push(setting_row(
            width,
            "Music folders",
            Some("No folders added yet. NemoMusic reads the files in them and keeps the list on this computer.".to_owned()),
            Space::new().into(),
        ));
    } else {
        for f in &app.music_folders {
            rows.push(
                row![label(f.clone(), type_scale::BODY_MD, false, dim)]
                    .padding(Padding::from([gap::MD, gap::LG]))
                    .into(),
            );
        }
    }
    if app.has_add_folder() {
        rows.push(
            row![tonal_button("Add folder", None, Some(Message::AddFolder))]
                .padding(Padding::from([gap::SM, gap::LG]))
                .into(),
        );
    }
    rows
}

fn appearance(app: &App, width: f32) -> Vec<Element<'_, Message>> {
    let s = &app.ui.settings;
    let seg = |tier: GlassTier| -> Element<'_, Message> {
        let on = s.glass_tier == tier;
        button(
            container(label(
                tier.label(),
                type_scale::BODY_MD,
                on,
                col(color::ON_SURFACE),
            ))
            .padding(Padding::from([0.0, gap::LG]))
            .height(Length::Fixed(40.0))
            .align_y(Alignment::Center),
        )
        .padding(0)
        .style(button_style(
            if on {
                col(color::ON_SURFACE.with_alpha(0.16))
            } else {
                col(color::ON_SURFACE.with_alpha(0.06))
            },
            shape::FULL,
        ))
        .on_press(Message::Ui(Event::Setting(SettingChange::GlassTier(tier))))
        .into()
    };
    vec![
        setting_row(
            width,
            "Glass",
            Some("Low draws the player bar from a pre-blurred cover. Off uses a plain surface. Live blur is not built yet.".to_owned()),
            row![seg(GlassTier::Low), Space::new().width(gap::SM), seg(GlassTier::Off)].into(),
        ),
        setting_row(
            width,
            "Reduce motion",
            Some("No cross-fades and no lyric glide. The progress bar keeps moving.".to_owned()),
            switch(s.reduce_motion, SettingChange::ReduceMotion),
        ),
        setting_row(
            width,
            "Reduce transparency",
            Some("The player bar becomes an opaque surface with a thin border.".to_owned()),
            switch(s.reduce_transparency, SettingChange::ReduceTransparency),
        ),
    ]
}

fn privacy(width: f32) -> Vec<Element<'static, Message>> {
    let dim = col(color::ON_SURFACE_VARIANT);
    let inner = width - 2.0 * gap::LG;
    let mut rows: Vec<Element<'static, Message>> = vec![
        container(paragraph(
            copy::PRIVACY_INTRO,
            type_scale::BODY_MD,
            col(color::ON_SURFACE),
            inner,
        ))
        .padding(Padding::from([gap::MD, gap::LG]))
        .into(),
    ];
    for (host, why) in copy::EGRESS {
        rows.push(
            container(
                column![
                    paragraph(host, type_scale::BODY_MD, col(color::ON_SURFACE), inner),
                    paragraph(why, type_scale::BODY_SM, dim, inner),
                ]
                .spacing(gap::XS),
            )
            .padding(Padding::from([gap::SM, gap::LG]))
            .into(),
        );
    }
    rows.push(
        container(paragraph(
            copy::PRIVACY_DATA,
            type_scale::BODY_SM,
            dim,
            inner,
        ))
        .padding(Padding::from([gap::MD, gap::LG]))
        .into(),
    );
    rows
}

fn about(width: f32) -> Vec<Element<'static, Message>> {
    let dim = col(color::ON_SURFACE_VARIANT);
    let inner = width - 2.0 * gap::LG;
    let block = |title: &'static str, body: String, ink: Color| -> Element<'static, Message> {
        container(
            column![
                label(title, type_scale::BODY_MD, true, col(color::ON_SURFACE)),
                paragraph(body, type_scale::BODY_SM, ink, inner),
            ]
            .spacing(gap::XS),
        )
        .padding(Padding::from([gap::MD, gap::LG]))
        .into()
    };
    vec![
        block(
            "NemoMusic",
            format!("Version {}", env!("CARGO_PKG_VERSION")),
            dim,
        ),
        block("Licences", copy::ABOUT_LICENSE.to_owned(), dim),
        block("Credits", copy::ABOUT_CREDITS.join("\n"), dim),
        block("YouTube Music", copy::ABOUT_TOS.to_owned(), dim),
        container(text_button(
            "Review the first-run notice",
            Message::Ui(Event::TosReview),
        ))
        .padding(Padding::from([gap::XS, gap::SM]))
        .into(),
    ]
}
