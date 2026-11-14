//! Palette extraction from cover pixels: decode, median cut and seed choice.

use super::oklch::rgb_to_oklch;
use super::{
    GREY_CHROMA, GREY_IMAGE_FRACTION, MAX_CHROMA, MEDIAN_CUT_BOXES, MIN_SEED_CHROMA, Palette,
    SAMPLE_PX, Seed, Tones, build_palette,
};
use crate::theme::Rgba;

/// Why a palette could not be derived from image bytes.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum PaletteError {
    /// The bytes are not a supported image.
    #[error("could not decode cover image")]
    Decode,
    /// The pixel buffer does not match the stated size.
    #[error("pixel buffer does not match the image size")]
    BadBuffer,
}

/// Decode `bytes` (JPEG, PNG or WebP), shrink to 64 px and derive the palette.
///
/// # Errors
/// [`PaletteError::Decode`] when the bytes are not a supported image.
pub fn from_image_bytes(bytes: &[u8]) -> Result<Palette, PaletteError> {
    let img = image::load_from_memory(bytes).map_err(|_| PaletteError::Decode)?;
    let small = img.thumbnail(SAMPLE_PX, SAMPLE_PX).to_rgba8();
    from_rgba(small.width(), small.height(), small.as_raw())
}

/// Derive the palette from raw RGBA8 pixels (any size; large images are not resized here, so
/// callers shrink first).
///
/// # Errors
/// [`PaletteError::BadBuffer`] when `pixels.len() != width * height * 4`.
pub fn from_rgba(width: u32, height: u32, pixels: &[u8]) -> Result<Palette, PaletteError> {
    let expected = u64::from(width) * u64::from(height) * 4;
    if u64::try_from(pixels.len()).ok() != Some(expected) {
        return Err(PaletteError::BadBuffer);
    }
    let seed = seed_from_pixels(pixels);
    Ok(build_palette(seed, Tones::DARK))
}

/// Pick the seed (hue, chroma) of a cover from RGBA8 pixels.
#[must_use]
pub fn seed_from_pixels(pixels: &[u8]) -> Seed {
    match dominant_color(pixels) {
        Some(c) => {
            let lch = rgb_to_oklch(c);
            Seed {
                hue: lch.h,
                chroma: lch.c.clamp(MIN_SEED_CHROMA, MAX_CHROMA),
            }
        }
        // Whole image is grey (or empty): a neutral accent with no hue.
        None => Seed {
            hue: 0.0,
            chroma: 0.0,
        },
    }
}

/// Dominant non-grey colour by median cut, or `None` when the whole image is grey (or has no
/// opaque pixel). Boxes are scored by `count x (chroma + 0.04)`, so a small vivid area can win
/// over a large muddy one without letting noise dominate.
#[must_use]
pub fn dominant_color(pixels: &[u8]) -> Option<Rgba> {
    let mut colored: Vec<[u8; 3]> = Vec::new();
    let mut opaque = 0_usize;
    for px in pixels.as_chunks::<4>().0 {
        if px[3] < 128 {
            continue;
        }
        opaque += 1;
        let c = Rgba {
            r: px[0],
            g: px[1],
            b: px[2],
            a: 255,
        };
        if rgb_to_oklch(c).c >= GREY_CHROMA {
            colored.push([px[0], px[1], px[2]]);
        }
    }
    if opaque == 0 {
        return None;
    }
    // Counts are tiny (at most 4096), so the float conversion is exact.
    #[allow(clippy::cast_precision_loss)]
    let fraction = colored.len() as f64 / opaque as f64;
    if colored.is_empty() || fraction < GREY_IMAGE_FRACTION {
        return None;
    }
    let boxes = median_cut(colored, MEDIAN_CUT_BOXES);
    boxes
        .iter()
        .map(|b| {
            let avg = average(b);
            #[allow(clippy::cast_precision_loss)]
            let score = b.len() as f64 * (rgb_to_oklch(avg).c + 0.04);
            (score, avg)
        })
        .max_by(|a, b| a.0.total_cmp(&b.0))
        .map(|(_, c)| c)
}

fn average(pixels: &[[u8; 3]]) -> Rgba {
    let n = pixels.len().max(1) as u64;
    let sum = pixels.iter().fold([0_u64; 3], |mut acc, p| {
        for (a, v) in acc.iter_mut().zip(p) {
            *a += u64::from(*v);
        }
        acc
    });
    // The mean of u8 values fits in u8.
    #[allow(clippy::cast_possible_truncation)]
    let ch = |i: usize| (sum[i] / n) as u8;
    Rgba {
        r: ch(0),
        g: ch(1),
        b: ch(2),
        a: 255,
    }
}

/// Split the pixel set at the median of its widest channel until there are `target` boxes (or no
/// box can be split).
fn median_cut(pixels: Vec<[u8; 3]>, target: usize) -> Vec<Vec<[u8; 3]>> {
    let mut boxes = vec![pixels];
    while boxes.len() < target {
        // Split the box with the widest channel range among those with 2+ pixels.
        let pick = boxes
            .iter()
            .enumerate()
            .filter(|(_, b)| b.len() >= 2)
            .map(|(i, b)| (i, widest_channel(b)))
            .max_by_key(|(_, (_, range))| *range);
        let Some((index, (channel, range))) = pick else {
            break;
        };
        if range == 0 {
            break;
        }
        let mut b = boxes.swap_remove(index);
        b.sort_unstable_by_key(|p| p[channel]);
        let upper = b.split_off(b.len() / 2);
        boxes.push(b);
        boxes.push(upper);
    }
    boxes
}

fn widest_channel(pixels: &[[u8; 3]]) -> (usize, u8) {
    (0..3)
        .map(|ch| {
            let (lo, hi) = pixels.iter().fold((u8::MAX, u8::MIN), |(lo, hi), p| {
                (lo.min(p[ch]), hi.max(p[ch]))
            });
            (ch, hi - lo)
        })
        .max_by_key(|(_, range)| *range)
        .unwrap_or((0, 0))
}
