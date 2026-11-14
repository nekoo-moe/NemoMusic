//! The cover slot of the player capsule: size-matched bake and the fade from the old cover.

use iced::widget::{image, stack};
use iced::{Element, Length};

use crate::gui::app::{App, CoverView, Message};
use crate::gui::views::kit::art_placeholder;

/// One cover handle for a slot: the 128 px bar bake up to 96 px, the 480 px bake above.
fn cover_handle(c: &CoverView, size: f32) -> image::Handle {
    if size <= 96.0 {
        c.art_bar.clone()
    } else {
        c.art.clone()
    }
}

/// Current cover at `size` px with radius `radius`, or the placeholder. When the track changed
/// the old cover stays under the new one while it fades in (400 ms).
pub fn cover_art<'a>(app: &App, size: f32, radius: f32) -> Element<'a, Message> {
    let Some(c) = &app.cover else {
        return art_placeholder(size, radius, false);
    };
    let fade = app.anim.cover();
    let draw = |handle: image::Handle, opacity: f32| {
        image(handle)
            .width(Length::Fixed(size))
            .height(Length::Fixed(size))
            .content_fit(iced::ContentFit::Fill)
            .opacity(opacity)
    };
    match &app.prev_cover {
        Some(prev) if fade < 0.999 => stack![
            draw(cover_handle(prev, size), 1.0),
            draw(cover_handle(c, size), fade)
        ]
        .width(Length::Fixed(size))
        .height(Length::Fixed(size))
        .into(),
        _ => draw(cover_handle(c, size), 1.0).into(),
    }
}
