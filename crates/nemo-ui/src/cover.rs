//! Everything derived from one cover image, computed off the UI thread: the accent palette and
//! the pre-blurred backdrop texture used by the Tier Low glass bar and the Now Playing backdrop
//! (`design.md` sections 7 and 9). No GUI dependency.

#![allow(
    clippy::many_single_char_names,
    clippy::cast_precision_loss,
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    clippy::manual_midpoint
)] // colour and easing maths: short names are the standard notation, casts are of small bounded values

use std::io::Cursor;

use image::{ImageFormat, imageops};

use crate::palette::{self, Palette, PaletteError};
use crate::theme::{Rgba, color, contrast_ratio, glass};

/// Edge length of the pre-blurred texture. The GPU scales it up, which only softens it more.
pub const BACKDROP_PX: u32 = 96;
/// Gaussian sigma (in texture pixels) of the pre-blur. At 96 px this is a strong blur.
pub const BACKDROP_SIGMA: f32 = 5.0;

/// A cover, analysed.
#[derive(Debug, Clone, PartialEq)]
pub struct CoverData {
    /// Accent palette (dark surfaces).
    pub palette: Palette,
    /// Seed the palette was built from, so the app can rebuild it for another theme.
    pub seed: palette::Seed,
    /// Pre-blurred backdrop, PNG bytes (square, [`BACKDROP_PX`] wide).
    pub backdrop_png: Vec<u8>,
    /// The lightest pixel of the backdrop: the worst case behind light text.
    pub backdrop_lightest: Rgba,
    /// Average colour of the left, middle and right thirds of the blurred backdrop: the glass
    /// bar paints these as a gradient (a 3-sample version of the pre-blurred texture).
    pub backdrop_strip: [Rgba; 3],
    /// The song-coloured canvas for the Now Playing pane and the full-screen Now Playing.
    pub canvas: crate::canvas::Canvas,
}

/// Decode `bytes`, derive the palette and the blurred backdrop.
///
/// # Errors
/// [`PaletteError::Decode`] when the bytes are not a supported image.
pub fn analyze(bytes: &[u8]) -> Result<CoverData, PaletteError> {
    let img = image::load_from_memory(bytes).map_err(|_| PaletteError::Decode)?;
    let sample = img
        .thumbnail(palette::SAMPLE_PX, palette::SAMPLE_PX)
        .to_rgba8();
    let palette = palette::from_rgba(sample.width(), sample.height(), sample.as_raw())?;
    let seed = palette::seed_from_pixels(sample.as_raw());

    // `cover` crop to a square first so the blurred texture has the cover's aspect (1:1).
    let (w, h) = (img.width(), img.height());
    let side = w.min(h).max(1);
    let square = img.crop_imm((w - side) / 2, (h - side) / 2, side, side);
    let small = imageops::resize(
        &square.to_rgba8(),
        BACKDROP_PX,
        BACKDROP_PX,
        imageops::FilterType::Triangle,
    );
    let blurred = imageops::blur(&small, BACKDROP_SIGMA);
    let lightest = blurred
        .pixels()
        .map(|p| Rgba {
            r: p.0[0],
            g: p.0[1],
            b: p.0[2],
            a: 255,
        })
        .max_by(|a, b| a.relative_luminance().total_cmp(&b.relative_luminance()))
        .unwrap_or(color::SURFACE);
    let mut backdrop_png = Vec::new();
    blurred
        .write_to(&mut Cursor::new(&mut backdrop_png), ImageFormat::Png)
        .map_err(|_| PaletteError::Decode)?;
    let strip = [0_u32, 1, 2].map(|third| {
        let (x0, x1) = (third * BACKDROP_PX / 3, (third + 1) * BACKDROP_PX / 3);
        let mut sum = [0_u64; 3];
        let mut n = 0_u64;
        for y in 0..BACKDROP_PX {
            for x in x0..x1 {
                let p = blurred.get_pixel(x, y).0;
                for i in 0..3 {
                    sum[i] += u64::from(p[i]);
                }
                n += 1;
            }
        }
        // The mean of u8 values fits in u8.
        #[allow(clippy::cast_possible_truncation)]
        let ch = |i: usize| (sum[i] / n.max(1)) as u8;
        Rgba {
            r: ch(0),
            g: ch(1),
            b: ch(2),
            a: 255,
        }
    });
    Ok(CoverData {
        palette,
        seed,
        backdrop_png,
        backdrop_lightest: lightest,
        backdrop_strip: strip,
        canvas: crate::canvas::build(blurred.as_raw(), BACKDROP_PX),
    })
}

