//! Settings (`ui-reference.md` section 4): a 200 px section list on the left, grouped rows on the
//! right. Changes go through the reducer, which tells the app (`on_settings`).

use iced::widget::{Space, button, column, container, row, slider, toggler};
use iced::{Alignment, Background, Color, Element, Length, Padding};

use super::super::app::{App, Message, SettingsSection};
use super::super::assets::Icon;
use super::super::style::{animated, button_outlined, col, fit, gap, icon, label, slider_style};
use super::kit::{page, page_title, paragraph, text_button, tile, tonal_button, vspace};
use crate::copy;
use crate::format;
use crate::layout;
use crate::scheme::{AccentIntensity, BaseMode, CornerStyle, ThemeChoice, swatch};
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
        color::surface_container(),
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
        col(color::on_surface())
    )]
    .spacing(gap::XS)
    .width(Length::Fixed(text_w));
    if let Some(h) = helper {
        text = text.push(paragraph(
            h,
            type_scale::BODY_SM,
            col(color::on_surface_variant()),
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
    switch_with(on, move |v| Message::Ui(Event::Setting(change(v))))
}

/// Like [`switch`], with any message.
fn switch_with<'a>(on: bool, msg: impl Fn(bool) -> Message + 'a) -> Element<'a, Message> {
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
            .style(move |_, status| {
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
                    col(color::on_surface()),
                    col(color::on_surface().with_alpha(0.24)),
                    col(color::on_surface()),
                    6.0,
                )),
                Space::new().width(gap::MD),
                container(label(
                    format!("{volume}%"),
                    type_scale::BODY_MD,
                    false,
                    col(color::on_surface_variant()),
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
    let dim = col(color::on_surface_variant());
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
                    col(color::on_surface())
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
            label(offset, type_scale::BODY_MD, false, col(color::on_surface())).into(),
        ),
    ]
    .into_iter()
    .chain(app.lyric_sources.iter().map(|s| {
        let id = s.id;
        let mut helper = format!(
            "Asks {}. {}",
            s.hosts.join(", "),
            if s.default_enabled {
                "On by default."
            } else {
                "Off until you turn it on."
            }
        );
        if let Some(risk) = s.risk_note {
            helper = format!("{helper} Risk: {risk}");
        }
        setting_row(
            width,
            id.as_str(),
            Some(helper),
            switch_with(s.enabled, move |v| Message::SetLyricsSource(id, v)),
        )
    }))
    .collect()
}

fn library(app: &App, width: f32) -> Vec<Element<'_, Message>> {
    let dim = col(color::on_surface_variant());
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

/// A row whose control needs the full width: text on top, the control under it.
fn stacked_row<'a>(
    width: f32,
    title: &'a str,
    helper: Option<String>,
    control: Element<'a, Message>,
) -> Element<'a, Message> {
    let inner = width - 2.0 * gap::LG;
    let mut c = column![label(
        title,
        type_scale::BODY_MD,
        false,
        col(color::on_surface())
    )]
    .spacing(gap::XS)
    .width(Length::Fixed(inner));
    if let Some(h) = helper {
        c = c.push(paragraph(
            h,
            type_scale::BODY_SM,
            col(color::on_surface_variant()),
            inner,
        ));
    }
    c = c.push(vspace(gap::SM)).push(control);
    container(c)
        .padding(Padding::from([gap::MD, gap::LG]))
        .into()
}

