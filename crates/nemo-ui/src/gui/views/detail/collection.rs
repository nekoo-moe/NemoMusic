//! The album and playlist page in the YouTube Music layout (collection-page-ytm spec): a tall
//! gradient from the page cover's colour behind everything; a centred left column with the cover,
//! the title, the owner, the meta lines and round Shuffle and Play buttons; the track list on the
//! right. Two columns from a 1280 px window when the content is wide enough; stacked otherwise.
//! In two columns each column scrolls on its own, so the cover and the buttons stay in view.

use iced::alignment::Horizontal;
use iced::widget::text::Wrapping;
use iced::widget::{Space, column, container, row, stack};
use iced::{Alignment, Background, Element, Length, Padding};
use nemo_core::catalog::{CollectionMeta, Privacy};
use nemo_core::track::{Artwork, Track};

use super::super::kit::{PAGE_BOTTOM, Skeleton, art, text_button, tonal_button, vspace};
use super::meta_line::{count_line, kind_line};
use super::song_row::{self, Rows};
use super::sort_menu;
use crate::collection_sort;
use crate::gui::app::{App, ArtShape, DetailKey, ListSource, Message, ScWriteMsg};
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
    /// Who can see the playlist, when known.
    pub privacy: Option<Privacy>,
    /// The other header facts (owner picture, views, count, length).
    pub meta: &'a CollectionMeta,
    /// The songs, in the page's own order.
    pub tracks: &'a [Track],
    /// Album page: numbered rows without covers, `songs` in the count line.
    pub numbered: bool,
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

/// Size of the owner picture beside the owner's name.
const AVATAR: u32 = 24;

/// The owner's name, with their picture before it when the header gives one.
fn owner_line<'a>(
    app: &'a App,
    c: &Collection<'a>,
    owner: &str,
    width: f32,
) -> Element<'a, Message> {
    let ink = col(color::on_surface());
    let avatar = (!c.meta.owner_avatar.is_empty()).then(|| {
        art(
            app,
            &Artwork::Remote(c.meta.owner_avatar.clone()),
            AVATAR,
            ArtShape::Circle,
        )
    });
    // The avatar slot is a small exact number.
    #[allow(clippy::cast_precision_loss)]
    let name_w = if avatar.is_some() {
        width - AVATAR as f32 - gap::SM
    } else {
        width
    };
    let name = label(
        fit(owner, name_w, type_scale::TITLE_SM.size),
        type_scale::TITLE_SM,
        true,
        ink,
    );
    match avatar {
        Some(a) => row![a, Space::new().width(gap::SM), name]
            .align_y(Alignment::Center)
            .into(),
        None => name.into(),
    }
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
        info = info
            .push(vspace(gap::SM))
            .push(owner_line(app, c, owner, width));
    }
    info = info
        .push(vspace(gap::SM))
        .push(centred(label(
            kind_line(c.kind, c.privacy, c.year),
            type_scale::BODY_MD,
            false,
            dim,
        )))
        .push(centred(label(
            count_line(c.meta, c.tracks, c.numbered),
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
    let mut info = info
        .push(vspace(gap::XL))
        .push(row![shuffle, Space::new().width(gap::LG), play].align_y(Alignment::Center));
    // A playlist the user made on SoundCloud can be renamed from its page.
    if app.sc_can_write() && app.sc_owns_playlist(&key.id) {
        info = info.push(vspace(gap::LG)).push(tonal_button(
            "Rename",
            None,
            Some(Message::ScWrite(ScWriteMsg::Rename(key.id.clone()))),
        ));
    }
    info.into()
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
    // Rows in the order the Sort control shows; row `k` plays as index `k` of the same order
    // (`App::tracks_of`), so Play and Shuffle follow what the user sees.
    let order = collection_sort::order(c.tracks, app.collection_sort(key));
    let mut list = column![sort_menu::view(app, key, c.numbered), vspace(gap::SM)];
    for (k, &i) in order.iter().enumerate().take(app.rows_limit) {
        list = list.push(song_row::view(app, &c.tracks[i], k, &rows));
    }
    if c.tracks.len() > app.rows_limit {
        list = list
            .push(vspace(gap::SM))
            .push(text_button("Show more", Message::ShowMore));
    }
    list.into()
}

/// The page while it loads, in the same layout: blocks for the cover, the text, the two buttons
/// and the rows.
pub fn skeleton(app: &App, sk: Skeleton, content_w: u32) -> Element<'_, Message> {
    use crate::theme::shape;
    let left = |w: f32, art_px: f32| -> Element<'_, Message> {
        column![
            sk.block(Length::Fixed(art_px), art_px, shape::lg()),
            vspace(gap::XL),
            sk.line(w * 0.7, type_scale::HEADLINE_MD),
            vspace(gap::SM),
            sk.line(w * 0.4, type_scale::BODY_MD),
            vspace(gap::XS),
            sk.line(w * 0.5, type_scale::BODY_SM),
            vspace(gap::XL),
            row![
                sk.circle(ROUND_BUTTON),
                Space::new().width(gap::LG),
                sk.circle(PLAY_DISC),
            ]
            .align_y(Alignment::Center),
        ]
        .align_x(Alignment::Center)
        .width(Length::Fixed(w))
        .into()
    };
    let rows = |w: f32| -> Element<'_, Message> {
        let mut c = column![];
        for i in 0..7_usize {
            c = c.push(sk.row(w, i));
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
}
