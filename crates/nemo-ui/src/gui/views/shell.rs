//! Shell: opaque sidebar, top bar, content router, queue panel and the floating player bar.

use iced::widget::{Space, button, column, container, row, scrollable, stack};
use iced::{Alignment, Element, Length, Padding};

use super::super::app::{App, Message};
use super::super::assets::Icon;
use super::super::style::{button_style, col, fill, gap, icon, icon_button, label};
use super::{bar, home};
use crate::layout;
use crate::route::{NavEntry, Route};
use crate::state::Event;
use crate::theme::{color, shape, type_scale};
use nemo_core::player::PlayerCommand;

/// Sidebar entry height.
const ENTRY_H: f32 = 40.0;

/// Sidebar + top bar + content + floating bar (+ queue panel).
pub fn view(app: &App, w: u32, h: u32) -> Element<'_, Message> {
    let sidebar_w = layout::sidebar_width(w);
    let pushes = layout::queue_pushes_content(w, app.ui.queue_open);
    let queue_w = if pushes { layout::QUEUE_WIDTH } else { 0 };

    let main = column![top_bar(app), content(app, w, h)]
        .width(Length::Fill)
        .height(Length::Fill);
    let mut body = row![sidebar(app, sidebar_w), main]
        .width(Length::Fill)
        .height(Length::Fill);
    if pushes {
        body = body.push(queue_panel(app));
    }

    // The bar floats over the main column, inset 12 px from the window edges.
    let inset = layout::PLAYER_BAR_INSET;
    let bar_w = w.saturating_sub(sidebar_w + queue_w + 2 * inset);
    let mut layers: Vec<Element<'_, Message>> = vec![body.into()];
    if app.ui.snapshot.current.is_some() {
        // Pixel sizes of a window are small, exact in f32.
        #[allow(clippy::cast_precision_loss)]
        let (left, right) = ((sidebar_w + inset) as f32, (queue_w + inset) as f32);
        layers.push(
            container(bar::view(app, bar_w))
                .width(Length::Fill)
                .height(Length::Fill)
                .align_y(Alignment::End)
                .padding(Padding {
                    top: 0.0,
                    right,
                    bottom: gap::MD,
                    left,
                })
                .into(),
        );
    }
    if app.ui.queue_open && !pushes {
        // Below 1280 the panel overlays the content (opaque, never glass).
        layers.push(
            container(queue_panel(app))
                .width(Length::Fill)
                .height(Length::Fill)
                .align_x(Alignment::End)
                .into(),
        );
    }
    stack(layers).into()
}

