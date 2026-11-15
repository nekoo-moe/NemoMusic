//! A track row of the album and playlist page (YouTube Music style): cover (or the position on
//! an album), title over "Artist • Album", the source or quality badge, the duration and the
//! menu. Pressing it plays the list from this row; right click or the menu button opens the row
//! menu, as in every other track list.

use iced::widget::{Space, button, column, container, mouse_area, row};
use iced::{Alignment, Color, Element, Length, Padding};
use nemo_core::audio::Origin;
use nemo_core::track::Track;

use super::super::kit::{ROW_H, art, eq_bars, play_badge};
use crate::gui::app::{App, ArtShape, ListSource, Message};
use crate::gui::assets::Icon;
use crate::gui::fx::{BadgeAnchor, fx};
use crate::gui::style::{button_flat, col, fit, gap, icon_button, label, tip};
use crate::state::Event;
use crate::theme::{color, shape, type_scale};

/// Slot of the cover or the position number.
const LEAD: f32 = 44.0;
/// Width of the duration column.
const TIME_W: f32 = 56.0;
/// Width of the menu button.
const MENU_W: f32 = 40.0;
/// Width of the source or quality badge.
const BADGE_W: f32 = 96.0;
/// Narrowest row that still shows the badge.
const BADGE_FROM: f32 = 560.0;

/// How a list of rows looks.
pub struct Rows {
    /// Row width in px.
    pub width: f32,
    /// The list the rows play from.
    pub source: ListSource,
    /// Album page: the position instead of the cover, and no album name.
    pub numbered: bool,
}

/// `Artist • Album`, or only the artists when the album is unknown or not wanted.
#[must_use]
pub fn second_line(track: &Track, with_album: bool) -> String {
    let artists = crate::format::artists(&track.artists);
    match track.album.as_deref().map(str::trim) {
        Some(album) if with_album && !album.is_empty() => {
            if track.artists.is_empty() {
                album.to_owned()
            } else {
                format!("{artists} \u{2022} {album}")
            }
        }
        _ => artists,
    }
}

/// The badge text: the real stream facts for the playing row, else where the song comes from.
/// Never "Lossless" for a YouTube stream: the playing row's text comes from the signal path.
fn badge(app: &App, track: &Track, current: bool) -> String {
    if current {
        return app
            .ui
            .snapshot
            .signal_path
            .as_ref()
            .map_or_else(|| "Unknown".to_owned(), |p| p.source.badge());
    }
    match track.origin {
        Origin::Local => "Local".to_owned(),
        Origin::YoutubeMusic => "YouTube".to_owned(),
        Origin::SoundCloud => "SoundCloud".to_owned(),
    }
}

/// One row, [`ROW_H`] high.
pub fn view<'a>(app: &'a App, track: &Track, index: usize, rows: &Rows) -> Element<'a, Message> {
    let current = app.ui.snapshot.current.as_ref().map(|t| &t.id) == Some(&track.id);
    let dim = col(color::on_surface_variant());
    let accent = app.accent();
    let show_badge = rows.width >= BADGE_FROM;
    let badge_w = if show_badge { BADGE_W + gap::MD } else { 0.0 };
    let fixed = LEAD + gap::MD + TIME_W + MENU_W + 2.0 * gap::SM + badge_w + gap::MD;
    let text_w = (rows.width - fixed).max(96.0);

    let lead: Element<'a, Message> = if rows.numbered {
        let inner: Element<'a, Message> = if current {
            eq_bars(app, col(accent.primary), 16.0)
        } else {
            label(format!("{}", index + 1), type_scale::BODY_MD, false, dim).into()
        };
        container(inner)
            .width(Length::Fixed(LEAD))
            .height(Length::Fixed(LEAD))
            .center_x(Length::Fixed(LEAD))
            .center_y(Length::Fixed(LEAD))
            .into()
    } else {
        art(app, &track.artwork, 44, ArtShape::Row)
    };
    let title_ink = if current {
        col(accent.primary)
    } else {
        col(color::on_surface())
    };
    let mut line = row![
        lead,
        Space::new().width(gap::MD),
        column![
            label(
                fit(&track.title, text_w, type_scale::BODY_MD.size),
                type_scale::BODY_MD,
                true,
                title_ink
            ),
            label(
                fit(
                    &second_line(track, !rows.numbered),
                    text_w,
                    type_scale::BODY_SM.size
                ),
                type_scale::BODY_SM,
                false,
                dim
            ),
        ]
        .width(Length::Fixed(text_w)),
        Space::new().width(Length::Fill),
    ]
    .align_y(Alignment::Center);
    if current && !rows.numbered {
        line = line
            .push(eq_bars(app, col(accent.primary), 16.0))
            .push(Space::new().width(gap::MD));
    }
    if show_badge {
        line = line.push(
            container(label(
                fit(
                    &badge(app, track, current),
                    BADGE_W,
                    type_scale::LABEL_SM.size,
                ),
                type_scale::LABEL_SM,
                false,
                dim,
            ))
            .width(Length::Fixed(BADGE_W + gap::MD))
            .align_x(Alignment::End)
            .padding(Padding {
                right: gap::MD,
                ..Padding::ZERO
            }),
        );
    }
    line = line.push(
        container(label(
            track
                .duration
                .map_or_else(|| "-:--".to_owned(), crate::format::clock),
            type_scale::BODY_MD,
            false,
            dim,
        ))
        .width(Length::Fixed(TIME_W))
        .align_x(Alignment::End),
    );
    let play = Message::PlayAt {
        source: rows.source.clone(),
        index,
    };
    let idle = if current {
        col(color::surface_container_high())
    } else {
        Color::TRANSPARENT
    };
    let main = button(
        container(line)
            .height(Length::Fixed(ROW_H))
            .width(Length::Fill)
            .padding(Padding::from([0.0, gap::SM]))
            .align_y(Alignment::Center),
    )
    .padding(0)
    .width(Length::Fill)
    .on_press(play.clone());
    let radius = shape::lg();
    let host = fx(main.style(button_flat(idle, radius)))
        .radius(radius)
        .badge(play_badge(
            app,
            32.0,
            BadgeAnchor::CenterLeft(if rows.numbered { 6.0 } else { 14.0 }),
            play,
        ));
    let host = mouse_area(host).on_right_press(Message::Ui(Event::OpenMenu(track.clone())));
    row![
        host,
        tip(
            icon_button(
                Icon::More,
                20.0,
                MENU_W,
                dim,
                Color::TRANSPARENT,
                Some(Message::Ui(Event::OpenMenu(track.clone()))),
            ),
            "More",
        ),
    ]
    .align_y(Alignment::Center)
    .width(Length::Fixed(rows.width))
    .height(Length::Fixed(ROW_H))
    .into()
}

#[cfg(test)]
mod tests {
    use super::second_line;
    use crate::mock;

    #[test]
    fn the_second_line_is_artist_and_album_and_skips_what_is_missing() {
        let mut t = mock::track(0);
        t.artists = vec!["Velvet Harbor".to_owned()];
        t.album = Some("Field Recordings".to_owned());
        assert_eq!(
            second_line(&t, true),
            "Velvet Harbor \u{2022} Field Recordings"
        );
        assert_eq!(second_line(&t, false), "Velvet Harbor");
        t.album = Some("  ".to_owned());
        assert_eq!(second_line(&t, true), "Velvet Harbor");
        t.album = None;
        assert_eq!(second_line(&t, true), "Velvet Harbor");
        t.artists.clear();
        t.album = Some("Field Recordings".to_owned());
        assert_eq!(second_line(&t, true), "Field Recordings");
    }
}
