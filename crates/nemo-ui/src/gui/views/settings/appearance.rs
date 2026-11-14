//! The Appearance section: surface, colour theme, accent, corners, glass and motion.

use iced::widget::{Space, button, column, container, row};
use iced::{Alignment, Element, Length, Padding};

use super::control::{segmented, setting_row, stacked_row};
use super::switch;
use crate::gui::app::{App, Message};
use crate::gui::assets::Icon;
use crate::gui::style::{animated, button_outlined, col, fit, gap, icon, label};
use crate::gui::views::kit::vspace;
use crate::scheme::{AccentIntensity, BaseMode, CornerStyle, ThemeChoice, swatch};
use crate::settings::{GlassTier, SettingChange};
use crate::state::Event;
use crate::theme::{color, shape, type_scale};

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
                crate::gui::fx::fx(btn.style(button_outlined(
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

pub(super) fn appearance(app: &App, width: f32) -> Vec<Element<'_, Message>> {
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
