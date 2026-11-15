//! The album and playlist page in the YouTube Music layout (collection-page-ytm spec): a tall
//! gradient from the page cover's colour behind everything; a centred left column with the cover,
//! the title, the owner, the meta lines and round Shuffle and Play buttons; the track list on the
//! right. Two columns from a 1280 px window when the content is wide enough; stacked otherwise.
//! In two columns each column scrolls on its own, so the cover and the buttons stay in view.

use std::time::Duration;

use iced::alignment::Horizontal;
use iced::widget::text::Wrapping;
use iced::widget::{Space, column, container, row, stack};
use iced::{Alignment, Background, Element, Length, Padding};
use nemo_core::track::{Artwork, Track};

use super::super::kit::{PAGE_BOTTOM, art, text_button, vspace};
use super::song_row::{self, Rows};
use crate::gui::app::{App, ArtShape, DetailKey, ListSource, Message};
use crate::gui::assets::Icon;
use crate::gui::fx::Fx;
use crate::gui::style::{DOWN, col, fit, gap, icon_button, label, scroll, tip};
use crate::scheme::{GLOW_MID_ALPHA, GLOW_MID_AT, GLOW_TOP_ALPHA, page_glow};
use crate::theme::{color, type_scale};

/// Narrowest window with two columns.
pub const TWO_COLUMN_WINDOW: u32 = 1280;
/// Narrowest content column with two columns.
pub const TWO_COLUMN_CONTENT: u32 = 640;
/// Share of the content width the left column takes in two columns.
const LEFT_SHARE: f32 = 0.38;
/// Narrowest and widest left column.
const LEFT_MIN: u32 = 260;
const LEFT_MAX: u32 = 360;
/// Cover of the stacked layout, and of narrow panes.
const STACKED_ART: u32 = 240;
const STACKED_ART_NARROW: u32 = 200;
/// Height of the gradient behind the stacked layout (it scrolls with the page).
const STACKED_GLOW_H: f32 = 520.0;
/// Size of the Play disc and of the Shuffle button.
const PLAY_DISC: f32 = 64.0;
const ROUND_BUTTON: f32 = 48.0;

/// The page's layout for a window and content width.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Columns {
    /// Left column and list widths in px (with a 32 px gap between them).
    Two {
        /// Left column (and cover) width.
        left: u32,
        /// Track list width.
        list: u32,
    },
    /// One column: the cover size.
    Stacked {
        /// Cover size.
        art: u32,
    },
}

/// Two columns at `window_w >= 1280` when the content is at least 640 px wide (the right pane
/// can make it narrower); the left column is 38 % of it, 260 to 360 px, a multiple of 4.
#[must_use]
pub fn columns(window_w: u32, content_w: u32) -> Columns {
    if window_w >= TWO_COLUMN_WINDOW && content_w >= TWO_COLUMN_CONTENT {
        let gap = 32_u32;
        // Content widths are small exact numbers.
        #[allow(
            clippy::cast_precision_loss,
            clippy::cast_possible_truncation,
            clippy::cast_sign_loss
        )]
        let share = (content_w.saturating_sub(gap) as f32 * LEFT_SHARE) as u32;
        let left = share.clamp(LEFT_MIN, LEFT_MAX) / 4 * 4;
        Columns::Two {
            left,
            list: content_w.saturating_sub(left + gap),
        }
    } else {
        Columns::Stacked {
            art: if content_w >= 480 {
                STACKED_ART
            } else {
                STACKED_ART_NARROW
            },
        }
    }
}

/// What the page shows; every field comes from the catalog, nothing is made up.
pub struct Collection<'a> {
    /// `Album` or `Playlist`.
    pub kind: &'static str,
    /// The page cover.
    pub artwork: Artwork,
    /// Title.
    pub title: &'a str,
    /// Album artists or the playlist's author, when known.
    pub owner: Option<String>,
    /// Release year, when known.
    pub year: Option<u16>,
    /// The songs.
    pub tracks: &'a [Track],
    /// Album page: numbered rows without covers.
    pub numbered: bool,
}

/// `Album • 2023`, or the kind alone when the year is unknown.
#[must_use]
pub fn kind_line(kind: &str, year: Option<u16>) -> String {
    year.map_or_else(|| kind.to_owned(), |y| format!("{kind} \u{2022} {y}"))
}

