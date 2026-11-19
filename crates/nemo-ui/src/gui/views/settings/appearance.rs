//! The Appearance section (appearance-glass spec): a live preview, then cards for the
//! background, surface, colour theme, accent, corners, Now Playing style, player bar glass,
//! window glass, and motion and accessibility.

use iced::widget::{Space, button, column, container, row};
use iced::{Alignment, Element, Length};

use super::control::segmented;
use crate::copy::appearance as words;
use crate::copy::{HEARTBEAT_HELPER, HEARTBEAT_LABEL, SONG_COLOUR_HELPER, SONG_COLOUR_LABEL};
use crate::gui::app::{App, Message};
use crate::gui::assets::Icon;
use crate::gui::style::{button_outlined, col, fit, gap, icon, label};
use crate::gui::views::kit::{paragraph, vspace};
use crate::scheme::{AccentIntensity, BaseMode, CornerStyle, ThemeChoice, swatch};
use crate::settings::{GlassTier, NowPlayingStyle, SettingChange};
use crate::state::Event;
use crate::theme::{color, shape, type_scale};
use crate::window_glass::GlassState;

mod controls;
mod preview;
#[cfg(test)]
mod tests;

use controls::{
    CARD_PAD, SwitchMode, aside, card, control_card, head, opacity_slider, segmented_width, switch,
    switch_row,
};

/// The colour swatches: three dots drawn with the palette the theme would give right now, the
/// name, a note, and a check on the chosen one.
fn theme_swatches(app: &App, inner: f32) -> Element<'_, Message> {
    let s = &app.ui.settings;
    let accent = app.accent();
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
                            color: col(color::surface_container_highest()),
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
                crate::gui::fx::fx(btn.style(button_outlined(
                    col(color::surface_container_highest()),
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

/// A card with one segmented control on the right edge.
fn seg_card<'a, T: Copy + PartialEq + 'a>(
    app: &App,
    width: f32,
    (title, helper): (&'a str, &'a str),
    options: &[(T, &'static str, Option<Icon>)],
    current: T,
    change: fn(T) -> SettingChange,
) -> Element<'a, Message> {
    let labels: Vec<&str> = options.iter().map(|o| o.1).collect();
    control_card(
        width,
        title,
        helper,
        segmented(app, options, current, change),
        segmented_width(&labels),
    )
}

/// Match colours to the playing song, and the heartbeat that needs it.
fn background_card(app: &App, width: f32) -> Element<'_, Message> {
    let s = &app.ui.settings;
    let inner = width - 2.0 * CARD_PAD;
    let beat = s.heartbeat_available(app.ui.reduced_motion);
    let beat_helper = if beat {
        HEARTBEAT_HELPER.to_owned()
    } else {
        format!("{HEARTBEAT_HELPER} {}", words::HEARTBEAT_OFF_NOTE)
    };
    let beat_mode = if beat {
        SwitchMode::Normal
    } else {
        SwitchMode::Disabled
    };
    card(
        width,
        vec![
            head(
                words::BACKGROUND_TITLE,
                type_scale::TITLE_SM,
                words::BACKGROUND_HELPER,
                inner,
            ),
            switch_row(
                inner,
                SONG_COLOUR_LABEL,
                SONG_COLOUR_HELPER.to_owned(),
                switch(
                    s.song_colour_theme,
                    SwitchMode::Normal,
                    SettingChange::SongColourTheme,
                ),
            ),
            switch_row(
                inner,
                HEARTBEAT_LABEL,
                beat_helper,
                switch(s.song_heartbeat, beat_mode, SettingChange::SongHeartbeat),
            ),
        ],
    )
}

/// The colour cards: seed cards across the whole card.
fn theme_card(app: &App, width: f32) -> Element<'_, Message> {
    let inner = width - 2.0 * CARD_PAD;
    card(
        width,
        vec![
            head(
                "Colour theme",
                type_scale::TITLE_SM,
                words::THEME_HELPER,
                inner,
            ),
            theme_swatches(app, inner),
        ],
    )
}

/// The see-through window: switch, restart note, state of the running window, opacity, and what
/// blur needs.
fn window_glass_card(app: &App, width: f32) -> Element<'_, Message> {
    let s = &app.ui.settings;
    let g = app.window_glass();
    let inner = width - 2.0 * CARD_PAD;
    let mode = if g.state() == GlassState::Overridden {
        SwitchMode::Overridden
    } else {
        SwitchMode::Normal
    };
    let status = g.status();
    let text = move |w: f32| -> Element<'_, Message> {
        column![
            head(
                words::WINDOW_GLASS_TITLE,
                type_scale::TITLE_SM,
                words::WINDOW_GLASS_HELPER,
                w
            ),
            paragraph(
                words::WINDOW_GLASS_RESTART,
                type_scale::BODY_SM,
                col(color::on_surface()),
                w
            ),
            paragraph(
                status,
                type_scale::BODY_SM,
                col(color::on_surface_variant()),
                w
            ),
        ]
        .spacing(gap::XS)
        .into()
    };
    let dim = if s.window_glass { 1.0 } else { 0.38 };
    card(
        width,
        vec![
            aside(
                inner,
                text,
                switch(s.window_glass, mode, SettingChange::WindowGlass),
                mode.box_w(),
            ),
            aside(
                inner,
                |w| {
                    head(
                        words::OPACITY_TITLE,
                        type_scale::BODY_MD,
                        words::OPACITY_HELPER,
                        w,
                    )
                },
                opacity_slider(app, s.window_glass_opacity, dim),
                204.0,
            ),
            paragraph(
                words::WINDOW_GLASS_BLUR,
                type_scale::BODY_SM,
                col(color::on_surface_variant()),
                inner,
            ),
        ],
    )
}

