//! Bundled fonts and icons. Licences are in `assets/LICENSES.md`.

use iced::Font;
use iced::font::{Family, Weight};
use iced::widget::svg;

/// Google Sans Flex Regular, 400 (SIL OFL 1.1, tabular figures baked in).
pub const FONT_REGULAR: &[u8] = include_bytes!("../../assets/fonts/GoogleSansFlex-Regular.ttf");
/// Google Sans Flex Medium, 500.
pub const FONT_MEDIUM: &[u8] = include_bytes!("../../assets/fonts/GoogleSansFlex-Medium.ttf");
/// Google Sans Flex `SemiBold`, 600.
pub const FONT_SEMIBOLD: &[u8] = include_bytes!("../../assets/fonts/GoogleSansFlex-SemiBold.ttf");
/// Family name of the bundled font.
pub const FAMILY: &str = "Google Sans Flex";

/// `font.ui` and `font.display` (one bundled family) at 400 or 600. The digits of the bundled
/// files are the font's own tabular figures (`tnum` mapped onto the default glyphs, see
/// `assets/LICENSES.md`), so times and rates do not jitter; iced 0.14 cannot switch OpenType
/// features on.
#[must_use]
pub const fn font(semibold: bool) -> Font {
    font_w(if semibold {
        Weight::Semibold
    } else {
        Weight::Normal
    })
}

/// The bundled family at an exact weight: `Normal`, `Medium` or `Semibold`.
#[must_use]
pub const fn font_w(weight: Weight) -> Font {
    Font {
        family: Family::Name(FAMILY),
        weight,
        ..Font::DEFAULT
    }
}