/// `12 songs • 43 min` (or `1 h 5 min`); the time is left out when no song has a length.
#[must_use]
pub fn count_line(tracks: &[Track]) -> String {
    let songs = if tracks.len() == 1 {
        "1 song".to_owned()
    } else {
        format!("{} songs", tracks.len())
    };
    let total: Duration = tracks.iter().filter_map(|t| t.duration).sum();
    if total.is_zero() {
        return songs;
    }
    let mins = total.as_secs().div_ceil(60);
    let time = if mins >= 60 {
        format!("{} h {} min", mins / 60, mins % 60)
    } else {
        format!("{mins} min")
    };
    format!("{songs} \u{2022} {time}")
}

/// The page.
pub fn view<'a>(
    app: &'a App,
    key: &DetailKey,
    c: &Collection<'a>,
    content_w: u32,
) -> Element<'a, Message> {
    match columns(app.window_px().0, content_w) {
        Columns::Two { left, list } => two_columns(app, key, c, left, list),
        Columns::Stacked { art } => stacked(app, key, c, content_w, art),
    }
}

fn two_columns<'a>(
    app: &'a App,
    key: &DetailKey,
    c: &Collection<'a>,
    left: u32,
    list: u32,
) -> Element<'a, Message> {
    // Column widths are small exact numbers.
    #[allow(clippy::cast_precision_loss)]
    let (left_w, list_w) = (left as f32, list as f32);
    let pad = Padding {
        top: gap::SM,
        bottom: PAGE_BOTTOM,
        ..Padding::ZERO
    };
    let hero = scroll(container(hero(app, key, c, left_w, left)).padding(pad))
        .width(Length::Fixed(left_w))
        .height(Length::Fill);
    let rows = scroll(container(tracks(app, key, c, list_w)).padding(pad))
        .width(Length::Fixed(list_w + gap::SM))
        .height(Length::Fill);
    let body = container(row![hero, Space::new().width(gap::XXL), rows].height(Length::Fill))
        .width(Length::Fill)
        .height(Length::Fill)
        .center_x(Length::Fill);
    stack![body]
        .push_under(glow(app, key, Length::Fill))
        .width(Length::Fill)
        .height(Length::Fill)
        .into()
}

fn stacked<'a>(
    app: &'a App,
    key: &DetailKey,
    c: &Collection<'a>,
    content_w: u32,
    art_px: u32,
) -> Element<'a, Message> {
    // Content widths are small exact numbers.
    #[allow(clippy::cast_precision_loss)]
    let w = content_w as f32;
    let body = column![
        hero(app, key, c, w, art_px),
        vspace(gap::XL),
        tracks(app, key, c, w)
    ]
    .width(Length::Fixed(w));
    let framed = container(body)
        .width(Length::Fill)
        .padding(Padding {
            top: gap::SM,
            bottom: PAGE_BOTTOM,
            ..Padding::ZERO
        })
        .center_x(Length::Fill);
    let layered = stack![framed].push_under(glow(app, key, Length::Fixed(STACKED_GLOW_H)));
    scroll(layered)
        .width(Length::Fill)
        .height(Length::Fill)
        .into()
}

/// The tall gradient from the page cover's colour, fading into the pane at the bottom. It
/// fades in with the page entrance.
fn glow<'a>(app: &App, key: &DetailKey, height: Length) -> Element<'a, Message> {
    let [top, mid] = page_glow(app.page_seeds.get(key).copied(), app.ui.settings.base);
    let k = app.anim.page().clamp(0.0, 1.0);
    let g = iced::gradient::Linear::new(iced::Radians(DOWN))
        .add_stop(0.0, col(top.with_alpha(GLOW_TOP_ALPHA * k)))
        .add_stop(GLOW_MID_AT, col(mid.with_alpha(GLOW_MID_ALPHA * k)))
        .add_stop(1.0, col(mid.with_alpha(0.0)));
    container(Space::new())
        .width(Length::Fill)
        .height(height)
        .style(move |_| container::Style {
            background: Some(Background::Gradient(g.into())),
            ..container::Style::default()
        })
        .into()
}