/// Decode `bytes` and cut the corners to transparent with a radius of `fraction` x the image
/// width (0.5 gives a circle). Returns PNG bytes. Done here because the tiny-skia renderer
/// ignores `border_radius` on images, so rounding is baked into the texture for every renderer.
///
/// # Errors
/// [`PaletteError::Decode`] when the bytes are not a supported image.
pub fn round_corners(bytes: &[u8], fraction: f32) -> Result<Vec<u8>, PaletteError> {
    let mut img = image::load_from_memory(bytes)
        .map_err(|_| PaletteError::Decode)?
        .to_rgba8();
    cut_corners(&mut img, fraction);
    let mut out = Vec::new();
    img.write_to(&mut Cursor::new(&mut out), ImageFormat::Png)
        .map_err(|_| PaletteError::Decode)?;
    Ok(out)
}

/// Make the corners of `img` transparent: radius `fraction` x width, anti-aliased over one px.
fn cut_corners(img: &mut image::RgbaImage, fraction: f32) {
    let (w, h) = (img.width(), img.height());
    // Image sizes are small, exact in f32.
    #[allow(clippy::cast_precision_loss)]
    let (wf, hf) = (w as f32, h as f32);
    let r = (fraction.clamp(0.0, 0.5) * wf).max(0.0);
    for (x, y, p) in img.enumerate_pixels_mut() {
        #[allow(clippy::cast_precision_loss)]
        let (px, py) = (x as f32 + 0.5, y as f32 + 0.5);
        // Distance outside the rounded rectangle, in px (anti-aliased over one px).
        let cx = px.clamp(r, (wf - r).max(r));
        let cy = py.clamp(r, (hf - r).max(r));
        let d = ((px - cx).powi(2) + (py - cy).powi(2)).sqrt() - r;
        let coverage = (0.5 - d).clamp(0.0, 1.0);
        if coverage < 1.0 {
            // Coverage is 0..1, so the product stays within u8.
            #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
            let a = (f32::from(p.0[3]) * coverage).round() as u8;
            p.0[3] = a;
        }
    }
}

/// A decoded, cropped, scaled picture as raw RGBA pixels: iced shows it without decoding
/// anything on the UI thread (`Handle::from_rgba`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Baked {
    /// Width in px.
    pub width: u32,
    /// Height in px.
    pub height: u32,
    /// `width * height * 4` bytes, straight alpha.
    pub rgba: Vec<u8>,
}