fn sidebar(app: &App, width: u32) -> Element<'_, Message> {
    let wide = width >= layout::SIDEBAR_WIDE;
    let route = app.ui.route();
    let selected = route.entry();

    let mut logo = row![icon(Icon::Note, 24.0, col(color::ON_SURFACE))].align_y(Alignment::Center);
    if wide {
        logo = logo.push(Space::new().width(gap::SM)).push(label(
            "NemoMusic",
            type_scale::TITLE_SM,
            true,
            col(color::ON_SURFACE),
        ));
    }
    // Width and height are small exact values.
    #[allow(clippy::cast_precision_loss)]
    let sidebar_w = width as f32;
    let mut list = column![
        container(logo)
            .height(Length::Fixed(56.0))
            .width(Length::Fill)
            .padding(Padding::from([0.0, gap::LG]))
            .align_y(Alignment::Center)
    ]
    .spacing(gap::XS)
    .width(Length::Fill);

    for entry in NavEntry::ALL {
        let on = selected == Some(entry);
        let ink = col(if on {
            color::ON_SURFACE
        } else {
            color::ON_SURFACE_VARIANT
        });
        let glyph = match entry {
            NavEntry::Home => Icon::Home,
            NavEntry::Search => Icon::Search,
            NavEntry::Library => Icon::Library,
            NavEntry::Queue => Icon::Queue,
            NavEntry::Settings => Icon::Settings,
        };
        let content: Element<'_, Message> = if wide {
            row![
                icon(glyph, 20.0, ink),
                Space::new().width(gap::MD),
                label(entry.label(), type_scale::BODY_MD, on, ink)
            ]
            .align_y(Alignment::Center)
            .into()
        } else {
            container(icon(glyph, 20.0, ink))
                .width(Length::Fill)
                .center_x(Length::Fill)
                .into()
        };
        let idle = if on {
            col(color::SURFACE_CONTAINER_HIGH)
        } else {
            iced::Color::TRANSPARENT
        };
        let target = Message::Ui(Event::Go(entry.route()));
        let b = button(
            container(content)
                .height(Length::Fixed(ENTRY_H))
                .width(Length::Fill)
                .padding(Padding::from([0.0, gap::MD]))
                .align_y(Alignment::Center),
        )
        .padding(0)
        .width(Length::Fill)
        .style(button_style(idle, shape::MD))
        .on_press(target);
        list = list.push(container(b).padding(Padding::from([0.0, gap::SM])));
    }

    if wide && let Some(Ok(home)) = &app.home {
        {
            list = list.push(Space::new().height(gap::LG));
            list = list.push(
                container(
                    container(label(
                        "Playlists",
                        type_scale::LABEL_SM,
                        false,
                        col(color::ON_SURFACE_VARIANT),
                    ))
                    .padding(Padding::from([gap::XS, gap::MD])),
                )
                .padding(Padding::from([0.0, gap::SM])),
            );
            for p in home.playlists.iter().take(4) {
                let on = *route == Route::Playlist(p.id.clone());
                let ink = col(if on {
                    color::ON_SURFACE
                } else {
                    color::ON_SURFACE_VARIANT
                });
                let idle = if on {
                    col(color::SURFACE_CONTAINER_HIGH)
                } else {
                    iced::Color::TRANSPARENT
                };
                let b = button(
                    container(label(
                        super::super::style::fit(
                            &p.title,
                            sidebar_w - 56.0,
                            type_scale::BODY_MD.size,
                        ),
                        type_scale::BODY_MD,
                        on,
                        ink,
                    ))
                    .height(Length::Fixed(ENTRY_H))
                    .width(Length::Fill)
                    .padding(Padding::from([0.0, gap::MD]))
                    .align_y(Alignment::Center),
                )
                .padding(0)
                .width(Length::Fill)
                .style(button_style(idle, shape::MD))
                .on_press(Message::Ui(Event::Go(Route::Playlist(p.id.clone()))));
                list = list.push(container(b).padding(Padding::from([0.0, gap::SM])));
            }
        }
    }

    fill(list, color::SURFACE_CONTAINER_LOW)
        .width(Length::Fixed(sidebar_w))
        .height(Length::Fill)
        .into()
}

fn top_bar(app: &App) -> Element<'_, Message> {
    let ink = col(color::ON_SURFACE);
    let dim = col(color::ON_SURFACE_VARIANT);
    let back = icon_button(
        Icon::ChevronLeft,
        20.0,
        40.0,
        if app.ui.nav.can_back() {
            ink
        } else {
            col(color::DISABLED)
        },
        iced::Color::TRANSPARENT,
        app.ui.nav.can_back().then_some(Message::Ui(Event::Back)),
    );
    let forward = icon_button(
        Icon::ChevronRight,
        20.0,
        40.0,
        if app.ui.nav.can_forward() {
            ink
        } else {
            col(color::DISABLED)
        },
        iced::Color::TRANSPARENT,
        app.ui
            .nav
            .can_forward()
            .then_some(Message::Ui(Event::Forward)),
    );
    let search = button(
        container(
            row![
                icon(Icon::Search, 20.0, dim),
                Space::new().width(gap::SM),
                label(
                    "Search songs, albums, artists",
                    type_scale::BODY_MD,
                    false,
                    dim
                )
            ]
            .align_y(Alignment::Center),
        )
        .height(Length::Fill)
        .width(Length::Fill)
        .center_y(Length::Fill)
        .padding(Padding::from([0.0, gap::LG])),
    )
    .width(Length::Fixed(480.0))
    .height(Length::Fixed(40.0))
    .padding(0)
    .clip(true)
    .style(button_style(
        col(color::SURFACE_CONTAINER_HIGH),
        shape::FULL,
    ))
    .on_press(Message::Ui(Event::Go(Route::Search(String::new()))));
    let avatar = container(label("N", type_scale::BODY_MD, true, ink))
        .width(Length::Fixed(32.0))
        .height(Length::Fixed(32.0))
        .center_x(Length::Fixed(32.0))
        .center_y(Length::Fixed(32.0))
        .style(|_| container::Style {
            background: Some(col(color::SURFACE_CONTAINER_HIGH).into()),
            border: iced::Border {
                radius: shape::FULL.into(),
                ..iced::Border::default()
            },
            ..container::Style::default()
        });
    row![
        back,
        forward,
        Space::new().width(gap::LG),
        search,
        Space::new().width(Length::Fill),
        avatar
    ]
    .align_y(Alignment::Center)
    .padding(Padding::from([0.0, gap::XL]))
    .height(Length::Fixed(56.0))
    .into()
}

