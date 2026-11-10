//! Home: shelves of cards from `catalog.search`, laid out with [`crate::layout::grid`].

use iced::widget::{Space, button, column, container, image, row, scrollable};
use iced::{Alignment, Element, Length, Padding};

use super::super::app::{App, ArtState, HomeData, Message, art_key};
use super::super::assets::Icon;
use super::super::style::{button_style, col, fit, gap, icon, label};
use crate::layout;
use crate::route::Route;
use crate::state::Event;
use crate::theme::{color, shape, type_scale};
use nemo_core::track::Artwork;

/// The Home screen.
pub fn view(app: &App, w: u32) -> Element<'_, Message> {
    let grid = layout::grid(w, app.ui.queue_open);
    let body: Element<'_, Message> = match &app.home {
        None => message("Loading\u{2026}"),
        Some(Err(_)) => message("Could not load the catalog. Try again in a moment."),
        Some(Ok(home)) => shelves(app, home, grid),
    };
    scrollable(body)
        .width(Length::Fill)
        .height(Length::Fill)
        .into()
}

fn message(text: &'static str) -> Element<'static, Message> {
    container(label(
        text,
        type_scale::BODY_MD,
        false,
        col(color::ON_SURFACE_VARIANT),
    ))
    .padding(gap::XL)
    .into()
}

fn shelves<'a>(app: &'a App, home: &'a HomeData, grid: layout::Grid) -> Element<'a, Message> {
    let n = grid.columns as usize;
    // Grid numbers are small integers, exact in f32.
    #[allow(clippy::cast_precision_loss)]
    let (card, gap_px, content_w) = (grid.card as f32, grid.gap as f32, grid.content as f32);

    let albums = home.albums.iter().take(n).map(|a| {
        card_view(
            app,
            &Artwork::Remote(a.thumbnails.clone()),
            &a.title,
            &crate::format::artists(&a.artists),
            card,
            false,
            Message::Ui(Event::Go(Route::Album(a.id.clone()))),
        )
    });
    let artists = home.artists.iter().take(n).map(|a| {
        card_view(
            app,
            &Artwork::Remote(a.thumbnails.clone()),
            &a.name,
            "Artist",
            card,
            true,
            Message::Ui(Event::Go(Route::Artist(a.id.clone()))),
        )
    });
    let playlists = home.playlists.iter().take(n).map(|p| {
        card_view(
            app,
            &Artwork::Remote(p.thumbnails.clone()),
            &p.title,
            p.author.as_deref().unwrap_or("Playlist"),
            card,
            false,
            Message::Ui(Event::Go(Route::Playlist(p.id.clone()))),
        )
    });
    let songs = home.songs.iter().take(n).enumerate().map(|(i, t)| {
        card_view(
            app,
            &t.artwork,
            &t.title,
            &crate::format::artists(&t.artists),
            card,
            false,
            Message::PlayShelfSong(i),
        )
    });

    let mut col_ = column![
        label(
            "Welcome back",
            type_scale::HEADLINE_MD,
            true,
            col(color::ON_SURFACE)
        ),
        Space::new().height(gap::XL),
    ]
    .width(Length::Fixed(content_w));
    let shelves: [(&str, Vec<Element<'a, Message>>); 4] = [
        ("Albums for you", albums.collect()),
        ("Artists", artists.collect()),
        ("Playlists", playlists.collect()),
        ("Songs", songs.collect()),
    ];
    let last = shelves.len() - 1;
    for (i, (title, cards)) in shelves.into_iter().enumerate() {
        col_ = col_.push(shelf_header(title));
        col_ = col_.push(Space::new().height(gap::MD));
        col_ = col_.push(row(cards).spacing(gap_px));
        if i != last {
            col_ = col_.push(Space::new().height(gap::XXL));
        }
    }
    // Bottom padding 112 so the floating bar never hides the last row.
    #[allow(clippy::cast_precision_loss)]
    let bottom = layout::CONTENT_BOTTOM_PADDING as f32;
    col_ = col_.push(Space::new().height(bottom));

    container(col_)
        .width(Length::Fill)
        .padding(Padding {
            top: gap::XL,
            ..Padding::ZERO
        })
        .center_x(Length::Fill)
        .into()
}

fn shelf_header(title: &str) -> Element<'_, Message> {
    row![
        label(title, type_scale::TITLE_MD, true, col(color::ON_SURFACE)),
        Space::new().width(Length::Fill),
        label(
            "Show all",
            type_scale::BODY_MD,
            false,
            col(color::ON_SURFACE_VARIANT)
        )
    ]
    .align_y(Alignment::Center)
    .into()
}

/// A card: square art, title 14/20 semibold, subtitle 12/18 (`ui-reference.md` section 3).
pub fn card_view<'a>(
    app: &'a App,
    artwork: &Artwork,
    title: &str,
    subtitle: &str,
    size: f32,
    round: bool,
    on_press: Message,
) -> Element<'a, Message> {
    let radius = if round { size / 2.0 } else { shape::MD };
    let art: Element<'a, Message> = match art_key(artwork).and_then(|k| app.art.get(&k)) {
        Some(ArtState::Ready { square, circle }) => {
            image(if round { circle } else { square }.clone())
                .width(Length::Fixed(size))
                .height(Length::Fixed(size))
                .content_fit(iced::ContentFit::Fill)
                .into()
        }
        other => art_placeholder(size, radius, matches!(other, Some(ArtState::Loading))),
    };
    let align = if round {
        Alignment::Center
    } else {
        Alignment::Start
    };
    let text = column![
        label(
            fit(title, size, type_scale::BODY_MD.size),
            type_scale::BODY_MD,
            true,
            col(color::ON_SURFACE)
        ),
        label(
            fit(subtitle, size, type_scale::BODY_SM.size),
            type_scale::BODY_SM,
            false,
            col(color::ON_SURFACE_VARIANT)
        ),
    ]
    .width(Length::Fixed(size))
    .align_x(align);
    button(
        column![art, Space::new().height(gap::SM), text]
            .width(Length::Fixed(size))
            .align_x(align),
    )
    .padding(0)
    .style(button_style(iced::Color::TRANSPARENT, shape::MD))
    .on_press(on_press)
    .into()
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
        border: iced::Border {
            radius: radius.into(),
            ..iced::Border::default()
        },
        ..container::Style::default()
    })
    .into()
}
