//! Layout numbers of the floating player bar (`player-bar-redesign` spec): the capsule width,
//! the three zone widths and the step the right zone folds to. Pure functions of the window
//! width, so every rule is a unit test. Sizes are logical pixels.

use super::{PLAYER_BAR_HEIGHT, PLAYER_BAR_INSET};

/// Widest capsule. Wider windows keep the capsule centred at this width.
pub const MAX_WIDTH: u32 = 1200;
/// Padding inside the capsule, left and right.
pub const PADDING: u32 = 16;
/// Gap between two zones.
pub const ZONE_GAP: u32 = 16;
/// Capsule width from which the volume slider shows.
pub const SLIDER_FROM: u32 = 1140;
/// Capsule width from which the extra buttons show; below it they fold into `More`.
pub const EXTRAS_FROM: u32 = 1056;
/// Narrowest left zone: cover, a little text and the heart.
pub const LEFT_MIN: u32 = 220;
/// Widest left zone.
pub const LEFT_MAX: u32 = 300;
/// Narrowest centre zone: the transport row (240) with room for a seek track.
pub const CENTRE_MIN: u32 = 320;
/// Width of one icon button in the right zone.
pub const BUTTON: u32 = 36;
/// Gap between two right zone items.
pub const ITEM_GAP: u32 = 4;
/// Widest quality pill in the steps that have room (longer text ends in an ellipsis; the
/// popover has the full path).
pub const PILL_MAX: u32 = 208;
/// Widest quality pill when the right zone is folded.
pub const PILL_MAX_FOLDED: u32 = 168;
/// Width of the inline volume slider.
pub const VOLUME_SLIDER: u32 = 80;

/// How much of the right zone is shown.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Step {
    /// Pill, lyrics, queue, tools, speaker with slider, full-screen.
    Full,
    /// The same without the volume slider (the speaker still mutes).
    NoSlider,
    /// Pill, queue, speaker and one `More` button.
    Folded,
}

/// Widths of the zones inside the capsule.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Zones {
    /// Right zone step.
    pub step: Step,
    /// Left zone (cover, title, artist, heart).
    pub left: u32,
    /// Centre zone (transport and seek row).
    pub centre: u32,
    /// Right zone.
    pub right: u32,
}

/// Capsule width for a window width: inset 12 px on both sides, at most [`MAX_WIDTH`].
#[must_use]
pub fn width(window_w: u32) -> u32 {
    window_w.saturating_sub(2 * PLAYER_BAR_INSET).min(MAX_WIDTH)
}

/// Height the capsule and the gap below it take from the bottom of the window.
#[must_use]
pub const fn reserve() -> u32 {
    PLAYER_BAR_HEIGHT + PLAYER_BAR_INSET
}

/// The step the right zone shows at capsule width `bar_w`.
#[must_use]
pub const fn step(bar_w: u32) -> Step {
    if bar_w >= SLIDER_FROM {
        Step::Full
    } else if bar_w >= EXTRAS_FROM {
        Step::NoSlider
    } else {
        Step::Folded
    }
}

/// Widest quality pill at `step`.
#[must_use]
pub const fn pill_max(step: Step) -> u32 {
    match step {
        Step::Full | Step::NoSlider => PILL_MAX,
        Step::Folded => PILL_MAX_FOLDED,
    }
}

/// Width of the right zone at `step`, from its parts.
#[must_use]
pub const fn right_width(step: Step) -> u32 {
    // Items and the number of gaps between them.
    let (items, buttons, gaps) = match step {
        // Pill, lyrics, queue, tools, speaker, full-screen: 1 pill + 5 buttons, the slider.
        Step::Full => (PILL_MAX + VOLUME_SLIDER, 5, 6),
        Step::NoSlider => (PILL_MAX, 5, 5),
        // Pill, queue, speaker, more.
        Step::Folded => (PILL_MAX_FOLDED, 3, 3),
    };
    items + buttons * BUTTON + gaps * ITEM_GAP
}

/// Split capsule width `bar_w` into zones. The left zone is 30% of the inner width within
/// [`LEFT_MIN`] and [`LEFT_MAX`]; the centre takes the rest and never drops under
/// [`CENTRE_MIN`] unless the capsule is narrower than a 960 px window allows.
#[must_use]
pub fn zones(bar_w: u32) -> Zones {
    let step = step(bar_w);
    let right = right_width(step);
    let inner = bar_w.saturating_sub(2 * PADDING + 2 * ZONE_GAP);
    let wanted = (inner * 30 / 100).clamp(LEFT_MIN, LEFT_MAX);
    let room = inner.saturating_sub(right + CENTRE_MIN);
    let left = wanted.min(room.max(LEFT_MIN));
    let centre = inner.saturating_sub(left + right);
    Zones {
        step,
        left,
        centre,
        right,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn capsule_width_follows_the_window_and_stops_at_the_maximum() {
        assert_eq!(width(960), 936);
        assert_eq!(width(1100), 1076);
        assert_eq!(width(1280), 1200);
        assert_eq!(width(1920), 1200);
        assert_eq!(width(2560), 1200);
        assert_eq!(width(10), 0);
    }

    #[test]
    fn right_zone_folds_in_steps_as_the_window_narrows() {
        // 1280 and wider: everything. 1100: no slider. 960: folded.
        assert_eq!(step(width(1920)), Step::Full);
        assert_eq!(step(width(1280)), Step::Full);
        assert_eq!(step(width(1164)), Step::Full);
        assert_eq!(step(width(1163)), Step::NoSlider);
        assert_eq!(step(width(1100)), Step::NoSlider);
        assert_eq!(step(width(1080)), Step::NoSlider);
        assert_eq!(step(width(1079)), Step::Folded);
        assert_eq!(step(width(960)), Step::Folded);
    }

    #[test]
    fn right_zone_gets_narrower_with_each_step() {
        let [full, no_slider, folded] = [Step::Full, Step::NoSlider, Step::Folded].map(right_width);
        assert_eq!((full, no_slider, folded), (492, 408, 288));
    }

    #[test]
    fn zones_never_overlap_and_the_centre_keeps_its_room() {
        for window_w in 960..=3000 {
            let bar_w = width(window_w);
            let z = zones(bar_w);
            let used = z.left + z.centre + z.right + 2 * ZONE_GAP + 2 * PADDING;
            assert!(used <= bar_w, "window {window_w}: {used} > {bar_w}");
            assert!(
                z.centre >= CENTRE_MIN,
                "window {window_w}: centre {}",
                z.centre
            );
            assert!((LEFT_MIN..=LEFT_MAX).contains(&z.left), "window {window_w}");
        }
    }

    #[test]
    fn zones_at_the_target_windows() {
        let at = |w: u32| zones(width(w));
        assert_eq!((at(960).left, at(960).centre), (261, 323));
        assert_eq!((at(1100).left, at(1100).centre), (284, 320));
        assert_eq!((at(1280).left, at(1280).right), (300, 492));
    }

    #[test]
    fn reserve_keeps_the_panes_above_the_capsule() {
        assert_eq!(reserve(), 100);
    }
}
