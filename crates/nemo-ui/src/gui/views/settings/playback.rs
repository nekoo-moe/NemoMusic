//! Settings sections about listening: playback, audio output, lyrics and the library.

use iced::widget::{Space, container, row, slider};
use iced::{Alignment, Element, Length, Padding};

use super::control::setting_row;
use super::switch;
use crate::copy;
use crate::format;
use crate::gui::app::{App, Message};
use crate::gui::style::{col, gap, label, slider_style};
use crate::gui::views::kit::{paragraph, tonal_button};
use crate::settings::SettingChange;
use crate::signal;
use crate::state::Event;
use crate::theme::{color, type_scale};

pub(super) fn playback(app: &App, width: f32) -> Vec<Element<'_, Message>> {
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

pub(super) fn audio_output(app: &App, width: f32) -> Vec<Element<'_, Message>> {
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

pub(super) fn lyrics(app: &App, width: f32) -> Vec<Element<'_, Message>> {
    let offset = if app.ui.snapshot.current.is_some() {
        format::offset_seconds(app.ui.lyrics_offset_ms)
    } else {
        "\u{2014}".to_owned()
    };
    let mut rows = vec![setting_row(
        width,
        "Timing offset",
        Some(
            "Set per song on Now Playing, from -10.0 to +10.0 s in 50 ms steps. Press [ and ] while a song plays. Shown here: the current song."
                .to_owned(),
        ),
        label(offset, type_scale::BODY_MD, false, col(color::on_surface())).into(),
    )];
    rows.extend(crate::gui::views::settings_lyrics::sources(app, width));
    rows
}

pub(super) fn library(app: &App, width: f32) -> Vec<Element<'_, Message>> {
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