/// Reduce motion and Reduce transparency.
fn motion_card(app: &App, width: f32) -> Element<'_, Message> {
    let s = &app.ui.settings;
    let inner = width - 2.0 * CARD_PAD;
    card(
        width,
        vec![
            head(words::MOTION_TITLE, type_scale::TITLE_SM, "", inner),
            switch_row(
                inner,
                "Reduce motion",
                words::REDUCE_MOTION_HELPER.to_owned(),
                switch(
                    s.reduce_motion,
                    SwitchMode::Normal,
                    SettingChange::ReduceMotion,
                ),
            ),
            switch_row(
                inner,
                "Reduce transparency",
                words::REDUCE_TRANSPARENCY_HELPER.to_owned(),
                switch(
                    s.reduce_transparency,
                    SwitchMode::Normal,
                    SettingChange::ReduceTransparency,
                ),
            ),
        ],
    )
}

/// The whole page: the preview, then the cards.
pub(super) fn page(app: &App, width: f32) -> Element<'_, Message> {
    let s = &app.ui.settings;
    column![
        preview::preview(app, width),
        background_card(app, width),
        seg_card(
            app,
            width,
            ("Surface", words::SURFACE_HELPER),
            &[
                (BaseMode::Dark, "Dark", Some(Icon::DarkMode)),
                (BaseMode::Amoled, "AMOLED black", Some(Icon::Tune)),
                (BaseMode::Light, "Light", Some(Icon::LightMode)),
            ],
            s.base,
            SettingChange::Base,
        ),
        theme_card(app, width),
        seg_card(
            app,
            width,
            ("Accent intensity", words::ACCENT_HELPER),
            &[
                (AccentIntensity::Soft, "Soft", None),
                (AccentIntensity::Standard, "Standard", None),
                (AccentIntensity::Vivid, "Vivid", None),
            ],
            s.accent_intensity,
            SettingChange::AccentIntensity,
        ),
        seg_card(
            app,
            width,
            ("Corners", words::CORNERS_HELPER),
            &[
                (CornerStyle::Sharp, "Sharp", None),
                (CornerStyle::Soft, "Soft", None),
                (CornerStyle::Round, "Round", None),
            ],
            s.corner_style,
            SettingChange::CornerStyle,
        ),
        seg_card(
            app,
            width,
            ("Now Playing style", words::NOW_PLAYING_HELPER),
            &[
                (NowPlayingStyle::Classic, "Classic", None),
                (NowPlayingStyle::M3Expressive, "Material 3 Expressive", None),
            ],
            s.now_playing_style,
            SettingChange::NowPlayingStyle,
        ),
        seg_card(
            app,
            width,
            (words::GLASS_TITLE, words::GLASS_HELPER),
            &[
                (GlassTier::Low, GlassTier::Low.label(), None),
                (GlassTier::Off, GlassTier::Off.label(), None),
            ],
            s.glass_tier,
            SettingChange::GlassTier,
        ),
        window_glass_card(app, width),
        motion_card(app, width),
    ]
    .spacing(gap::MD)
    .width(Length::Fixed(width))
    .into()
}
