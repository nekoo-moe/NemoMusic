//! Pieces shared by the screens: page frame, cards and grids, track rows, buttons, empty states.

use iced::widget::text::Wrapping;
use iced::widget::{Space, button, column, container, image, row, scrollable};
use iced::{Alignment, Background, Border, Color, Element, Length, Padding};

use super::super::app::{App, ArtShape, ListSource, Message};
use super::super::assets::Icon;
use super::super::style::{button_style, col, fit, gap, icon, icon_button, label, tip};
use crate::layout;
use crate::palette::Palette;
use crate::signal;
use crate::state::Event;
use crate::theme::{color, shape, type_scale};
use nemo_core::audio::Origin;
use nemo_core::track::{Artwork, Track};

/// Bottom padding of a page so the floating bar never hides the last row.
#[allow(clippy::cast_precision_loss)] // 112 is exact in f32
pub const PAGE_BOTTOM: f32 = layout::CONTENT_BOTTOM_PADDING as f32;
/// Track row height.
#[allow(clippy::cast_precision_loss)] // 48 is exact in f32
pub const ROW_H: f32 = layout::TRACK_ROW as f32;

/// A scrolling page: a column `content` px wide, centred, with top padding 24 and bottom 112.
pub fn page<'a>(content_w: u32, body: impl Into<Element<'a, Message>>) -> Element<'a, Message> {
    // Content widths are small exact numbers.
    #[allow(clippy::cast_precision_loss)]
    let width = content_w as f32;
    let framed = container(column![body.into()].width(Length::Fixed(width)))
        .width(Length::Fill)
        .padding(Padding {
            top: gap::XL,
            bottom: PAGE_BOTTOM,
            ..Padding::ZERO
        })
        .center_x(Length::Fill);
    scrollable(framed)
        .width(Length::Fill)
        .height(Length::Fill)
        .into()
}

/// `title-md` semibold section title.
pub fn section_title<'a>(text: impl Into<String>) -> Element<'a, Message> {
    label(
        text.into(),
        type_scale::TITLE_MD,
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
        (size / 4.0).clamp(16.0, 48.0),
        col(color::ON_SURFACE_VARIANT),
    ))
    .width(Length::Fixed(size))
    .height(Length::Fixed(size))
    .center_x(Length::Fixed(size))
    .center_y(Length::Fixed(size))
    .style(move |_| container::Style {
        background: Some(col(color::SURFACE_CONTAINER).into()),
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
    let radius = match art_shape {
        ArtShape::Circle => px / 2.0,
        ArtShape::Card => shape::MD,
        ArtShape::Row => shape::SM,
        ArtShape::Hero => shape::LG,
    };
    art_placeholder(px, radius, true)
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

/// A card: square art, title 14/20 semibold, subtitle 12/18 (`ui-reference.md` section 3).
pub fn card_view<'a>(app: &App, card: Card, size: u32) -> Element<'a, Message> {
    // Card sizes are small exact numbers.
    #[allow(clippy::cast_precision_loss)]
    let px = size as f32;
    let round = card.shape == ArtShape::Circle;
    let align = if round {
        Alignment::Center
    } else {
        Alignment::Start
    };
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
    .align_x(align);
    button(
        column![
            art(app, &card.artwork, size, card.shape),
            vspace(gap::SM),
            text
        ]
        .width(Length::Fixed(px))
        .align_x(align),
    )
    .padding(0)
    .style(button_style(Color::TRANSPARENT, shape::MD))
    .on_press(card.on_press)
    .into()
}

/// Cards in rows of `grid.columns`, at most `max_rows` rows.
pub fn card_grid<'a>(
    app: &App,
    grid: layout::Grid,
    cards: Vec<Card>,
    max_rows: usize,
) -> Element<'a, Message> {
    // Grid numbers are small exact numbers.
    #[allow(clippy::cast_precision_loss)]
    let gap_px = grid.gap as f32;
    let per_row = grid.columns as usize;
    let mut rows = column![].spacing(gap::XL);
    let mut it = cards.into_iter().take(per_row * max_rows).peekable();
    while it.peek().is_some() {
        let line: Vec<Element<'a, Message>> = it
            .by_ref()
            .take(per_row)
            .map(|c| card_view(app, c, grid.card))
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
    button(
        row![
            icon(glyph, 20.0, col(accent.on_primary)),
            Space::new().width(gap::SM),
            label(text, type_scale::BODY_MD, true, col(accent.on_primary)),
        ]
        .align_y(Alignment::Center)
        .padding(Padding::from([0.0, gap::XL]))
        .height(Length::Fixed(48.0)),
    )
    .padding(0)
    .style(button_style(col(accent.primary), shape::FULL))
    .on_press(message)
    .into()
}

/// Tonal pill button.
pub fn tonal_button(
    text: &str,
    glyph: Option<Icon>,
    message: Option<Message>,
) -> Element<'_, Message> {
    let ink = col(color::ON_SURFACE);
    let mut content = row![].align_y(Alignment::Center);
    if let Some(g) = glyph {
        content = content
            .push(icon(g, 20.0, ink))
            .push(Space::new().width(gap::SM));
    }
    content = content.push(label(text, type_scale::BODY_MD, true, ink));
    let b = button(
        content
            .padding(Padding::from([0.0, gap::XL]))
            .height(Length::Fixed(48.0)),
    )
    .padding(0)
    .style(button_style(
        col(color::SURFACE_CONTAINER_HIGH),
        shape::FULL,
    ));
    match message {
        Some(m) => b.on_press(m).into(),
        None => b.into(),
    }
}