/// A segmented control: one pill per option, the chosen one filled with the accent container.
fn segmented<'a, T: Copy + PartialEq + 'a>(
    app: &App,
    options: &[(T, &'static str, Option<Icon>)],
    current: T,
    change: fn(T) -> SettingChange,
) -> Element<'a, Message> {
    let accent = app.accent();
    let mut r = row![].spacing(gap::SM).align_y(Alignment::Center);
    for (value, text, glyph) in options {
        let on = *value == current;
        let (fill, ink) = if on {
            (
                col(accent.secondary_container),
                col(accent.on_secondary_container),
            )
        } else {
            (
                col(color::on_surface().with_alpha(0.06)),
                col(color::on_surface()),
            )
        };
        let mut content = row![].align_y(Alignment::Center);
        if on {
            content = content
                .push(icon(Icon::Check, 18.0, ink))
                .push(Space::new().width(gap::XS));
        } else if let Some(g) = glyph {
            content = content
                .push(icon(*g, 18.0, ink))
                .push(Space::new().width(gap::XS));
        }
        content = content.push(label(*text, type_scale::BODY_MD, on, ink));
        let b = button(
            container(content)
                .padding(Padding::from([0.0, gap::LG]))
                .height(Length::Fixed(40.0))
                .align_y(Alignment::Center),
        )
        .padding(0)
        .on_press(Message::Ui(Event::Setting(change(*value))));
        r = r.push(animated(b, fill, shape::FULL));
    }
    r.into()
}

/// The colour swatches: three dots drawn with the palette the theme would give right now, the
/// name, a note, and a check on the chosen one.
fn theme_swatches(app: &App, width: f32) -> Element<'_, Message> {
    let s = &app.ui.settings;
    let accent = app.accent();
    let inner = width - 2.0 * gap::LG;
    let per_row: u8 = if inner >= 640.0 { 7 } else { 4 };
    let per_row_f = f32::from(per_row);
    let cell = ((inner - (per_row_f - 1.0) * gap::SM) / per_row_f).floor();
    let mut rows = column![].spacing(gap::SM);
    for chunk in ThemeChoice::ALL.chunks(usize::from(per_row)) {
        let mut line = row![].spacing(gap::SM);
        for choice in chunk {
            let on = s.theme == *choice;
            let [a, b, c] = swatch(*choice, s.base, s.accent_intensity);
            let dot = |rgba: crate::theme::Rgba| {
                container(Space::new())
                    .width(Length::Fixed(20.0))
                    .height(Length::Fixed(20.0))
                    .style(move |_| container::Style {
                        background: Some(col(rgba).into()),
                        border: iced::Border {
                            radius: shape::FULL.into(),
                            width: 2.0,
                            color: col(color::surface_container_high()),
                        },
                        ..container::Style::default()
                    })
            };
            let dots = row![dot(a), dot(b), dot(c)].spacing(-6.0);
            let name_ink = col(color::on_surface());
            let content = column![
                row![
                    dots,
                    Space::new().width(Length::Fill),
                    if on {
                        Element::from(icon(Icon::CheckCircle, 18.0, col(accent.primary)))
                    } else {
                        Space::new().width(18.0).height(18.0).into()
                    }
                ]
                .align_y(Alignment::Center),
                vspace(gap::SM),
                label(choice.label(), type_scale::BODY_MD, true, name_ink),
                label(
                    fit(
                        choice.note(),
                        cell - 2.0 * gap::MD,
                        type_scale::LABEL_SM.size
                    ),
                    type_scale::LABEL_SM,
                    false,
                    col(color::on_surface_variant())
                ),
            ]
            .width(Length::Fixed(cell - 2.0 * gap::MD));
            let outline = if on { Some(col(accent.primary)) } else { None };
            let btn = button(container(content).padding(gap::MD))
                .padding(0)
                .width(Length::Fixed(cell))
                .on_press(Message::Ui(Event::Setting(SettingChange::Theme(*choice))));
            line = line.push(
                super::super::fx::fx(btn.style(button_outlined(
                    col(color::surface_container_high()),
                    outline,
                    shape::md(),
                )))
                .radius(shape::md()),
            );
        }
        rows = rows.push(line);
    }
    rows.into()
}

fn appearance(app: &App, width: f32) -> Vec<Element<'_, Message>> {
    let s = &app.ui.settings;
    let seg = |tier: GlassTier| -> Element<'_, Message> {
        let on = s.glass_tier == tier;
        let idle = if on {
            col(color::on_surface().with_alpha(0.16))
        } else {
            col(color::on_surface().with_alpha(0.06))
        };
        let b = button(
            container(label(
                tier.label(),
                type_scale::BODY_MD,
                on,
                col(color::on_surface()),
            ))
            .padding(Padding::from([0.0, gap::LG]))
            .height(Length::Fixed(40.0))
            .align_y(Alignment::Center),
        )
        .padding(0)
        .on_press(Message::Ui(Event::Setting(SettingChange::GlassTier(tier))));
        animated(b, idle, shape::FULL)
    };
    vec![
        stacked_row(
            width,
            "Surface",
            Some("Dark is the default. AMOLED black saves power on OLED screens. Light is for bright rooms.".to_owned()),
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
            "Colour theme",
            Some("Dynamic follows the cover of the song that plays. The others keep one palette.".to_owned()),
            theme_swatches(app, width),
        ),
        stacked_row(
            width,
            "Accent intensity",
            Some("How strong the accent colour is. Text stays at 4.5:1 contrast or better.".to_owned()),
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
            Some("Sharp, the Material 3 default, or extra round.".to_owned()),
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
        setting_row(
            width,
            "Glass",
            Some("Low draws the player bar from a pre-blurred cover. Off uses a plain surface. Live blur is not built yet. Light mode always uses the plain surface.".to_owned()),
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
    let dim = col(color::on_surface_variant());
    let inner = width - 2.0 * gap::LG;
    let mut rows: Vec<Element<'static, Message>> = vec![
        container(paragraph(
            copy::PRIVACY_INTRO,
            type_scale::BODY_MD,
            col(color::on_surface()),
            inner,
        ))
        .padding(Padding::from([gap::MD, gap::LG]))
        .into(),
    ];
    for (host, why) in copy::EGRESS {
        rows.push(
            container(
                column![
                    paragraph(host, type_scale::BODY_MD, col(color::on_surface()), inner),
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
    let dim = col(color::on_surface_variant());
    let inner = width - 2.0 * gap::LG;
    let block = |title: &'static str, body: String, ink: Color| -> Element<'static, Message> {
        container(
            column![
                label(title, type_scale::BODY_MD, true, col(color::on_surface())),
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
