//! Pieces shared by the screens: page frame, cards and grids, track rows, buttons, chips, empty
//! states. Every pressable piece is wrapped in [`fx`](super::super::fx) so it has an animated
//! state layer; cards also lift and enter with a stagger.

use iced::font::Weight;
use iced::widget::text::Wrapping;
use iced::widget::{Space, button, column, container, image, row, stack};
use iced::{Alignment, Background, Border, Color, Element, Length, Padding};

use super::super::app::{App, ArtShape, ListSource, Message};
use super::super::assets::Icon;
use super::super::fx::{entrance, fx};
use super::super::style::{
    animated, button_flat, button_outlined, col, fit, gap, icon, icon_button, label, label_w,
    scroll, tip, vertical_gradient,
};
use crate::layout;
use crate::palette::Palette;
use crate::signal;
use crate::state::Event;
use crate::theme::{color, shape, type_scale};
use nemo_core::audio::Origin;
use nemo_core::track::{Artwork, Track};

/// Bottom padding of a page so the floating capsule never hides the last row.
#[allow(clippy::cast_precision_loss)] // 96 is exact in f32
pub const PAGE_BOTTOM: f32 = layout::PANE_BOTTOM_RESERVE as f32;
/// Track row height.
pub const ROW_H: f32 = 56.0;
/// Height of a Quick picks row.
pub const QUICK_H: f32 = 64.0;

/// The colour of the pane behind a page; entrances fade to it.
#[must_use]
pub fn pane_bg() -> Color {
    col(color::SURFACE_CONTAINER)
}

/// A scrolling page: a column `content_w` px wide, centred, with a thin scroll bar and bottom
/// padding for the capsule.
pub fn page<'a>(content_w: u32, body: impl Into<Element<'a, Message>>) -> Element<'a, Message> {
    // Content widths are small exact numbers.
    #[allow(clippy::cast_precision_loss)]
    let width = content_w as f32;
    let framed = container(column![body.into()].width(Length::Fixed(width)))
        .width(Length::Fill)
        .padding(Padding {
            top: gap::SM,
            bottom: PAGE_BOTTOM,
            ..Padding::ZERO
        })
        .center_x(Length::Fill);
    scroll(framed)
        .width(Length::Fill)
        .height(Length::Fill)
        .into()
}

/// `title-lg` semibold section title.
pub fn section_title<'a>(text: impl Into<String>) -> Element<'a, Message> {
    label(
        text.into(),
        type_scale::TITLE_LG,
        true,
        col(color::ON_SURFACE),
    )
    .into()
}

/// `headline-md` page title.
pub fn page_title<'a>(text: impl Into<String>) -> Element<'a, Message> {
    label(
        text.into(),
        type_scale::HEADLINE_MD,
        true,
        col(color::ON_SURFACE),
    )
    .into()
}

/// Small caps line above a title.
pub fn eyebrow<'a>(text: impl Into<String>) -> Element<'a, Message> {
    label_w(
        text.into().to_uppercase(),
        type_scale::LABEL_SM,
        Weight::Medium,
        col(color::ON_SURFACE_VARIANT),
    )
    .into()
}

/// Wrapped paragraph of `width` px.
pub fn paragraph<'a>(
    text: impl Into<String>,
    style: crate::theme::TextStyle,
    ink: Color,
    width: f32,
) -> Element<'a, Message> {
    label(text.into(), style, false, ink)
        .wrapping(Wrapping::Word)
        .width(Length::Fixed(width))
        .into()
}

/// Vertical space.
pub fn vspace<'a>(h: f32) -> Element<'a, Message> {
    Space::new().height(h).into()
}

/// Neutral placeholder with a note glyph (`design.md` section 9).
pub fn art_placeholder<'a>(size: f32, radius: f32, loading: bool) -> Element<'a, Message> {
    let _ = loading;
    container(icon(
        Icon::Note,
        (size / 3.0).clamp(16.0, 56.0),
        col(color::ON_SURFACE_VARIANT.with_alpha(0.7)),
    ))
    .width(Length::Fixed(size))
    .height(Length::Fixed(size))
    .center_x(Length::Fixed(size))
    .center_y(Length::Fixed(size))
    .style(move |_| container::Style {
        background: Some(vertical_gradient(
            col(color::SURFACE_CONTAINER_HIGHEST),
            col(color::SURFACE_CONTAINER_HIGH),
        )),
        border: Border {
            radius: radius.into(),
            ..Border::default()
        },
        ..container::Style::default()
    })
    .into()
}