/// Small text button (height 40) in `on-surface-variant`.
pub fn text_button(text: &str, message: Message) -> Element<'_, Message> {
    button(
        container(label(
            text,
            type_scale::BODY_MD,
            true,
            col(color::ON_SURFACE),
        ))
        .padding(Padding::from([0.0, gap::LG]))
        .height(Length::Fixed(40.0))
        .align_y(Alignment::Center),
    )
    .padding(0)
    .style(button_style(Color::TRANSPARENT, shape::FULL))
    .on_press(message)
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

/// One track row, 48 px high: art or number, title and artist, album, badge, duration, menu.
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

    let lead: Element<'a, Message> = if ctx.numbered {
        let inner: Element<'a, Message> = if current {
            icon(Icon::Volume, 16.0, ink).into()
        } else {
            label(format!("{}", index + 1), type_scale::BODY_MD, false, dim).into()
        };
        container(inner)
            .width(Length::Fixed(40.0))
            .height(Length::Fixed(40.0))
            .center_x(Length::Fixed(40.0))
            .center_y(Length::Fixed(40.0))
            .into()
    } else {
        art(app, &track.artwork, 40, ArtShape::Row)
    };

    let show_badge = width >= 640.0;
    let show_album = ctx.show_album && width >= 760.0;
    let badge_w = if show_badge { 96.0 + gap::MD } else { 0.0 };
    // lead 40 + gap 12, duration 56, menu 40, row padding 2 x 8.
    let fixed = 40.0 + gap::MD + 56.0 + 40.0 + 2.0 * gap::SM + badge_w;
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
                ink
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
    .style(button_style(idle, shape::MD))
    .on_press(Message::PlayAt {
        source: ctx.source.clone(),
        index,
    });
    row![
        main,
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

/// Rows of `tracks`, at most `limit` of them, then a "Show more" button when more exist.
pub fn track_list<'a>(
    app: &'a App,
    tracks: &'a [Track],
    limit: usize,
    ctx: &RowCtx,
) -> Element<'a, Message> {
    let mut c = column![];
    for (i, t) in tracks.iter().enumerate().take(limit) {
        c = c.push(track_row(app, t, i, ctx));
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
                radius: shape::MD.into(),
                ..Border::default()
            },
            ..container::Style::default()
        })
        .into()
}
