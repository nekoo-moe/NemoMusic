//! Pieces shared by the screens: page frame, cards and grids, track rows, buttons, chips, empty
//! states. Every pressable piece is wrapped in [`fx`](super::super::fx) so it has an animated
//! state layer; cards also lift and enter with a stagger.

use iced::font::Weight;
use iced::widget::text::Wrapping;
use iced::widget::{Space, button, column, container, image, mouse_area, row, stack};
use iced::{Alignment, Background, Border, Color, Element, Length, Padding};

use super::super::app::{App, ArtShape, ListSource, Message};
use super::super::assets::Icon;
use super::super::fx::{Badge, BadgeAnchor, entrance, fx};
use super::super::style::{
    animated, button_flat, button_outlined, col, fit, fit_lines, gap, icon, icon_button, label,
    label_w, scroll, tip, vertical_gradient,
};
use crate::layout;
use crate::palette::Palette;
use crate::signal;
use crate::state::Event;
use crate::theme::{color, shape, type_scale};
use nemo_core::audio::Origin;
use nemo_core::track::{Artwork, Track};

/// Bottom padding of a scrolling page. The panes already end above the capsule, so this is only
/// breathing room under the last row.
pub const PAGE_BOTTOM: f32 = 16.0;
/// Track row height.
pub const ROW_H: f32 = 56.0;
/// Height of a Quick picks row.
pub const QUICK_H: f32 = 64.0;

/// The colour of the pane behind a page; entrances fade to it.
#[must_use]
pub fn pane_bg() -> Color {
    col(color::surface_container().with_alpha(crate::scheme::PANE_ALPHA))
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
        col(color::on_surface()),
    )
    .into()
}

/// `headline-md` page title.
pub fn page_title<'a>(text: impl Into<String>) -> Element<'a, Message> {
    label(
        text.into(),
        type_scale::HEADLINE_MD,
        true,
        col(color::on_surface()),
    )
    .into()
}

/// Small caps line above a title.
pub fn eyebrow<'a>(text: impl Into<String>) -> Element<'a, Message> {
    label_w(
        text.into().to_uppercase(),
        type_scale::LABEL_SM,
        Weight::Medium,
        col(color::on_surface_variant()),
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
        col(color::on_surface_variant().with_alpha(0.7)),
    ))
    .width(Length::Fixed(size))
    .height(Length::Fixed(size))
    .center_x(Length::Fixed(size))
    .center_y(Length::Fixed(size))
    .style(move |_| container::Style {
        background: Some(vertical_gradient(
            col(color::surface_container_highest()),
            col(color::surface_container_high()),
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
    /// Sent by the play button that fades in on hover; none for cards that cannot play.
    pub play: Option<Message>,
}

/// The hover play button in the cover's colours.
pub fn play_badge<'a>(
    app: &App,
    size: f32,
    anchor: BadgeAnchor,
    message: Message,
) -> Badge<'a, Message> {
    let accent = app.accent();
    Badge {
        size,
        anchor,
        fill: col(accent.primary),
        ink: col(accent.on_primary),
        glyph: Icon::Play.handle(),
        on_press: Box::new(move || message.clone()),
    }
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
                col(color::on_surface())
            ),
            label(
                fit(&card.subtitle, px, type_scale::BODY_SM.size),
                type_scale::BODY_SM,
                false,
                col(color::on_surface_variant())
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
            fx(b.style(button_flat(Color::TRANSPARENT, shape::xl())))
                .radius(shape::xl())
                .lift(4.0),
            progress,
            pane_bg(),
        );
    }
    let ink = col(color::on_surface());
    let text = column![
        // Two lines for a long title, broken at a space; never a cut inside a word.
        label(
            fit_lines(&card.title, px - 28.0, type_scale::TITLE_SM.size, 2),
            type_scale::TITLE_SM,
            true,
            ink
        ),
        label(
            fit(&card.subtitle, px - 28.0, type_scale::BODY_SM.size),
            type_scale::BODY_SM,
            false,
            col(color::on_surface().with_alpha(0.78))
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
    let radius = shape::scaled(super::super::app::TILE_RADIUS);
    let mut host = fx(b.style(button_flat(Color::TRANSPARENT, radius)))
        .radius(radius)
        .lift(4.0)
        .zoom(0.025);
    if let Some(play) = card.play {
        host = host.badge(play_badge(app, 44.0, BadgeAnchor::BottomRight(12.0), play));
    }
    entrance(host, progress, pane_bg())
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
        col(color::on_surface())
    } else {
        col(color::on_surface().with_alpha(0.38))
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
            col(color::surface_container_highest()),
            shape::FULL,
        ),
        None => fx(b.style(button_flat(
            col(color::surface_container_high()),
            shape::FULL,
        )))
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
            col(color::on_surface()),
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
    chip_padded(text, selected, accent, message, gap::MD + gap::XS)
}

