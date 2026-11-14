//! Shell: three rounded panes on `surface-container` (navigation and library, content, Now
//! Playing) with 8 px gaps, the floating capsule on top, and the right pane's overlay mode below
//! 1280 px. `docs/ui-redesign.md` section 2 has the numbers.

use iced::widget::{column, container, row, stack};
use iced::{Alignment, Background, Border, Color, Element, Length, Padding};

use super::super::app::{App, Message};
use super::super::fx::entrance;
use super::super::style::{col, gap};
use super::kit::pane_bg;
use super::{bar, detail, explore, home, library, pane, search, settings};
use crate::layout;
use crate::route::Route;
use crate::theme::{color, shape};

mod nav;
mod sidebar;
mod topbar;

use nav::left_pane;
use topbar::top_bar;

/// A rounded pane background.
pub fn pane_surface<'a>(
    content: impl Into<Element<'a, Message>>,
    width: Length,
    bg: Background,
) -> iced::widget::Container<'a, Message> {
    container(content)
        .width(width)
        .height(Length::Fill)
        .clip(true)
        .style(move |_| container::Style {
            background: Some(bg),
            border: Border {
                radius: shape::scaled(layout::PANE_RADIUS).into(),
                ..Border::default()
            },
            ..container::Style::default()
        })
}

/// The window below the notices: the panes and the capsule.
pub fn view(app: &App, w: u32, h: u32) -> Element<'_, Message> {
    // Pane numbers are small, exact in f32.
    #[allow(clippy::cast_precision_loss)]
    let (left_w, right_full) = (layout::left_width(w) as f32, layout::right_width(w) as f32);
    let pushes = layout::right_pushes(w);
    let open = app.anim.pane.value.max(0.0);
    let center_w = layout::center_width(w, open);

    let mut body = row![left_pane(app, left_w, h), center_pane(app, center_w, w, h)]
        .spacing(gap::SM)
        .width(Length::Fill)
        .height(Length::Fill);
    // The panes end above the capsule while it shows, so it never covers a row.
    let capsule_p = app.anim.capsule.value.clamp(0.0, 1.0);
    // A small constant, exact in f32.
    #[allow(clippy::cast_precision_loss)]
    let reserve = layout::PANE_BOTTOM_RESERVE as f32 * capsule_p;
    if pushes && open > 0.002 {
        body = body.push(pane::view(
            app,
            right_full,
            Length::Fixed((right_full * open.min(1.0)).round()),
        ));
    }
    let mut layers: Vec<Element<'_, Message>> = vec![
        container(body)
            .padding(Padding {
                bottom: gap::SM + reserve,
                ..Padding::new(gap::SM)
            })
            .width(Length::Fill)
            .height(Length::Fill)
            .into(),
    ];
    if !pushes && app.pane_open {
        layers.push(
            container(entrance(
                pane::view(app, right_full, Length::Fixed(right_full)),
                app.anim.pane.value.clamp(0.0, 1.0),
                pane_bg(),
            ))
            .width(Length::Fill)
            .height(Length::Fill)
            .align_x(Alignment::End)
            .padding(Padding {
                bottom: gap::SM + reserve,
                ..Padding::new(gap::SM)
            })
            .into(),
        );
    }
    if app.ui.snapshot.current.is_some() || app.anim.capsule.value > 0.01 {
        // Capsule widths are small, exact in f32.
        #[allow(clippy::cast_precision_loss)]
        let cap_w = layout::capsule_width(w) as f32;
        let p = app.anim.capsule.value.clamp(0.0, 1.0);
        // A small constant, exact in f32.
        #[allow(clippy::cast_precision_loss)]
        let capsule_margin = layout::CAPSULE_MARGIN as f32;
        layers.push(
            container(
                super::super::fx::Fx::new(bar::view(app, cap_w))
                    .passive()
                    .entrance(p, 28.0, Color::TRANSPARENT),
            )
            .width(Length::Fill)
            .height(Length::Fill)
            .align_y(Alignment::End)
            .center_x(Length::Fill)
            .padding(Padding {
                bottom: capsule_margin,
                ..Padding::ZERO
            })
            .into(),
        );
    }
    stack(layers).into()
}

// ---------------------------------------------------------------------------------------------
// Centre pane
// ---------------------------------------------------------------------------------------------

fn center_pane(app: &App, center_w: u32, _w: u32, h: u32) -> Element<'_, Message> {
    let page = content(app, center_w, h);
    // Home staggers its own items; every other page fades and slides in as one.
    let page = if matches!(app.ui.route(), Route::Home) {
        page
    } else {
        entrance(page, app.anim.page(), pane_bg())
    };
    pane_surface(
        column![top_bar(app, center_w), page],
        Length::Fill,
        col(color::surface_container().with_alpha(crate::scheme::PANE_ALPHA)).into(),
    )
    .into()
}

/// The content router: one arm per screen. `cw` is the width of the centre pane.
fn content(app: &App, cw: u32, _h: u32) -> Element<'_, Message> {
    match app.ui.route() {
        Route::Search(_) => search::view(app, cw),
        Route::Explore => explore::view(app, cw),
        Route::Library => library::view(app, cw),
        Route::Settings => settings::view(app, cw),
        Route::Album(_) | Route::Artist(_) | Route::Playlist(_) => detail::view(app, cw),
        // The queue is a tab of the right pane and Now Playing is its own screen; both are
        // handled before this.
        Route::Home | Route::Queue | Route::NowPlaying => home::view(app, cw),
    }
}