/// Decode `bytes`, crop to a centred square (`cover`), shrink to at most `max_px` (a smaller
/// source stays at its own size: covers are never upscaled) and cut the corners with a radius of
/// `fraction` x width (0.5 gives a circle). Runs on a worker thread.
///
/// # Errors
/// [`PaletteError::Decode`] when the bytes are not a supported image.
pub fn bake(bytes: &[u8], max_px: u32, fraction: f32) -> Result<Baked, PaletteError> {
    let img = image::load_from_memory(bytes).map_err(|_| PaletteError::Decode)?;
    let (w, h) = (img.width(), img.height());
    let side = w.min(h).max(1);
    let square = img.crop_imm((w - side) / 2, (h - side) / 2, side, side);
    let target = side.min(max_px.max(1));
    let mut out = if target < side {
        imageops::resize(
            &square.to_rgba8(),
            target,
            target,
            imageops::FilterType::Triangle,
        )
    } else {
        square.to_rgba8()
    };
    cut_corners(&mut out, fraction);
    Ok(Baked {
        width: out.width(),
        height: out.height(),
        rgba: out.into_raw(),
    })
}
/// Tint alpha for the glass bar over a backdrop whose lightest colour is `backdrop`
/// (`design.md` section 6): start at 0.55 and raise in 0.05 steps up to 0.90 until both
/// `on-surface` and `on-surface-variant` reach 4.5:1 over tint plus backdrop. `None` means the
/// bar must use the tonal fallback.
#[must_use]
pub fn glass_tint_alpha(backdrop: Rgba) -> Option<f32> {
    let mut alpha = glass::TINT_ALPHA;
    while alpha <= glass::TINT_ALPHA_MAX + 1e-4 {
        let bar = glass::TINT_BASE.with_alpha(alpha).over(backdrop);
        let worst = contrast_ratio(color::ON_SURFACE, bar)
            .min(contrast_ratio(color::ON_SURFACE_VARIANT, bar));
        if worst >= palette::TEXT_CONTRAST {
            return Some(alpha);
        }
        alpha += glass::TINT_ALPHA_STEP;
    }
    None
}

/// Opacity of the dimmed lyric lines (`on-surface-variant` at 0.72, `design.md` section 10).
pub const LYRIC_DIM_ALPHA: f32 = 0.72;
/// Lowest scrim alpha over the blurred Now Playing backdrop (`design.md` section 9: >= 0.60).
pub const SCRIM_ALPHA_MIN: f32 = 0.72;

/// Scrim alpha for the Now Playing backdrop whose lightest colour is `backdrop`: from 0.72 in
/// 0.04 steps until the dimmest text (`on-surface-variant` at 0.72) reaches 4.5:1; at worst the
/// backdrop is fully covered by `surface`.
#[must_use]
pub fn backdrop_scrim_alpha(backdrop: Rgba) -> f32 {
    let mut alpha = SCRIM_ALPHA_MIN;
    while alpha < 1.0 {
        let bg = color::SURFACE.with_alpha(alpha).over(backdrop);
        let dim = color::ON_SURFACE_VARIANT
            .with_alpha(LYRIC_DIM_ALPHA)
            .over(bg);
        if contrast_ratio(dim, bg) >= palette::TEXT_CONTRAST {
            return alpha;
        }
        alpha += 0.04;
    }
    1.0
}
#[cfg(test)]
mod tests {
    use super::*;
    use image::{Rgba as ImgRgba, RgbaImage};

    fn png(img: &RgbaImage) -> Vec<u8> {
        let mut out = Vec::new();
        img.write_to(&mut Cursor::new(&mut out), ImageFormat::Png)
            .expect("invariant: encoding a small RGBA image cannot fail");
        out
    }

    #[test]
    fn bake_crops_square_never_upscales_and_cuts_corners() {
        let wide = RgbaImage::from_pixel(200, 100, ImgRgba([200, 40, 40, 255]));
        let small = bake(&png(&wide), 400, 0.12).expect("bake");
        assert_eq!(
            (small.width, small.height),
            (100, 100),
            "square, not upscaled"
        );
        assert_eq!(small.rgba.len(), 100 * 100 * 4);
        assert_eq!(small.rgba[3], 0, "the corner pixel is transparent");
        let mid = (50 * 100 + 50) * 4;
        assert_eq!(small.rgba[mid + 3], 255);
        let big = RgbaImage::from_pixel(600, 600, ImgRgba([10, 200, 40, 255]));
        let shrunk = bake(&png(&big), 150, 0.5).expect("bake");
        assert_eq!((shrunk.width, shrunk.height), (150, 150));
        assert!(bake(b"not an image", 100, 0.1).is_err());
    }

    #[test]
    fn backdrop_is_a_small_square_png() {
        let bytes = png(&RgbaImage::from_pixel(
            300,
            200,
            ImgRgba([200, 40, 60, 255]),
        ));
        let data = analyze(&bytes).expect("decodes");
        let back = image::load_from_memory(&data.backdrop_png).expect("png");
        assert_eq!((back.width(), back.height()), (BACKDROP_PX, BACKDROP_PX));
        assert!(data.palette.passes());
    }