/// A picture in a `size` px slot: the cached image or the placeholder.
pub fn art<'a>(
    app: &App,
    artwork: &Artwork,
    size: u32,
    art_shape: ArtShape,
) -> Element<'a, Message> {
    // Slot sizes are small exact numbers.
    #[allow(clippy::cast_precision_loss)]
    let px = size as f32;
    if let Some(handle) = app.art_handle(artwork, size, art_shape) {
        return image(handle)
            .width(Length::Fixed(px))
            .height(Length::Fixed(px))
            .content_fit(iced::ContentFit::Fill)
            .into();
    }
    art_placeholder(px, art_shape.radius(px), true)
}

/// What a card shows.
pub struct Card {
    /// Picture source.
    pub artwork: Artwork,
    /// Title.
    pub title: String,
    /// Subtitle.
    pub subtitle: String,
    /// Picture shape.
    pub shape: ArtShape,
    /// Pressed.
    pub on_press: Message,
}

fn scrim_style(radius: f32) -> impl Fn(&iced::Theme) -> container::Style {
    move |_| container::Style {
        background: Some(Background::Gradient(
            iced::gradient::Linear::new(iced::Radians(super::super::style::DOWN))
                .add_stop(0.0, Color::from_rgba(0.0, 0.0, 0.0, 0.0))
                .add_stop(0.52, Color::from_rgba(0.0, 0.0, 0.0, 0.0))
                .add_stop(1.0, Color::from_rgba(0.0, 0.0, 0.0, 0.82))
                .into(),
        )),
        border: Border {
            radius: radius.into(),
            ..Border::default()
        },
        ..container::Style::default()
    }
}

/// A large card: the art fills it, the title and subtitle sit on a bottom scrim, and it lifts on
/// hover. Artists get a round picture with the text below. `stagger` is its place in the
/// entrance order.
pub fn card_view<'a>(app: &App, card: Card, size: u32, stagger: usize) -> Element<'a, Message> {
    // Card sizes are small exact numbers.
    #[allow(clippy::cast_precision_loss)]
    let px = size as f32;
    let progress = app.anim.reveal(stagger);
    if card.shape == ArtShape::Circle {
        let text = column![
            label(
                fit(&card.title, px, type_scale::BODY_MD.size),
                type_scale::BODY_MD,
                true,
                col(color::ON_SURFACE)
            ),
            label(
                fit(&card.subtitle, px, type_scale::BODY_SM.size),
                type_scale::BODY_SM,
                false,
                col(color::ON_SURFACE_VARIANT)
            ),
        ]
        .width(Length::Fixed(px))
        .align_x(Alignment::Center);
        let b = button(
            column![
                art(app, &card.artwork, size, ArtShape::Circle),
                vspace(gap::MD),
                text
            ]
            .width(Length::Fixed(px))
            .align_x(Alignment::Center),
        )
        .padding(0)
        .on_press(card.on_press);
        return entrance(
            fx(b.style(button_flat(Color::TRANSPARENT, shape::XL)))
                .radius(shape::XL)
                .lift(4.0),
            progress,
            pane_bg(),
        );
    }
    let ink = col(color::ON_SURFACE);
    let text = column![
        label(
            fit(&card.title, px - 28.0, type_scale::TITLE_SM.size),
            type_scale::TITLE_SM,
            true,
            ink
        ),
        label(
            fit(&card.subtitle, px - 28.0, type_scale::BODY_SM.size),
            type_scale::BODY_SM,
            false,
            col(color::ON_SURFACE.with_alpha(0.78))
        ),
    ];
    let overlay = container(text)
        .width(Length::Fixed(px))
        .height(Length::Fixed(px))
        .align_y(Alignment::End)
        .padding(Padding {
            top: 0.0,
            right: gap::MD,
            bottom: gap::MD,
            left: gap::MD,
        })
        .style(scrim_style(super::super::app::TILE_RADIUS));
    let b = button(stack![
        art(app, &card.artwork, size, ArtShape::Tile),
        overlay
    ])
    .padding(0)
    .on_press(card.on_press);
    entrance(
        fx(b.style(button_flat(
            Color::TRANSPARENT,
            super::super::app::TILE_RADIUS,
        )))
        .radius(super::super::app::TILE_RADIUS)
        .lift(4.0),
        progress,
        pane_bg(),
    )
}

