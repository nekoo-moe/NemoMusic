//! Test helper: the outer size an element takes when it is laid out, by a headless renderer.
//! Run the tests that use it with `ICED_TEST_BACKEND=tiny-skia cargo test -p nemo-ui -- --ignored`.

use iced::widget::{Id, container};
use iced::{Element, Pixels, Size};
use iced_test::Simulator;
use iced_test::selector::Candidate;

use crate::gui::app::Message;
use crate::gui::assets;

/// The name of the probe container.
const PROBE: &str = "skeleton-probe";

/// The size of `view` when laid out in a 1400 x 1000 window, alone in a container that wraps it.
///
/// # Panics
/// When the headless renderer cannot be created or the probe is not found (tests only).
#[must_use]
pub fn size_of(view: Element<'_, Message>) -> Size {
    let settings = iced::Settings {
        fonts: assets::ALL_FONTS.iter().map(|f| (*f).into()).collect(),
        default_font: assets::font(false),
        default_text_size: Pixels(14.0),
        ..iced::Settings::default()
    };
    let probe = container(view).id(Id::new(PROBE));
    let mut sim = Simulator::with_size(settings, Size::new(1400.0, 1000.0), probe);
    let target = sim.find(|c: Candidate<'_>| match c {
        Candidate::Container {
            id: Some(id),
            bounds,
            ..
        } if *id == Id::new(PROBE) => Some(bounds),
        _ => None,
    });
    target.expect("the probe is laid out").size()
}
