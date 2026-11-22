//! The Appearance section (appearance-glass spec): a live preview, then cards for the
//! background, surface, colour theme, accent, corners, Now Playing style, player bar,
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
use crate::i18n::t;
use crate::scheme::{AccentIntensity, BaseMode, CornerStyle, ThemeChoice, swatch};
use crate::settings::{Language, NowPlayingStyle, SettingChange};
use crate::state::Event;
use crate::theme::{color, shape, type_scale};
use crate::window_glass::GlassState;

mod controls;
mod preview;
pub(in crate::gui::views::settings) mod restart;
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
                label(
                    choice.label_localized(s.language),
                    type_scale::BODY_MD,
                    true,
                    name_ink
                ),
                label(
                    fit(
                        choice.note_localized(s.language),
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
    let lang = s.language;
    let inner = width - 2.0 * CARD_PAD;
    let beat = s.heartbeat_available(app.ui.reduced_motion);
    let beat_helper = if beat {
        t(
            lang,
            HEARTBEAT_HELPER,
            "Chuyển động nhẹ nhàng theo nhịp tim khi có bài hát đang phát.",
        )
        .to_owned()
    } else {
        format!(
            "{} {}",
            t(
                lang,
                HEARTBEAT_HELPER,
                "Chuyển động nhẹ nhàng theo nhịp tim khi có bài hát đang phát."
            ),
            t(
                lang,
                words::HEARTBEAT_OFF_NOTE,
                "Tắt khi Giảm chuyển động được bật."
            )
        )
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
                t(lang, words::BACKGROUND_TITLE, "Nền cửa sổ"),
                type_scale::TITLE_SM,
                t(
                    lang,
                    words::BACKGROUND_HELPER,
                    "Cách cửa sổ tô màu phía sau các thẻ và danh sách.",
                ),
                inner,
            ),
            switch_row(
                inner,
                t(lang, SONG_COLOUR_LABEL, "Phối màu theo bài hát đang phát"),
                t(
                    lang,
                    SONG_COLOUR_HELPER,
                    "Toàn bộ cửa sổ lấy màu từ ảnh bìa của bài hát đang phát.",
                )
                .to_owned(),
                switch(
                    lang,
                    s.song_colour_theme,
                    SwitchMode::Normal,
                    SettingChange::SongColourTheme,
                ),
            ),
            switch_row(
                inner,
                t(lang, HEARTBEAT_LABEL, "Nhịp thở nhẹ nhàng"),
                beat_helper,
                switch(
                    lang,
                    s.song_heartbeat,
                    beat_mode,
                    SettingChange::SongHeartbeat,
                ),
            ),
        ],
    )
}

/// The colour cards: seed cards across the whole card.
fn theme_card(app: &App, width: f32) -> Element<'_, Message> {
    let s = &app.ui.settings;
    let lang = s.language;
    let inner = width - 2.0 * CARD_PAD;
    card(
        width,
        vec![
            head(
                t(lang, "Colour theme", "Chủ đề màu sắc"),
                type_scale::TITLE_SM,
                t(lang, words::THEME_HELPER, "Màu sắc chính của ứng dụng."),
                inner,
            ),
            theme_swatches(app, inner),
        ],
    )
}

/// The player bar: what it is made of. It has no control, because the bar is one Material
/// surface (bar-material spec).
fn bar_card<'a>(lang: Language, width: f32) -> Element<'a, Message> {
    let inner = width - 2.0 * CARD_PAD;
    card(
        width,
        vec![head(
            t(lang, words::BAR_TITLE, "Thanh phát nhạc"),
            type_scale::TITLE_SM,
            t(lang, words::BAR_HELPER, "Thanh phát nổi ở cuối cửa sổ."),
            inner,
        )],
    )
}