/// Cards in rows of `grid.columns`, at most `max_rows` rows. `base` is the stagger index of the
/// first card.
pub fn card_grid<'a>(
    app: &App,
    grid: layout::Grid,
    cards: Vec<Card>,
    max_rows: usize,
    base: usize,
) -> Element<'a, Message> {
    // Grid numbers are small exact numbers.
    #[allow(clippy::cast_precision_loss)]
    let gap_px = grid.gap as f32;
    let per_row = grid.columns as usize;
    let mut rows = column![].spacing(gap::XL);
    let mut it = cards
        .into_iter()
        .take(per_row * max_rows)
        .enumerate()
        .peekable();
    while it.peek().is_some() {
        let line: Vec<Element<'a, Message>> = it
            .by_ref()
            .take(per_row)
            .map(|(i, c)| card_view(app, c, grid.card, base + i))
            .collect();
        rows = rows.push(row(line).spacing(gap_px));
    }
    rows.into()
}

/// A shelf header: title on the left.
pub fn shelf_header<'a>(title: impl Into<String>) -> Element<'a, Message> {
    column![section_title(title), vspace(gap::MD)].into()
}

/// Filled pill button in the accent colour (the one primary action of a region).
pub fn filled_button(
    text: &str,
    glyph: Icon,
    accent: Palette,
    message: Message,
) -> Element<'_, Message> {
    let b = button(
        row![
            icon(glyph, 20.0, col(accent.on_primary)),
            Space::new().width(gap::SM),
            label_w(
                text,
                type_scale::BODY_MD,
                Weight::Medium,
                col(accent.on_primary)
            ),
        ]
        .align_y(Alignment::Center)
        .padding(Padding::from([0.0, gap::XL]))
        .height(Length::Fixed(48.0)),
    )
    .padding(0)
    .on_press(message);
    fx(b.style(button_flat(col(accent.primary), shape::FULL)))
        .layer(col(accent.on_primary))
        .radius(shape::FULL)
        .into()
}

/// Tonal pill button.
pub fn tonal_button(
    text: &str,
    glyph: Option<Icon>,
    message: Option<Message>,
) -> Element<'_, Message> {
    let enabled = message.is_some();
    let ink = if enabled {
        col(color::ON_SURFACE)
    } else {
        col(color::ON_SURFACE.with_alpha(0.38))
    };
    let mut content = row![].align_y(Alignment::Center);
    if let Some(g) = glyph {
        content = content
            .push(icon(g, 20.0, ink))
            .push(Space::new().width(gap::SM));
    }
    content = content.push(label_w(text, type_scale::BODY_MD, Weight::Medium, ink));
    let b = button(
        content
            .padding(Padding::from([0.0, gap::XL]))
            .height(Length::Fixed(48.0)),
    )
    .padding(0);
    match message {
        Some(m) => animated(
            b.on_press(m),
            col(color::SURFACE_CONTAINER_HIGHEST),
            shape::FULL,
        ),
        None => fx(b.style(button_flat(col(color::SURFACE_CONTAINER_HIGH), shape::FULL)))
            .passive()
            .into(),
    }
}

/// Small text button (height 40).
pub fn text_button(text: &str, message: Message) -> Element<'_, Message> {
    let b = button(
        container(label_w(
            text,
            type_scale::BODY_MD,
            Weight::Medium,
            col(color::ON_SURFACE),
        ))
        .padding(Padding::from([0.0, gap::LG]))
        .height(Length::Fixed(40.0))
        .align_y(Alignment::Center),
    )
    .padding(0)
    .on_press(message);
    animated(b, Color::TRANSPARENT, shape::FULL)
}

/// A filter chip, height 32: outlined when off, the cover's `secondary-container` when on.
pub fn chip(text: &str, selected: bool, accent: Palette, message: Message) -> Element<'_, Message> {
    let (fill, ink, outline) = if selected {
        (
            col(accent.secondary_container),
            col(accent.on_secondary_container),
            None,
        )
    } else {
        (
            Color::TRANSPARENT,
            col(color::ON_SURFACE),
            Some(col(color::OUTLINE)),
        )
    };
    let b = button(
        container(label_w(text, type_scale::LABEL_MD, Weight::Medium, ink))
            .padding(Padding::from([0.0, gap::MD + gap::XS]))
            .height(Length::Fixed(32.0))
            .align_y(Alignment::Center),
    )
    .padding(0)
    .on_press(message);
    fx(b.style(button_outlined(fill, outline, shape::FULL)))
        .layer(ink)
        .radius(shape::FULL)
        .into()
}

