//! The rows of a setup step rise in one after the other (brand-intro spec, task 3): each row is
//! wrapped so it rises [`ROW_RISE_PX`] and fades in, [`crate::setup::motion::STAGGER_MS`] after
//! the row before it. Once a row has settled it is returned as it is.

use iced::Element;

use crate::gui::app::{App, Message};
use crate::gui::fx::Fx;
use crate::gui::style::col;
use crate::setup::motion::{ROW_RISE_PX, row};
use crate::theme::color;

/// Row number `index` of the step, risen to the point the clock has reached.
pub(super) fn rise<'a>(
    app: &App,
    index: usize,
    content: Element<'a, Message>,
) -> Element<'a, Message> {
    let p = row(app.setup_ms(), index);
    if p >= 0.999 {
        return content;
    }
    Fx::new(content)
        .passive()
        .entrance(p, ROW_RISE_PX, col(color::surface_container()))
        .into()
}

/// A column of rows, numbered from 0.
pub(super) fn rise_all<'a>(
    app: &App,
    rows: impl IntoIterator<Item = Element<'a, Message>>,
) -> Vec<Element<'a, Message>> {
    rows.into_iter()
        .enumerate()
        .map(|(i, r)| rise(app, i, r))
        .collect()
}