/// [`chip`] with its own side padding (the narrow library pane uses 12).
pub fn chip_padded(
    text: &str,
    selected: bool,
    accent: Palette,
    message: Message,
    side: f32,
) -> Element<'_, Message> {
    let (fill, ink, outline) = if selected {
        (
            col(accent.secondary_container),
            col(accent.on_secondary_container),
            None,
        )
    } else {
        (
            Color::TRANSPARENT,
            col(color::on_surface()),
            Some(col(color::outline())),
        )
    };
    let b = button(
        container(label_w(text, type_scale::LABEL_MD, Weight::Medium, ink))
            .padding(Padding::from([0.0, side]))
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
    let dim = col(color::on_surface_variant());
    let mut c = column![
        icon(glyph, 32.0, dim),
        vspace(gap::MD),
        label(
            title.into(),
            type_scale::TITLE_MD,
            true,
            col(color::on_surface())
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
    track: &Track,
    index: usize,
    ctx: &RowCtx,
) -> Element<'a, Message> {
    let snap = &app.ui.snapshot;
    let current = snap.current.as_ref().map(|t| &t.id) == Some(&track.id);
    let dim = col(color::on_surface_variant());
    let ink = col(color::on_surface());
    let width = ctx.width;
    let accent = app.accent();

    let lead: Element<'a, Message> = if ctx.numbered {
        let inner: Element<'a, Message> = if current {
            eq_bars(app, col(accent.primary), 16.0)
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
    if current && !ctx.numbered {
        line = line
            .push(eq_bars(app, col(accent.primary), 16.0))
            .push(Space::new().width(gap::MD));
    }
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
    .on_press(Message::PlayAt {
        source: ctx.source.clone(),
        index,
    });
    let radius = shape::lg();
    let host = fx(main.style(button_flat(idle, radius)))
        .radius(radius)
        .badge(play_badge(
            app,
            32.0,
            BadgeAnchor::CenterLeft(if ctx.numbered { 6.0 } else { 14.0 }),
            Message::PlayAt {
                source: ctx.source.clone(),
                index,
            },
        ));
    let host = mouse_area(host).on_right_press(Message::Ui(Event::OpenMenu(track.clone())));
    row![
        host,
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
                    col(color::on_surface())
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
                col(color::on_surface_variant())
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
    .on_press(Message::PlayAt {
        source: source.clone(),
        index,
    });
    let source2 = source;
    let idle = if current {
        col(color::surface_container_high())
    } else {
        Color::TRANSPARENT
    };
    let radius = shape::lg();
    let host = fx(b.style(button_flat(idle, radius)))
        .radius(radius)
        .badge(play_badge(
            app,
            36.0,
            BadgeAnchor::CenterRight(12.0),
            Message::PlayAt {
                source: source2,
                index,
            },
        ));
    let host = mouse_area(host).on_right_press(Message::Ui(Event::OpenMenu(track.clone())));
    entrance(host, app.anim.reveal(stagger), pane_bg())
}

/// Rows of `tracks`, at most `limit` of them, then a "Show more" button when more exist.
pub fn track_list<'a>(
    app: &'a App,
    tracks: &[Track],
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
                radius: shape::lg().into(),
                ..Border::default()
            },
            ..container::Style::default()
        })
        .into()
}