/// A calm status block: glyph, title, detail and an optional action. Never red.
pub fn status(
    glyph: Icon,
    title: impl Into<String>,
    detail: impl Into<String>,
    action: Option<(&str, Message)>,
) -> Element<'_, Message> {
    let dim = col(color::ON_SURFACE_VARIANT);
    let mut c = column![
        icon(glyph, 32.0, dim),
        vspace(gap::MD),
        label(
            title.into(),
            type_scale::TITLE_MD,
            true,
            col(color::ON_SURFACE)
        ),
        vspace(gap::XS),
        paragraph(detail, type_scale::BODY_MD, dim, 420.0),
    ]
    .align_x(Alignment::Center);
    if let Some((text, message)) = action {
        c = c
            .push(vspace(gap::LG))
            .push(tonal_button(text, None, Some(message)));
    }
    container(c)
        .width(Length::Fill)
        .padding(Padding::from([gap::XXXL, gap::XL]))
        .center_x(Length::Fill)
        .into()
}

/// Where a row sits and what it shows.
pub struct RowCtx {
    /// Row width in px.
    pub width: f32,
    /// The list the rows belong to.
    pub source: ListSource,
    /// Show the position number instead of the cover.
    pub numbered: bool,
    /// Show the album column.
    pub show_album: bool,
}

