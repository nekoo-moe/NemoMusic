//! Keyboard map (`design.md` section 13). Pure: the GUI converts iced key events into [`Key`]
//! and [`Mods`], and the reducer acts on the resulting [`KeyAction`].

/// A key the app reacts to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Key {
    /// Space bar.
    Space,
    /// Arrow left.
    Left,
    /// Arrow right.
    Right,
    /// Arrow up.
    Up,
    /// Arrow down.
    Down,
    /// Escape.
    Escape,
    /// F3.
    F3,
    /// A character key (lower case).
    Char(char),
}

/// Modifier keys held.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Mods {
    /// Control (Command on macOS).
    pub ctrl: bool,
    /// Shift.
    pub shift: bool,
}

/// What a key press asks for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KeyAction {
    /// Play or pause.
    TogglePlay,
    /// Seek relative, in seconds (negative goes back).
    SeekBy(i64),
    /// Change the volume by this many percent points.
    VolumeBy(i8),
    /// Next track.
    Next,
    /// Previous track.
    Previous,
    /// Open search.
    OpenSearch,
    /// Open settings.
    OpenSettings,
    /// Close the top-most thing: Now Playing, the queue panel, or step back.
    Close,
    /// Show or hide the frame-time readout.
    ToggleFrameReadout,
    /// Change the lyrics offset by this many milliseconds.
    LyricsOffsetBy(i64),
}

/// Seek step in seconds (`Shift` uses [`SEEK_BIG_SECS`]).
pub const SEEK_SECS: i64 = 5;
/// Big seek step in seconds.
pub const SEEK_BIG_SECS: i64 = 30;
/// Volume step in percent points.
pub const VOLUME_STEP: i8 = 5;
/// Lyrics offset step in milliseconds (ADR 0002).
pub const LYRICS_OFFSET_STEP_MS: i64 = 50;

/// Map a key press to an action. In a text field only keys that cannot be typed (Esc, F3) and
/// Ctrl combinations apply: single-letter keys, Space and the arrows stay with the field.
#[must_use]
pub fn map(key: Key, mods: Mods, in_text_field: bool) -> Option<KeyAction> {
    match key {
        Key::Escape => Some(KeyAction::Close),
        Key::F3 => Some(KeyAction::ToggleFrameReadout),
        Key::Char('k') if mods.ctrl => Some(KeyAction::OpenSearch),
        Key::Char(',') if mods.ctrl => Some(KeyAction::OpenSettings),
        _ if in_text_field || mods.ctrl => None,
        Key::Space => Some(KeyAction::TogglePlay),
        Key::Left | Key::Right => {
            let step = if mods.shift { SEEK_BIG_SECS } else { SEEK_SECS };
            Some(KeyAction::SeekBy(if key == Key::Left {
                -step
            } else {
                step
            }))
        }
        Key::Up => Some(KeyAction::VolumeBy(VOLUME_STEP)),
        Key::Down => Some(KeyAction::VolumeBy(-VOLUME_STEP)),
        Key::Char('n') => Some(KeyAction::Next),
        Key::Char('p') => Some(KeyAction::Previous),
        Key::Char('/') => Some(KeyAction::OpenSearch),
        Key::Char('[') => Some(KeyAction::LyricsOffsetBy(-LYRICS_OFFSET_STEP_MS)),
        Key::Char(']') => Some(KeyAction::LyricsOffsetBy(LYRICS_OFFSET_STEP_MS)),
        Key::Char(_) => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const NONE: Mods = Mods {
        ctrl: false,
        shift: false,
    };
    const CTRL: Mods = Mods {
        ctrl: true,
        shift: false,
    };
    const SHIFT: Mods = Mods {
        ctrl: false,
        shift: true,
    };

    #[test]
    fn transport_keys() {
        assert_eq!(map(Key::Space, NONE, false), Some(KeyAction::TogglePlay));
        assert_eq!(map(Key::Char('n'), NONE, false), Some(KeyAction::Next));
        assert_eq!(map(Key::Char('p'), NONE, false), Some(KeyAction::Previous));
        assert_eq!(map(Key::Left, NONE, false), Some(KeyAction::SeekBy(-5)));
        assert_eq!(map(Key::Right, NONE, false), Some(KeyAction::SeekBy(5)));
        assert_eq!(map(Key::Left, SHIFT, false), Some(KeyAction::SeekBy(-30)));
        assert_eq!(map(Key::Right, SHIFT, false), Some(KeyAction::SeekBy(30)));
        assert_eq!(map(Key::Up, NONE, false), Some(KeyAction::VolumeBy(5)));
        assert_eq!(map(Key::Down, NONE, false), Some(KeyAction::VolumeBy(-5)));
    }

    #[test]
    fn search_settings_and_lyrics_keys() {
        assert_eq!(
            map(Key::Char('k'), CTRL, false),
            Some(KeyAction::OpenSearch)
        );
        assert_eq!(
            map(Key::Char('/'), NONE, false),
            Some(KeyAction::OpenSearch)
        );
        assert_eq!(
            map(Key::Char(','), CTRL, false),
            Some(KeyAction::OpenSettings)
        );
        assert_eq!(
            map(Key::Char('['), NONE, false),
            Some(KeyAction::LyricsOffsetBy(-50))
        );
        assert_eq!(
            map(Key::Char(']'), NONE, false),
            Some(KeyAction::LyricsOffsetBy(50))
        );
    }

    #[test]
    fn escape_and_f3_work_everywhere() {
        for in_text in [false, true] {
            assert_eq!(map(Key::Escape, NONE, in_text), Some(KeyAction::Close));
            assert_eq!(
                map(Key::F3, NONE, in_text),
                Some(KeyAction::ToggleFrameReadout)
            );
        }
    }

    #[test]
    fn single_keys_are_off_in_text_fields_but_ctrl_k_stays() {
        for key in [
            Key::Space,
            Key::Left,
            Key::Up,
            Key::Char('n'),
            Key::Char('p'),
            Key::Char('/'),
        ] {
            assert_eq!(map(key, NONE, true), None, "{key:?}");
        }
        assert_eq!(map(Key::Char('k'), CTRL, true), Some(KeyAction::OpenSearch));
    }

    #[test]
    fn ctrl_does_not_trigger_single_letter_actions() {
        assert_eq!(map(Key::Char('n'), CTRL, false), None);
        assert_eq!(map(Key::Space, CTRL, false), None);
        assert_eq!(map(Key::Char('x'), NONE, false), None);
    }
}