/// The see-through window: switch, the `Restart now` pill when the switch needs one, state of the
/// running window, opacity, and what blur needs.
fn window_glass_card(app: &App, width: f32) -> Element<'_, Message> {
    let s = &app.ui.settings;
    let lang = s.language;
    let g = app.window_glass();
    let inner = width - 2.0 * CARD_PAD;
    let mode = if g.state() == GlassState::Overridden {
        SwitchMode::Overridden
    } else {
        SwitchMode::Normal
    };
    let status = g.status();
    let needs_restart = app.restart_offered();
    let text = move |w: f32| -> Element<'_, Message> {
        let mut lines = column![head(
            t(lang, words::WINDOW_GLASS_TITLE, "Kính mờ cửa sổ"),
            type_scale::TITLE_SM,
            t(
                lang,
                words::WINDOW_GLASS_HELPER,
                "Hiệu ứng nhìn xuyên thấu phía sau cửa sổ ứng dụng."
            ),
            w
        )]
        .spacing(gap::XS);
        if needs_restart {
            lines = lines.push(paragraph(
                t(
                    lang,
                    words::RESTART_HELPER,
                    "Khởi động lại NemoMusic để áp dụng thay đổi.",
                ),
                type_scale::BODY_SM,
                col(color::on_surface()),
                w,
            ));
        }
        lines
            .push(paragraph(
                status,
                type_scale::BODY_SM,
                col(color::on_surface_variant()),
                w,
            ))
            .into()
    };
    let (control, control_w) = restart::control(
        app,
        switch(lang, s.window_glass, mode, SettingChange::WindowGlass),
        mode,
    );
    let dim = if s.window_glass { 1.0 } else { 0.38 };
    card(
        width,
        vec![
            aside(inner, text, control, control_w),
            aside(
                inner,
                move |w| {
                    head(
                        t(lang, words::OPACITY_TITLE, "Độ mờ"),
                        type_scale::BODY_MD,
                        t(
                            lang,
                            words::OPACITY_HELPER,
                            "Độ trong suốt của bề mặt kính.",
                        ),
                        w,
                    )
                },
                opacity_slider(app, s.window_glass_opacity, dim),
                204.0,
            ),
            paragraph(
                t(
                    lang,
                    words::WINDOW_GLASS_BLUR,
                    "Độ mờ kính được hỗ trợ tùy theo trình quản lý cửa sổ của hệ điều hành.",
                ),
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
    let lang = s.language;
    let inner = width - 2.0 * CARD_PAD;
    card(
        width,
        vec![
            head(
                t(lang, words::MOTION_TITLE, "Chuyển động"),
                type_scale::TITLE_SM,
                "",
                inner,
            ),
            switch_row(
                inner,
                t(lang, "Reduce motion", "Giảm chuyển động"),
                t(
                    lang,
                    words::REDUCE_MOTION_HELPER,
                    "Giảm tối đa các hiệu ứng hoạt ảnh chuyển động trong ứng dụng.",
                )
                .to_owned(),
                switch(
                    lang,
                    s.reduce_motion,
                    SwitchMode::Normal,
                    SettingChange::ReduceMotion,
                ),
            ),
            switch_row(
                inner,
                t(lang, "Reduce transparency", "Giảm độ trong suốt"),
                t(
                    lang,
                    words::REDUCE_TRANSPARENCY_HELPER,
                    "Tắt các bề mặt trong suốt để tăng độ tương phản và dễ đọc.",
                )
                .to_owned(),
                switch(
                    lang,
                    s.reduce_transparency,
                    SwitchMode::Normal,
                    SettingChange::ReduceTransparency,
                ),
            ),
            switch_row(
                inner,
                t(lang, "Show tips on Home", "Hiển thị mẹo ở Trang chủ"),
                t(
                    lang,
                    "Show one quiet tip or shortcut line under the greeting on Home.",
                    "Hiển thị một mẹo hoặc phím tắt nhỏ dưới lời chào ở Trang chủ.",
                )
                .to_owned(),
                switch(
                    lang,
                    s.show_tips,
                    SwitchMode::Normal,
                    SettingChange::ShowTips,
                ),
            ),
        ],
    )
}

/// The whole page: the preview, then the cards.
pub(super) fn page(app: &App, width: f32) -> Element<'_, Message> {
    let s = &app.ui.settings;
    let lang = s.language;
    column![
        preview::preview(app, width),
        background_card(app, width),
        seg_card(
            app,
            width,
            (
                t(lang, "Surface", "Chế độ nền"),
                t(
                    lang,
                    words::SURFACE_HELPER,
                    "Màu sắc bề mặt chủ đạo của ứng dụng."
                )
            ),
            &[
                (BaseMode::Dark, t(lang, "Dark", "Tối"), Some(Icon::DarkMode)),
                (
                    BaseMode::Amoled,
                    t(lang, "AMOLED black", "Đen AMOLED"),
                    Some(Icon::Tune)
                ),
                (
                    BaseMode::Light,
                    t(lang, "Light", "Sáng"),
                    Some(Icon::LightMode)
                ),
            ],
            s.base,
            SettingChange::Base,
        ),
        theme_card(app, width),
        seg_card(
            app,
            width,
            (
                t(lang, "Accent intensity", "Cường độ màu nhấn"),
                t(
                    lang,
                    words::ACCENT_HELPER,
                    "Độ nổi bật của các điểm nhấn màu."
                )
            ),
            &[
                (AccentIntensity::Soft, t(lang, "Soft", "Dịu"), None),
                (
                    AccentIntensity::Standard,
                    t(lang, "Standard", "Tiêu chuẩn"),
                    None
                ),
                (AccentIntensity::Vivid, t(lang, "Vivid", "Rực rỡ"), None),
            ],
            s.accent_intensity,
            SettingChange::AccentIntensity,
        ),
        seg_card(
            app,
            width,
            (
                t(lang, "Corners", "Bo góc"),
                t(
                    lang,
                    words::CORNERS_HELPER,
                    "Mức độ bo tròn các góc thẻ và nút."
                )
            ),
            &[
                (CornerStyle::Sharp, t(lang, "Sharp", "Vuông"), None),
                (CornerStyle::Soft, t(lang, "Soft", "Vừa"), None),
                (CornerStyle::Round, t(lang, "Round", "Bo tròn"), None),
            ],
            s.corner_style,
            SettingChange::CornerStyle,
        ),
        seg_card(
            app,
            width,
            (
                t(lang, "Now Playing style", "Kiểu hiển thị Now Playing"),
                t(
                    lang,
                    words::NOW_PLAYING_HELPER,
                    "Giao diện màn hình đang phát."
                )
            ),
            &[
                (
                    NowPlayingStyle::Classic,
                    t(lang, "Classic", "Cổ điển"),
                    None
                ),
                (
                    NowPlayingStyle::M3Expressive,
                    t(lang, "Material 3 Expressive", "Material 3 Biểu cảm"),
                    None
                ),
            ],
            s.now_playing_style,
            SettingChange::NowPlayingStyle,
        ),
        bar_card(lang, width),
        window_glass_card(app, width),
        motion_card(app, width),
    ]
    .spacing(gap::MD)
    .width(Length::Fixed(width))
    .into()
}