/// The page cover, or a 2 x 2 mosaic of the first song covers when the page has none.
fn cover<'a>(app: &'a App, c: &Collection<'_>, px: u32) -> Element<'a, Message> {
    let has_own = match &c.artwork {
        Artwork::None => false,
        Artwork::Remote(t) => !t.is_empty(),
        Artwork::LocalFile(_) => true,
    };
    let mut firsts: Vec<&Artwork> = Vec::new();
    for t in c.tracks {
        if !firsts.contains(&&t.artwork) {
            firsts.push(&t.artwork);
        }
        if firsts.len() == 4 {
            break;
        }
    }
    let art_el: Element<'a, Message> = if has_own || firsts.len() < 4 {
        art(app, &c.artwork, px, ArtShape::Hero)
    } else {
        // Half the slot less half the 4 px gap (`gap::XS`).
        let tile = px.saturating_sub(4) / 2;
        let cell = |a: &Artwork| art(app, a, tile, ArtShape::Card);
        column![
            row![
                cell(firsts[0]),
                Space::new().width(gap::XS),
                cell(firsts[1])
            ],
            vspace(gap::XS),
            row![
                cell(firsts[2]),
                Space::new().width(gap::XS),
                cell(firsts[3])
            ],
        ]
        .into()
    };
    Fx::new(art_el)
        .passive()
        .scale(0.86 + 0.14 * app.anim.page())
        .into()
}

/// The centred left column (or the top of the stacked page).
fn hero<'a>(
    app: &'a App,
    key: &DetailKey,
    c: &Collection<'a>,
    width: f32,
    art_px: u32,
) -> Element<'a, Message> {
    let ink = col(color::on_surface());
    let dim = col(color::on_surface_variant());
    let source = ListSource::Detail(key.clone());
    let centred = |t: iced::widget::Text<'a>| -> Element<'a, Message> {
        t.wrapping(Wrapping::Word)
            .width(Length::Fixed(width))
            .align_x(Horizontal::Center)
            .into()
    };
    let mut info = column![
        cover(app, c, art_px),
        vspace(gap::XL),
        centred(label(
            c.title.to_owned(),
            type_scale::HEADLINE_MD,
            true,
            ink
        )),
    ]
    .align_x(Alignment::Center)
    .width(Length::Fixed(width));
    if let Some(owner) = &c.owner {
        info = info.push(vspace(gap::SM)).push(label(
            fit(owner, width, type_scale::TITLE_SM.size),
            type_scale::TITLE_SM,
            true,
            ink,
        ));
    }
    info = info
        .push(vspace(gap::SM))
        .push(centred(label(
            kind_line(c.kind, c.year),
            type_scale::BODY_MD,
            false,
            dim,
        )))
        .push(centred(label(
            count_line(c.tracks),
            type_scale::BODY_MD,
            false,
            dim,
        )));
    let shuffle = tip(
        icon_button(
            Icon::Shuffle,
            24.0,
            ROUND_BUTTON,
            ink,
            col(color::surface_container_highest()),
            Some(Message::Shuffle(source.clone())),
        ),
        "Shuffle",
    );
    // The big Play disc is white (`on-surface`) with a `surface` glyph, as on YouTube Music
    // (owner override of the accent Play, 2026-11-15).
    let play = tip(
        icon_button(
            Icon::Play,
            30.0,
            PLAY_DISC,
            col(color::surface()),
            ink,
            Some(Message::PlayAt { source, index: 0 }),
        ),
        "Play",
    );
    info.push(vspace(gap::XL))
        .push(row![shuffle, Space::new().width(gap::LG), play].align_y(Alignment::Center))
        .into()
}

fn tracks<'a>(
    app: &'a App,
    key: &DetailKey,
    c: &Collection<'a>,
    width: f32,
) -> Element<'a, Message> {
    let rows = Rows {
        width,
        source: ListSource::Detail(key.clone()),
        numbered: c.numbered,
    };
    let mut list = column![];
    for (i, t) in c.tracks.iter().enumerate().take(app.rows_limit) {
        list = list.push(song_row::view(app, t, i, &rows));
    }
    if c.tracks.len() > app.rows_limit {
        list = list
            .push(vspace(gap::SM))
            .push(text_button("Show more", Message::ShowMore));
    }
    list.into()
}

