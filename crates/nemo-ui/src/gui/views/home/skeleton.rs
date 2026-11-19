//! Home before its first load: the chip row, the greeting, the hero banner and two shelves as
//! skeleton blocks. Only drawn when nothing is cached (skeleton-loading spec).

use iced::Element;
use iced::Length;
use iced::widget::column;

use crate::gui::app::{App, HomeFilter, Message};
use crate::gui::style::{col, gap, label};
use crate::gui::views::kit::{Skeleton, page, vspace};
use crate::layout;
use crate::theme::{color, type_scale};

/// Shelves of the skeleton.
const SHELVES: usize = 2;

/// The skeleton of the page.
pub(super) fn view<'a>(app: &App, sk: Skeleton, grid: layout::Grid) -> Element<'a, Message> {
    // Content widths are small exact numbers.
    #[allow(clippy::cast_precision_loss)]
    let content_w = grid.content as f32;
    let labels: Vec<&str> = HomeFilter::ALL.iter().map(|f| f.label()).collect();
    let mut c = column![
        sk.chip_row(&labels),
        vspace(gap::MD),
        label(
            app.day.greeting(),
            type_scale::HEADLINE_MD,
            true,
            col(color::on_surface())
        ),
        vspace(gap::MD),
        sk.hero(content_w),
        vspace(gap::XL),
    ]
    .width(Length::Fill);
    for _ in 0..SHELVES {
        c = c
            .push(sk.section_header(content_w * 0.2))
            .push(sk.cards(grid, 1, false))
            .push(vspace(gap::XXL));
    }
    page(grid.content, c)
}

#[cfg(test)]
mod tests {
    use iced::Element;
    use iced::widget::column;

    use super::super::hero::greeting;
    use crate::gui::app::Message;
    use crate::gui::headless::{Scenario, build_app};
    use crate::gui::style::gap;
    use crate::gui::views::kit::{Skeleton, size_of, vspace};
    use crate::mock::mock_backend;
    use crate::theme::type_scale;

    #[test]
    #[ignore = "builds a headless renderer: run with ICED_TEST_BACKEND=tiny-skia"]
    fn the_skeleton_hero_block_is_as_big_as_the_greeting_and_banner() {
        let app = build_app(&mock_backend(), &Scenario::default());
        for width in [677.0_f32, 1000.0, 1200.0] {
            let real = size_of(greeting(&app, width));
            let sk: Element<'_, Message> = column![
                Skeleton::still().line(120.0, type_scale::HEADLINE_MD),
                vspace(gap::MD),
                Skeleton::still().hero(width),
                vspace(gap::XL),
            ]
            .into();
            let sk = size_of(sk);
            assert!(
                (sk.height - real.height).abs() < 0.51 && (sk.width - width).abs() < 0.51,
                "{width}: skeleton {sk:?} against real {real:?}"
            );
        }
    }
}
