//! Bundled fonts and icons. Licences are in `assets/LICENSES.md`.

use iced::Font;
use iced::font::{Family, Weight};
use iced::widget::svg;

/// IBM Plex Sans Regular (SIL OFL 1.1).
pub const FONT_REGULAR: &[u8] = include_bytes!("../../assets/fonts/IBMPlexSans-Regular.ttf");
/// IBM Plex Sans `SemiBold` (SIL OFL 1.1).
pub const FONT_SEMIBOLD: &[u8] = include_bytes!("../../assets/fonts/IBMPlexSans-SemiBold.ttf");
/// Family name of the bundled font.
pub const FAMILY: &str = "IBM Plex Sans";

/// `font.ui` and `font.display` (one bundled family). Digits in IBM Plex Sans are tabular by
/// default (every digit advances 600/1000 em), so times and rates do not jitter; iced 0.14 has
/// no OpenType feature switch, so the font itself provides the numerals (`font.mono-num`).
#[must_use]
pub const fn font(semibold: bool) -> Font {
    Font {
        family: Family::Name(FAMILY),
        weight: if semibold {
            Weight::Semibold
        } else {
            Weight::Normal
        },
        ..Font::DEFAULT
    }
}

/// The icon set: one stroke style on a 24 px grid, 1.5 px stroke.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Icon {
    /// House.
    Home,
    /// Magnifier.
    Search,
    /// Books.
    Library,
    /// Queue list.
    Queue,
    /// Sliders.
    Settings,
    /// Play triangle.
    Play,
    /// Pause bars.
    Pause,
    /// Previous track.
    Prev,
    /// Next track.
    Next,
    /// Speaker.
    Volume,
    /// Chevron left.
    ChevronLeft,
    /// Chevron right.
    ChevronRight,
    /// Chevron down.
    ChevronDown,
    /// Music note.
    Note,
    /// Cross.
    Close,
}

impl Icon {
    fn source(self) -> &'static [u8] {
        match self {
            Self::Home => include_bytes!("../../assets/icons/home.svg"),
            Self::Search => include_bytes!("../../assets/icons/search.svg"),
            Self::Library => include_bytes!("../../assets/icons/library.svg"),
            Self::Queue => include_bytes!("../../assets/icons/queue.svg"),
            Self::Settings => include_bytes!("../../assets/icons/settings.svg"),
            Self::Play => include_bytes!("../../assets/icons/play.svg"),
            Self::Pause => include_bytes!("../../assets/icons/pause.svg"),
            Self::Prev => include_bytes!("../../assets/icons/prev.svg"),
            Self::Next => include_bytes!("../../assets/icons/next.svg"),
            Self::Volume => include_bytes!("../../assets/icons/volume.svg"),
            Self::ChevronLeft => include_bytes!("../../assets/icons/chevron-left.svg"),
            Self::ChevronRight => include_bytes!("../../assets/icons/chevron-right.svg"),
            Self::ChevronDown => include_bytes!("../../assets/icons/chevron-down.svg"),
            Self::Note => include_bytes!("../../assets/icons/note.svg"),
            Self::Close => include_bytes!("../../assets/icons/close.svg"),
        }
    }

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
    fn every_icon_is_a_24_grid_svg() {
        for icon in [
            Icon::Home,
            Icon::Search,
            Icon::Library,
            Icon::Queue,
            Icon::Settings,
            Icon::Play,
            Icon::Pause,
            Icon::Prev,
            Icon::Next,
            Icon::Volume,
            Icon::ChevronLeft,
            Icon::ChevronRight,
            Icon::ChevronDown,
            Icon::Note,
            Icon::Close,
        ] {
            let s = std::str::from_utf8(icon.source()).expect("utf8");
            assert!(
                s.starts_with("<svg") && s.contains("viewBox=\"0 0 24 24\""),
                "{icon:?}"
            );
            assert!(s.contains("stroke-width=\"1.5\""), "{icon:?}");
        }
    }

    #[test]
    fn bundled_fonts_are_ttf() {
        for f in [FONT_REGULAR, FONT_SEMIBOLD] {
            assert_eq!(&f[..4], &[0, 1, 0, 0], "TrueType header");
        }
    }

    /// Reads the digit advances from the bundled font: all ten must be equal (tabular).
    #[test]
    fn digits_are_tabular_in_the_bundled_font() {
        for font in [FONT_REGULAR, FONT_SEMIBOLD] {
            let advances = digit_advances(font).expect("font tables");
            assert!(advances.iter().all(|a| *a == advances[0]), "{advances:?}");
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

    /// Minimal TrueType reader: cmap format 4 for U+0030..U+0039, then `hmtx`.
    fn digit_advances(d: &[u8]) -> Option<Vec<u16>> {
        let tables = usize::from(be16(d, 4)?);
        let find = |tag: &[u8; 4]| {
            (0..tables).find_map(|i| {
                let rec = 12 + 16 * i;
                (d.get(rec..rec + 4)? == tag).then(|| be32(d, rec + 8))?
            })
        };
        let cmap = find(b"cmap")?;
        let hmtx = find(b"hmtx")?;
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
        let glyph = |c: u16| {
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
        };
        (0x30_u16..=0x39)
            .map(|c| be16(d, hmtx + 4 * usize::from(glyph(c)?)))
            .collect()
    }
}