    #[test]
    fn rounding_clears_the_corners_and_keeps_the_middle() {
        let bytes = png(&RgbaImage::from_pixel(
            100,
            100,
            ImgRgba([200, 40, 60, 255]),
        ));
        let out = round_corners(&bytes, 0.12).expect("ok");
        let img = image::load_from_memory(&out).expect("png").to_rgba8();
        assert_eq!(img.get_pixel(0, 0).0[3], 0);
        assert_eq!(img.get_pixel(99, 99).0[3], 0);
        assert_eq!(img.get_pixel(50, 50).0[3], 255);
        assert_eq!(img.get_pixel(50, 0).0[3], 255, "edges between corners stay");
        let circle = image::load_from_memory(&round_corners(&bytes, 0.5).expect("ok"))
            .expect("png")
            .to_rgba8();
        assert_eq!(circle.get_pixel(50, 2).0[3], 255);
        assert_eq!(circle.get_pixel(5, 5).0[3], 0);
        assert!(round_corners(b"x", 0.1).is_err());
    }

    #[test]
    fn backdrop_strip_follows_the_cover_left_to_right() {
        let mut img = RgbaImage::new(90, 90);
        for (x, _, p) in img.enumerate_pixels_mut() {
            *p = if x < 45 {
                ImgRgba([250, 0, 0, 255])
            } else {
                ImgRgba([0, 0, 250, 255])
            };
        }
        let d = analyze(&png(&img)).expect("ok");
        assert!(d.backdrop_strip[0].r > 200 && d.backdrop_strip[2].b > 200);
    }

    #[test]
    fn garbage_is_an_error() {
        assert_eq!(analyze(b"nope"), Err(PaletteError::Decode));
    }

    #[test]
    fn tint_alpha_rises_over_a_light_backdrop_and_stays_default_over_a_dark_one() {
        let dark = glass_tint_alpha(Rgba::from_rgb_hex(0x10_1010));
        assert_eq!(dark, Some(glass::TINT_ALPHA));
        let white = glass_tint_alpha(Rgba::from_rgb_hex(0xFF_FFFF));
        let alpha = white.expect("white is repairable with a stronger tint");
        assert!(alpha > glass::TINT_ALPHA && alpha <= glass::TINT_ALPHA_MAX + 1e-4);
        let bar = glass::TINT_BASE
            .with_alpha(alpha)
            .over(Rgba::from_rgb_hex(0xFF_FFFF));
        assert!(contrast_ratio(color::ON_SURFACE, bar) >= 4.5);
    }

    #[test]
    fn scrim_keeps_dim_lyrics_legible_over_any_backdrop() {
        for hex in [0x00_0000, 0x80_8080, 0xFF_FFFF, 0xFF_E000, 0x00_FFFF] {
            let back = Rgba::from_rgb_hex(hex);
            let a = backdrop_scrim_alpha(back);
            assert!(a >= SCRIM_ALPHA_MIN);
            let bg = color::SURFACE.with_alpha(a).over(back);
            let dim = color::ON_SURFACE_VARIANT
                .with_alpha(LYRIC_DIM_ALPHA)
                .over(bg);
            assert!(contrast_ratio(dim, bg) >= 4.5, "{hex:06X}");
            assert!(contrast_ratio(color::ON_SURFACE, bg) >= 4.5, "{hex:06X}");
        }
        assert!((backdrop_scrim_alpha(Rgba::from_rgb_hex(0)) - SCRIM_ALPHA_MIN).abs() < 1e-6);
    }

    #[test]
    fn analysing_white_covers_gives_a_legible_bar() {
        let bytes = png(&RgbaImage::from_pixel(
            64,
            64,
            ImgRgba([255, 255, 255, 255]),
        ));
        let data = analyze(&bytes).expect("decodes");
        assert!(glass_tint_alpha(data.backdrop_lightest).is_some());
    }
}
