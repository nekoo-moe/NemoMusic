//! The `Listening area` control (source-workspaces spec): the one place a person picks between
//! YouTube Music and SoundCloud. The Your accounts sheet and the SoundCloud section of Settings
//! both draw this same control. It is never drawn on Home, the top bar, the sidebar or the
//! player bar.
//!
//! While SoundCloud cannot be the area (it is off, or nobody is signed in) there is nothing to
//! choose, so one plain sentence says what to do instead of a control with a dead choice.

use iced::Element;
use iced::widget::column;

use super::kit::{paragraph, sliding_chips_with};
use crate::copy::soundcloud as text;
use crate::gui::app::{App, Message, ScMsg};
use crate::gui::style::{col, gap, label};
use crate::theme::{color, type_scale};
use crate::workspace::Source;

/// Slot of the control's sliding pill in `App::chip_pills`.
pub(crate) const PILL_SLOT: usize = 4;

/// The message of choice `i` of the control.
fn pick(i: usize) -> Message {
    Message::Sc(ScMsg::Source(
        Source::ALL.get(i).copied().unwrap_or_default(),
    ))
}

/// What Left and Right send from the focused choice `i`: the neighbour, with its direction so the
/// focus can follow. `None` at either end.
fn step(i: usize, right: bool) -> Option<Message> {
    let to = if right {
        i.checked_add(1)?
    } else {
        i.checked_sub(1)?
    };
    Some(Message::AreaKey {
        to: *Source::ALL.get(to)?,
        forward: right,
    })
}

/// The sentence that stands in for the control, when SoundCloud is not offered.
fn only_youtube(app: &App) -> &'static str {
    if app.sc_open() {
        text::AREA_NEEDS_SIGN_IN
    } else {
        text::AREA_NEEDS_ON
    }
}

/// The title, then the two choices (or the sentence that says why there is one), then one line
/// of help. `width` is the width the sentences wrap at. `None` in a build without SoundCloud:
/// there is only one area then.
pub fn control(app: &App, width: f32) -> Option<Element<'_, Message>> {
    app.sc_info()?;
    Some(choices(app, width))
}

fn choices(app: &App, width: f32) -> Element<'_, Message> {
    let dim = col(color::on_surface_variant());
    let title = label(
        text::AREA_TITLE,
        type_scale::TITLE_SM,
        true,
        col(color::on_surface()),
    );
    if !app.source_offered(Source::SoundCloud) {
        return column![
            title,
            paragraph(only_youtube(app), type_scale::BODY_SM, dim, width)
        ]
        .spacing(gap::SM)
        .into();
    }
    let labels = Source::ALL.map(Source::label);
    column![
        title,
        sliding_chips_with(
            app,
            &labels,
            app.source.index(),
            PILL_SLOT,
            pick,
            Some(step)
        ),
        paragraph(text::AREA_HELP, type_scale::BODY_SM, dim, width),
    ]
    .spacing(gap::SM)
    .into()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn left_and_right_pick_the_neighbour_and_stop_at_the_ends() {
        assert!(matches!(
            step(0, true),
            Some(Message::AreaKey {
                to: Source::SoundCloud,
                forward: true
            })
        ));
        assert!(matches!(
            step(1, false),
            Some(Message::AreaKey {
                to: Source::YouTube,
                forward: false
            })
        ));
        assert!(step(0, false).is_none() && step(1, true).is_none());
    }

    #[test]
    fn a_choice_sends_its_area_and_a_stray_index_falls_back_to_youtube_music() {
        assert!(matches!(
            pick(0),
            Message::Sc(ScMsg::Source(Source::YouTube))
        ));
        assert!(matches!(
            pick(1),
            Message::Sc(ScMsg::Source(Source::SoundCloud))
        ));
        assert!(matches!(
            pick(9),
            Message::Sc(ScMsg::Source(Source::YouTube))
        ));
    }
}