macro_rules! icons {
    ($($(#[$doc:meta])* $name:ident => $file:literal,)+) => {
        /// The icon set: Material Symbols Rounded (weight 400, 24 px grid, Apache-2.0).
        /// Names ending in `Fill` are the filled variants.
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
        pub enum Icon {
            $($(#[$doc])* $name,)+
        }

        impl Icon {
            /// Every icon, for tests.
            pub const ALL: &'static [Icon] = &[$(Icon::$name,)+];

            fn source(self) -> &'static [u8] {
                match self {
                    $(Self::$name => include_bytes!(concat!("../../assets/icons/", $file, ".svg")),)+
                }
            }
        }
    };
}

icons! {
    /// House.
    Home => "home",
    /// House, filled (active navigation).
    HomeFill => "home-fill",
    /// Magnifier.
    Search => "search",
    /// Magnifier, filled.
    SearchFill => "search-fill",
    /// Library.
    Library => "library-music",
    /// Library, filled.
    LibraryFill => "library-music-fill",
    /// Queue list.
    Queue => "queue-music",
    /// Settings gear.
    Settings => "settings",
    /// Play triangle, filled.
    Play => "play-arrow-fill",
    /// Pause bars, filled.
    Pause => "pause-fill",
    /// Previous track, filled.
    Prev => "skip-previous-fill",
    /// Next track, filled.
    Next => "skip-next-fill",
    /// Speaker.
    Volume => "volume-up",
    /// Speaker, muted.
    VolumeOff => "volume-off",
    /// Chevron left.
    ChevronLeft => "chevron-left",
    /// Chevron right.
    ChevronRight => "chevron-right",
    /// Chevron down.
    ChevronDown => "expand-more",
    /// Music note.
    Note => "music-note",
    /// Cross.
    Close => "close",
    /// Three dots.
    More => "more-vert",
    /// Crossing arrows.
    Shuffle => "shuffle",
    /// Progress ring with a gap.
    Spinner => "progress-activity",
    /// Circle with an i.
    Info => "info",
    /// Repeat.
    Repeat => "repeat",
    /// Repeat the current song.
    RepeatOne => "repeat-one",
    /// Heart outline.
    Heart => "favorite",
    /// Heart, filled (liked).
    HeartFill => "favorite-fill",
    /// Music note with a slash (nothing to show).
    MusicOff => "music-off",
    /// Plus.
    Add => "add",
    /// Compass.
    Explore => "explore",
    /// Person.
    Person => "person",
    /// Person, filled.
    PersonFill => "person-fill",
    /// Arrow out of a door.
    Logout => "logout",
    /// Key.
    Key => "key",
    /// Check mark.
    Check => "check",
    /// Check mark in a circle, filled.
    CheckCircle => "check-circle-fill",
    /// Lyrics.
    Lyrics => "lyrics",
    /// Right pane, close.
    PanelClose => "right-panel-close",
    /// Right pane, open.
    PanelOpen => "right-panel-open",
    /// Album disc.
    Album => "album",
    /// Microphone (artist).
    Mic => "mic-external-on",
    /// Playlist.
    Playlist => "playlist-play",
    /// Clock with arrow.
    History => "history",
    /// Clipboard.
    Paste => "content-paste",
    /// Arrow left.
    ArrowBack => "arrow-back",
    /// Arrow right.
    ArrowForward => "arrow-forward",
    /// Sliders.
    Tune => "tune",
    /// Lock.
    Lock => "lock",
    /// Arrows out of a box (open the full window).
    Expand => "open-in-full",
    /// Arrow into a door (sign in).
    Login => "login",
    /// Sleep timer.
    Sleep => "bedtime",
    /// Keyboard shortcuts.
    Keyboard => "keyboard",
    /// Sort order.
    Sort => "sort",
    /// Grid view.
    Grid => "grid-view",
    /// List view.
    List => "format-list-bulleted",
    /// Radio.
    Radio => "sensors",
    /// Link.
    Link => "link",
    /// Podcasts.
    Podcasts => "podcasts",
    /// Videos.
    Video => "smart-display",
    /// Drag handle.
    Drag => "drag-indicator",
    /// Enter full screen.
    FullScreen => "fullscreen",
    /// Leave full screen.
    FullScreenExit => "fullscreen-exit",
    /// Mini player.
    Mini => "picture-in-picture-alt",
    /// Playback speed.
    Speed => "speed",
    /// Folder.
    Folder => "folder",
    /// Light theme.
    LightMode => "light-mode",
    /// Dark theme.
    DarkMode => "dark-mode",
    /// Palette.
    Palette => "palette",
    /// Up.
    ArrowUp => "arrow-upward",
    /// Down.
    ArrowDown => "arrow-downward",
    /// Add to playlist.
    PlaylistAdd => "playlist-add",
    /// Play next.
    PlayNext => "queue-play-next",
    /// Collapse.
    ExpandLess => "expand-less",
    /// Timer.
    Timer => "timer",
    /// Add to library.
    LibraryAdd => "library-add",
}

impl Icon {
    /// An SVG handle for this icon.
    #[must_use]
    pub fn handle(self) -> svg::Handle {
        svg::Handle::from_memory(self.source())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_icon_is_a_material_symbol_svg() {
        for icon in Icon::ALL {
            let s = std::str::from_utf8(icon.source()).expect("utf8");
            assert!(
                s.starts_with("<svg") && s.contains("viewBox=\"0 -960 960 960\""),
                "{icon:?}"
            );
            assert!(s.contains("<path"), "{icon:?}");
        }
    }

    #[test]
    fn bundled_fonts_are_ttf() {
        for f in [FONT_REGULAR, FONT_MEDIUM, FONT_SEMIBOLD] {
            assert_eq!(&f[..4], &[0, 1, 0, 0], "TrueType header");
        }
    }

    /// Reads the digit advances from the bundled font: all ten must be equal (tabular).
    #[test]
    fn digits_are_tabular_in_the_bundled_font() {
        for font in [FONT_REGULAR, FONT_MEDIUM, FONT_SEMIBOLD] {
            let advances = digit_advances(font).expect("font tables");
            assert!(advances.iter().all(|a| *a == advances[0]), "{advances:?}");
        }
    }

    /// The Vietnamese letters of the owner's test strings are in the font.
    #[test]
    fn vietnamese_letters_have_glyphs() {
        for font in [FONT_REGULAR, FONT_MEDIUM, FONT_SEMIBOLD] {
            for c in "M\u{1edb}i Quen Nhau \u{110}\u{e3} R\u{1ee7} Ph\u{1b0}\u{1a1}ng \u{1ea0}\u{1ee9}\u{1ef9}".chars() {
                assert!(has_glyph(font, c), "missing {c:?}");
            }
        }
    }

    fn be16(d: &[u8], at: usize) -> Option<u16> {
        Some(u16::from_be_bytes([*d.get(at)?, *d.get(at + 1)?]))
    }

    fn be32(d: &[u8], at: usize) -> Option<usize> {
        Some(u32::from_be_bytes([
            *d.get(at)?,
            *d.get(at + 1)?,
            *d.get(at + 2)?,
            *d.get(at + 3)?,
        ]) as usize)
    }

    /// Glyph index of a BMP character through the cmap format 4 subtable.
    fn glyph_of(d: &[u8], c: u16) -> Option<u16> {
        let tables = usize::from(be16(d, 4)?);
        let cmap = (0..tables).find_map(|i| {
            let rec = 12 + 16 * i;
            (d.get(rec..rec + 4)? == b"cmap").then(|| be32(d, rec + 8))?
        })?;
        let subtables = usize::from(be16(d, cmap + 2)?);
        let fmt4 = (0..subtables).find_map(|i| {
            let off = cmap + be32(d, cmap + 4 + 8 * i + 4)?;
            (be16(d, off)? == 4).then_some(off)
        })?;
        let segs = usize::from(be16(d, fmt4 + 6)?) / 2;
        let ends = fmt4 + 14;
        let starts = ends + 2 * segs + 2;
        let deltas = starts + 2 * segs;
        let offsets = deltas + 2 * segs;
        (0..segs).find_map(|i| {
            let (end, start) = (be16(d, ends + 2 * i)?, be16(d, starts + 2 * i)?);
            if c < start || c > end {
                return None;
            }
            let delta = be16(d, deltas + 2 * i)?;
            let range = be16(d, offsets + 2 * i)?;
            if range == 0 {
                return Some(c.wrapping_add(delta));
            }
            let at = offsets + 2 * i + usize::from(range) + 2 * usize::from(c - start);
            Some(be16(d, at)?.wrapping_add(delta))
        })
    }

    fn has_glyph(d: &[u8], c: char) -> bool {
        u16::try_from(u32::from(c))
            .ok()
            .and_then(|c| glyph_of(d, c))
            .is_some_and(|g| g != 0)
    }

    /// Minimal TrueType reader: cmap format 4 for U+0030..U+0039, then `hmtx`.
    fn digit_advances(d: &[u8]) -> Option<Vec<u16>> {
        let tables = usize::from(be16(d, 4)?);
        let hmtx = (0..tables).find_map(|i| {
            let rec = 12 + 16 * i;
            (d.get(rec..rec + 4)? == b"hmtx").then(|| be32(d, rec + 8))?
        })?;
        (0x30_u16..=0x39)
            .map(|c| be16(d, hmtx + 4 * usize::from(glyph_of(d, c)?)))
            .collect()
    }
}