/// The content router. Search, Album, Artist, Playlist, Queue and Settings get their own
/// modules later; each only needs an arm here.
fn content(app: &App, w: u32, _h: u32) -> Element<'_, Message> {
    match app.ui.route() {
        Route::Home => home::view(app, w),
        other => placeholder(other),
    }
}

fn placeholder(route: &Route) -> Element<'static, Message> {
    let dim = col(color::ON_SURFACE_VARIANT);
    container(
        column![
            icon(Icon::Note, 32.0, dim),
            Space::new().height(gap::MD),
            label(
                route.title(),
                type_scale::TITLE_MD,
                true,
                col(color::ON_SURFACE)
            ),
            Space::new().height(gap::XS),
            label(
                "This screen is not built yet.",
                type_scale::BODY_MD,
                false,
                dim
            ),
        ]
        .align_x(Alignment::Center),
    )
    .width(Length::Fill)
    .height(Length::Fill)
    .center_x(Length::Fill)
    .center_y(Length::Fill)
    .into()
}

fn queue_panel(app: &App) -> Element<'_, Message> {
    let snap = &app.ui.snapshot;
    let header = row![
        label("Queue", type_scale::TITLE_MD, true, col(color::ON_SURFACE)),
        Space::new().width(Length::Fill),
        icon_button(
            Icon::Close,
            20.0,
            40.0,
            col(color::ON_SURFACE),
            iced::Color::TRANSPARENT,
            Some(Message::Ui(Event::ToggleQueue)),
        )
    ]
    .align_y(Alignment::Center)
    .height(Length::Fixed(56.0))
    .padding(Padding::from([0.0, gap::LG]));

    let mut rows = column![].spacing(0);
    for (i, t) in snap.queue.iter().enumerate() {
        let current = snap.queue_index == Some(i);
        let inner = column![
            label(
                super::super::style::fit(&t.title, 260.0, type_scale::BODY_MD.size),
                type_scale::BODY_MD,
                current,
                col(color::ON_SURFACE),
            ),
            label(
                super::super::style::fit(
                    &crate::format::artists(&t.artists),
                    260.0,
                    type_scale::BODY_SM.size
                ),
                type_scale::BODY_SM,
                false,
                col(color::ON_SURFACE_VARIANT),
            ),
        ];
        rows = rows.push(
            button(
                container(inner)
                    .height(Length::Fixed(48.0))
                    .width(Length::Fill)
                    .padding(Padding::from([0.0, gap::LG]))
                    .align_y(Alignment::Center),
            )
            .padding(0)
            .width(Length::Fill)
            .style(button_style(iced::Color::TRANSPARENT, shape::MD))
            .on_press(Message::Ui(Event::Command(PlayerCommand::SetQueue {
                tracks: snap.queue.clone(),
                start: i,
            }))),
        );
    }
    // The queue panel is opaque `surface-container`, never glass.
    fill(
        column![header, scrollable(rows).height(Length::Fill)],
        color::SURFACE_CONTAINER,
    )
    .width(Length::Fixed(f32::from(
        u16::try_from(layout::QUEUE_WIDTH).unwrap_or(320),
    )))
    .height(Length::Fill)
    .into()
}