/// One track row, 56 px high: art or number, title and artist, album, badge, duration, menu.
pub fn track_row<'a>(
    app: &'a App,
    track: &'a Track,
    index: usize,
    ctx: &RowCtx,
) -> Element<'a, Message> {
    let snap = &app.ui.snapshot;
    let current = snap.current.as_ref().map(|t| &t.id) == Some(&track.id);
    let dim = col(color::ON_SURFACE_VARIANT);
    let ink = col(color::ON_SURFACE);
    let width = ctx.width;
    let accent = app.accent();

    let lead: Element<'a, Message> = if ctx.numbered {
        let inner: Element<'a, Message> = if current {
            icon(Icon::Volume, 20.0, col(accent.primary)).into()
        } else {
            label(format!("{}", index + 1), type_scale::BODY_MD, false, dim).into()
        };
        container(inner)
            .width(Length::Fixed(44.0))
            .height(Length::Fixed(44.0))
            .center_x(Length::Fixed(44.0))
            .center_y(Length::Fixed(44.0))
            .into()
    } else {
        art(app, &track.artwork, 44, ArtShape::Row)
    };

    let show_badge = width >= 640.0;
    let show_album = ctx.show_album && width >= 760.0;
    let badge_w = if show_badge { 96.0 + gap::MD } else { 0.0 };
    // lead 44 + gap 12, duration 56, menu 40, row padding 2 x 8.
    let fixed = 44.0 + gap::MD + 56.0 + 40.0 + 2.0 * gap::SM + badge_w;
    let flex = (width - fixed).max(120.0);
    let (title_w, album_w) = if show_album {
        (flex * 0.55 - gap::MD, flex * 0.45)
    } else {
        (flex, 0.0)
    };

    let mut line = row![
        lead,
        Space::new().width(gap::MD),
        column![
            label(
                fit(&track.title, title_w, type_scale::BODY_MD.size),
                type_scale::BODY_MD,
                true,
                if current { col(accent.primary) } else { ink }
            ),
            label(
                fit(
                    &crate::format::artists(&track.artists),
                    title_w,
                    type_scale::BODY_SM.size
                ),
                type_scale::BODY_SM,
                false,
                dim
            ),
        ]
        .width(Length::Fixed(title_w)),
    ]
    .align_y(Alignment::Center);
    if show_album {
        line = line.push(Space::new().width(gap::MD)).push(
            container(label(
                fit(
                    track.album.as_deref().unwrap_or(""),
                    album_w,
                    type_scale::BODY_MD.size,
                ),
                type_scale::BODY_MD,
                false,
                dim,
            ))
            .width(Length::Fixed(album_w)),
        );
    }
    line = line.push(Space::new().width(Length::Fill));
    if show_badge {
        let text = if current {
            snap.signal_path
                .as_ref()
                .map_or_else(|| "Unknown".to_owned(), |p| p.source.badge())
        } else {
            match track.origin {
                Origin::Local => "Local".to_owned(),
                Origin::YoutubeMusic => "YouTube".to_owned(),
            }
        };
        line = line.push(
            container(label(
                fit(&text, 96.0, type_scale::LABEL_SM.size),
                type_scale::LABEL_SM,
                false,
                dim,
            ))
            .width(Length::Fixed(96.0 + gap::MD))
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
        .width(Length::Fixed(56.0))
        .align_x(Alignment::End),
    );

    let idle = if current {
        col(color::SURFACE_CONTAINER_HIGH)
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
    .on_press(Message::PlayAt {
        source: ctx.source.clone(),
        index,
    });
    row![
        animated(main, idle, shape::LG),
        tip(
            icon_button(
                Icon::More,
                20.0,
                40.0,
                dim,
                Color::TRANSPARENT,
                Some(Message::Ui(Event::OpenMenu(track.clone()))),
            ),
            "More",
        ),
    ]
    .align_y(Alignment::Center)
    .height(Length::Fixed(ROW_H))
    .into()
}

/// A "Quick picks" row: art 56, title and artist; the playing song is tinted.
pub fn quick_pick<'a>(
    app: &'a App,
    track: &'a Track,
    source: ListSource,
    index: usize,
    width: f32,
    stagger: usize,
) -> Element<'a, Message> {
    let current = app.ui.snapshot.current.as_ref().map(|t| &t.id) == Some(&track.id);
    let accent = app.accent();
    let text_w = (width - 56.0 - gap::MD - 2.0 * gap::SM - gap::SM).max(80.0);
    let line = row![
        art(app, &track.artwork, 56, ArtShape::Thumb),
        Space::new().width(gap::MD),
        column![
            label(
                fit(&track.title, text_w, type_scale::BODY_MD.size),
                type_scale::BODY_MD,
                true,
                if current {
                    col(accent.primary)
                } else {
                    col(color::ON_SURFACE)
                }
            ),
            label(
                fit(
                    &crate::format::artists(&track.artists),
                    text_w,
                    type_scale::BODY_SM.size
                ),
                type_scale::BODY_SM,
                false,
                col(color::ON_SURFACE_VARIANT)
            ),
        ]
        .width(Length::Fixed(text_w)),
    ]
    .align_y(Alignment::Center);
    let b = button(
        container(line)
            .width(Length::Fixed(width))
            .height(Length::Fixed(QUICK_H))
            .padding(Padding::from([0.0, gap::SM]))
            .align_y(Alignment::Center),
    )
    .padding(0)
    .on_press(Message::PlayAt { source, index });
    let idle = if current {
        col(color::SURFACE_CONTAINER_HIGH)
    } else {
        Color::TRANSPARENT
    };
    entrance(
        animated(b, idle, shape::LG),
        app.anim.reveal(stagger),
        pane_bg(),
    )
}

/// Rows of `tracks`, at most `limit` of them, then a "Show more" button when more exist.
pub fn track_list<'a>(
    app: &'a App,
    tracks: &'a [Track],
    limit: usize,
    ctx: &RowCtx,
) -> Element<'a, Message> {
    let mut c = column![];
    for (i, t) in tracks.iter().enumerate().take(limit) {
        c = c.push(entrance(
            track_row(app, t, i, ctx),
            app.anim.reveal(i.min(6)),
            pane_bg(),
        ));
    }
    if tracks.len() > limit {
        c = c
            .push(vspace(gap::SM))
            .push(text_button("Show more", Message::ShowMore));
    }
    c.into()
}

/// The accent-free badge text of the current signal path.
#[must_use]
pub fn badge_text(app: &App) -> String {
    signal::badge_label(app.ui.snapshot.signal_path.as_ref())
}

/// A rounded container with a flat background.
pub fn tile<'a>(
    content: impl Into<Element<'a, Message>>,
    bg: crate::theme::Rgba,
) -> Element<'a, Message> {
    container(content)
        .style(move |_| container::Style {
            background: Some(Background::Color(col(bg))),
            border: Border {
                radius: shape::LG.into(),
                ..Border::default()
            },
            ..container::Style::default()
        })
        .into()
}