/// The page while it loads, in the same layout: shimmering blocks for the cover, the text, the
/// two buttons and the rows.
pub fn skeleton(app: &App, content_w: u32) -> Element<'_, Message> {
    use super::skeleton_block as block;
    use crate::theme::shape;
    let p = app.shimmer;
    let left = |w: f32, art_px: f32| -> Element<'_, Message> {
        column![
            block(Length::Fixed(art_px), art_px, shape::lg(), p),
            vspace(gap::XL),
            block(Length::Fixed(w * 0.7), 28.0, 8.0, p),
            vspace(gap::MD),
            block(Length::Fixed(w * 0.4), 18.0, 9.0, p),
            vspace(gap::SM),
            block(Length::Fixed(w * 0.5), 14.0, 7.0, p),
            vspace(gap::XL),
            row![
                block(Length::Fixed(ROUND_BUTTON), ROUND_BUTTON, shape::FULL, p),
                Space::new().width(gap::LG),
                block(Length::Fixed(PLAY_DISC), PLAY_DISC, shape::FULL, p),
            ]
            .align_y(Alignment::Center),
        ]
        .align_x(Alignment::Center)
        .width(Length::Fixed(w))
        .into()
    };
    let rows = |w: f32| -> Element<'_, Message> {
        let mut c = column![];
        for i in 0..7_u8 {
            c = c.push(
                row![
                    block(Length::Fixed(44.0), 44.0, shape::sm(), p),
                    Space::new().width(gap::MD),
                    column![
                        block(
                            Length::Fixed(w * (0.42 + 0.06 * f32::from(i % 3))),
                            14.0,
                            7.0,
                            p
                        ),
                        vspace(gap::XS),
                        block(Length::Fixed(w * 0.28), 11.0, 6.0, p),
                    ],
                ]
                .align_y(Alignment::Center)
                .height(Length::Fixed(super::super::kit::ROW_H)),
            );
        }
        c.width(Length::Fixed(w)).into()
    };
    // Content widths are small exact numbers.
    #[allow(clippy::cast_precision_loss)]
    let body: Element<'_, Message> = match columns(app.window_px().0, content_w) {
        Columns::Two { left: l, list } => row![
            left(l as f32, l as f32),
            Space::new().width(gap::XXL),
            rows(list as f32)
        ]
        .into(),
        Columns::Stacked { art: a } => column![
            left(content_w as f32, a as f32),
            vspace(gap::XL),
            rows(content_w as f32)
        ]
        .into(),
    };
    container(body)
        .width(Length::Fill)
        .padding(Padding {
            top: gap::SM,
            ..Padding::ZERO
        })
        .center_x(Length::Fill)
        .into()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mock;

    #[test]
    fn two_columns_need_a_wide_window_and_a_wide_content_column() {
        assert!(matches!(columns(1920, 1000), Columns::Two { .. }));
        assert!(matches!(columns(1366, 677), Columns::Two { .. }));
        assert_eq!(columns(1279, 900), Columns::Stacked { art: STACKED_ART });
        assert_eq!(columns(1366, 600), Columns::Stacked { art: STACKED_ART });
        assert_eq!(
            columns(1000, 420),
            Columns::Stacked {
                art: STACKED_ART_NARROW
            }
        );
        for (w, c) in [(1280, 640), (1366, 677), (1920, 1000), (2560, 1200)] {
            let Columns::Two { left, list } = columns(w, c) else {
                panic!("{w} {c}");
            };
            assert!(
                (LEFT_MIN..=LEFT_MAX).contains(&left) && left % 4 == 0,
                "{left}"
            );
            assert_eq!(left + 32 + list, c);
            assert!(list >= 340, "{list}");
        }
    }

    #[test]
    fn meta_lines_show_only_what_the_catalog_has() {
        let tracks = mock::tracks().into_iter().take(4).collect::<Vec<_>>();
        assert_eq!(kind_line("Album", Some(2019)), "Album \u{2022} 2019");
        assert_eq!(kind_line("Playlist", None), "Playlist");
        let m = count_line(&tracks);
        assert!(m.starts_with("4 songs \u{2022} "), "{m}");
        assert!(m.ends_with(" min"), "{m}");
        assert_eq!(
            count_line(&tracks[..1]).split(" \u{2022} ").next(),
            Some("1 song")
        );
        let mut silent = tracks[0].clone();
        silent.duration = None;
        assert_eq!(count_line(&[silent]), "1 song");
    }
}
