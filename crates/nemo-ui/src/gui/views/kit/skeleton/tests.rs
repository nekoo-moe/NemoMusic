//! Each skeleton shape has the outer size of the real shape it stands for (skeleton-loading
//! spec, criterion 12). The shapes of Search, Home and the sidebar are measured next to the
//! views they mirror. These build a headless renderer, so they are ignored by default: run
//! `ICED_TEST_BACKEND=tiny-skia cargo test -p nemo-ui -- --ignored skeleton`.

use iced::widget::column;
use iced::{Element, Length, Size};

use super::measure::size_of;
use crate::gui::app::{App, ArtShape, ListSource, Message};
use crate::gui::headless::{Scenario, build_app};
use crate::gui::views::kit::{
    Card, LIST_ITEM_H, ROW_H, RowCtx, Skeleton, card_view, chip, chip_width, shelf_header,
    track_list,
};
use crate::mock::mock_backend;
use crate::theme::type_scale;

fn app() -> App {
    build_app(&mock_backend(), &Scenario::default())
}

/// A look that is not tied to an app: full opacity, still.
fn look() -> Skeleton {
    Skeleton::still()
}

fn assert_same(what: &str, skeleton: Size, real: Size) {
    assert!(
        (skeleton.width - real.width).abs() < 0.51 && (skeleton.height - real.height).abs() < 0.51,
        "{what}: skeleton {skeleton:?} against real {real:?}"
    );
}

fn card(shape: ArtShape) -> Card {
    Card {
        artwork: crate::mock::track(0).artwork,
        title: "A title".to_owned(),
        subtitle: "A subtitle".to_owned(),
        shape,
        on_press: Message::Noop,
        play: None,
    }
}

#[test]
#[ignore = "builds a headless renderer: run with ICED_TEST_BACKEND=tiny-skia"]
fn a_skeleton_card_is_as_big_as_a_card() {
    let app = app();
    for size in [160_u32, 200, 240] {
        let px = f32::from(u16::try_from(size).expect("small"));
        let real = size_of(card_view(&app, card(ArtShape::Tile), size, 0));
        assert_same("card", size_of(look().card(px)), real);
        let real = size_of(card_view(&app, card(ArtShape::Circle), size, 0));
        assert_same("artist card", size_of(look().artist_card(px)), real);
    }
}

#[test]
#[ignore = "builds a headless renderer: run with ICED_TEST_BACKEND=tiny-skia"]
fn a_skeleton_row_is_as_big_as_a_track_row() {
    let app = app();
    let tracks = vec![crate::mock::track(0)];
    for width in [480.0_f32, 677.0, 1000.0] {
        let ctx = RowCtx {
            width,
            source: ListSource::Search,
            numbered: false,
            show_album: true,
        };
        let real = size_of(track_list(&app, &tracks, 1, &ctx));
        assert_same("row", size_of(look().row(width, 0)), real);
        assert!((real.height - ROW_H).abs() < 0.51, "{real:?}");
    }
}

#[test]
#[ignore = "builds a headless renderer: run with ICED_TEST_BACKEND=tiny-skia"]
fn a_skeleton_header_is_as_big_as_a_shelf_header() {
    let real = size_of(shelf_header("Albums"));
    let sk = size_of(look().section_header(120.0));
    assert!((sk.height - real.height).abs() < 0.51, "{sk:?} {real:?}");
}

#[test]
#[ignore = "builds a headless renderer: run with ICED_TEST_BACKEND=tiny-skia"]
fn a_skeleton_chip_is_as_big_as_a_chip() {
    let app = app();
    for text in ["All", "Songs", "Community playlists"] {
        let real = size_of(chip(text, false, app.accent(), Message::Noop));
        let sk = size_of(look().chip_row(&[text]));
        assert_same(text, sk, real);
        assert!((sk.width - chip_width(text)).abs() < 0.51);
    }
}

#[test]
#[ignore = "builds a headless renderer: run with ICED_TEST_BACKEND=tiny-skia"]
fn a_skeleton_list_item_is_one_library_row_high() {
    let sk = size_of(look().list_item(216.0));
    assert_same("list item", sk, Size::new(216.0, LIST_ITEM_H));
}

#[test]
#[ignore = "builds a headless renderer: run with ICED_TEST_BACKEND=tiny-skia"]
fn a_skeleton_line_is_as_high_as_its_type_line() {
    let rows: Element<'_, Message> = column![
        look().line(100.0, type_scale::TITLE_LG),
        look().line(100.0, type_scale::BODY_SM)
    ]
    .width(Length::Shrink)
    .into();
    let size = size_of(rows);
    let want = f32::from(type_scale::TITLE_LG.line) + f32::from(type_scale::BODY_SM.line);
    assert!((size.height - want).abs() < 0.51, "{size:?}");
}