/// Three equaliser bars on the playing row. They move while a song plays and rest at fixed
/// heights when paused. Purely a state indicator: it is not audio analysis.
pub fn eq_bars<'a>(app: &App, ink: Color, h: f32) -> Element<'a, Message> {
    let moving = app.ui.is_playing() && !app.ui.is_buffering() && !app.ui.reduced_motion;
    let mut r = row![].spacing(2.0).align_y(Alignment::End);
    for i in 0..3_u8 {
        let k = f32::from(i);
        let level = if moving {
            0.30 + 0.70 * (app.eq_phase * (1.0 + 0.41 * k) + k * 1.9).sin().abs()
        } else {
            [0.40, 0.70, 0.50][usize::from(i)]
        };
        r = r.push(
            container(Space::new())
                .width(Length::Fixed(3.0))
                .height(Length::Fixed((h * level).max(3.0)))
                .style(move |_| container::Style {
                    background: Some(Background::Color(ink)),
                    border: Border {
                        radius: 1.5.into(),
                        ..Border::default()
                    },
                    ..container::Style::default()
                }),
        );
    }
    container(r)
        .width(Length::Fixed(13.0))
        .height(Length::Fixed(h))
        .align_y(Alignment::End)
        .into()
}

/// Side padding of a chip, and the gap between chips.
pub const CHIP_SIDE: f32 = 16.0;
/// Gap between sliding chips.
pub const CHIP_GAP: f32 = 8.0;

/// Width of a chip with this label.
#[must_use]
pub fn chip_width(text: &str) -> f32 {
    super::super::textfit::width(text, 12.0).ceil() + 2.0 * CHIP_SIDE
}

/// Left edge and width of chip `selected` in a row of chips with these labels.
#[must_use]
pub fn chip_geometry(labels: &[&str], selected: usize) -> (f32, f32) {
    let mut x = 0.0;
    for (i, l) in labels.iter().enumerate() {
        let w = chip_width(l);
        if i == selected {
            return (x, w);
        }
        x += w + CHIP_GAP;
    }
    (0.0, 0.0)
}

/// A row of chips with one selection pill that slides from the old chip to the new one (the
/// pill follows two springs kept in the app, slot `slot`). `pick` maps an index to a message.
pub fn sliding_chips<'a>(
    app: &App,
    labels: &[&'a str],
    selected: usize,
    slot: usize,
    pick: fn(usize) -> Message,
) -> Element<'a, Message> {
    let accent = app.accent();
    let (x, w) = {
        let (sx, sw) = &app.chip_pills[slot];
        (sx.value, sw.value)
    };
    let pill = container(Space::new())
        .width(Length::Fixed(w.max(0.0)))
        .height(Length::Fixed(32.0))
        .style(move |_| container::Style {
            background: Some(Background::Color(col(accent.secondary_container))),
            border: Border {
                radius: shape::FULL.into(),
                ..Border::default()
            },
            ..container::Style::default()
        });
    let pill_layer = row![Space::new().width(Length::Fixed(x.max(0.0))), pill];
    let mut chips = row![].spacing(CHIP_GAP);
    for (i, text) in labels.iter().enumerate() {
        let on = i == selected;
        let (ink, outline) = if on {
            (col(accent.on_secondary_container), None)
        } else {
            (col(color::on_surface()), Some(col(color::outline())))
        };
        let b = button(
            container(label_w(*text, type_scale::LABEL_MD, Weight::Medium, ink))
                .width(Length::Fixed(chip_width(text)))
                .height(Length::Fixed(32.0))
                .center_x(Length::Fixed(chip_width(text)))
                .align_y(Alignment::Center),
        )
        .padding(0)
        .on_press(pick(i));
        chips = chips.push(
            fx(b.style(button_outlined(Color::TRANSPARENT, outline, shape::FULL)))
                .layer(ink)
                .radius(shape::FULL),
        );
    }
    // A stack takes the size of its first layer, so give it the width of the whole row.
    let total: f32 = labels.iter().map(|t| chip_width(t)).sum::<f32>()
        + CHIP_GAP * f32::from(u8::try_from(labels.len().saturating_sub(1)).unwrap_or(0));
    stack![pill_layer, chips]
        .width(Length::Fixed(total))
        .height(Length::Fixed(32.0))
        .into()
}
