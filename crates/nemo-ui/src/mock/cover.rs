//! Procedurally generated mock covers.

use std::collections::HashMap;
use std::io::Cursor;
use std::sync::{Arc, Mutex};

use super::COVER_PREFIX;
use crate::palette::{Oklch, oklch_to_rgb};
use image::{ImageFormat, Rgb, RgbImage};
use nemo_core::BoxFuture;
use nemo_core::services::{ImageError, ImageSource};
use nemo_core::track::{Artwork, pick_thumbnail};

/// Cover bytes by (cover index, size).
type CoverCache = HashMap<(usize, u32), Arc<Vec<u8>>>;

/// Procedurally generated covers: a diagonal two-hue gradient with a soft shape, as PNG.
#[derive(Default)]
pub struct MockImages {
    cache: Mutex<CoverCache>,
}

/// Render cover `n` at `px` x `px` as a PNG.
///
/// # Panics
/// Never in practice: encoding an in-memory RGB image to PNG cannot fail.
#[must_use]
pub fn cover_png(n: usize, px: u32) -> Vec<u8> {
    let px = px.clamp(16, 1024);
    // Hues spread by the golden angle so neighbours differ.
    let hue = (n as f64 * 137.5).rem_euclid(360.0);
    let hue2 = (hue + 50.0 + (n % 3) as f64 * 20.0).rem_euclid(360.0);
    let ink = |l: f64, c: f64, h: f64| {
        let rgb = oklch_to_rgb(Oklch { l, c, h });
        [rgb.r, rgb.g, rgb.b]
    };
    let (a, b, shape) = (
        ink(0.42, 0.13, hue),
        ink(0.72, 0.13, hue2),
        ink(0.90, 0.07, hue2),
    );
    let mut img = RgbImage::new(px, px);
    let size = px as f32;
    let centre = (0.3 + 0.1 * (n % 5) as f32, 0.7 - 0.08 * (n % 4) as f32);
    for (x, y, pixel) in img.enumerate_pixels_mut() {
        let (u, v) = (x as f32 / size, y as f32 / size);
        let t = (0.5 * (u + v)).clamp(0.0, 1.0);
        let mut c = [0_u8; 3];
        for i in 0..3 {
            c[i] = (f32::from(a[i]) * (1.0 - t) + f32::from(b[i]) * t).round() as u8;
        }
        // A soft disc, different position per cover.
        let d = ((u - centre.0).powi(2) + (v - centre.1).powi(2)).sqrt();
        let m = (1.0 - ((d - 0.22) / 0.03).clamp(0.0, 1.0)) * 0.55;
        for i in 0..3 {
            c[i] = (f32::from(c[i]) * (1.0 - m) + f32::from(shape[i]) * m).round() as u8;
        }
        *pixel = Rgb(c);
    }
    let mut out = Vec::new();
    img.write_to(&mut Cursor::new(&mut out), ImageFormat::Png)
        .expect("invariant: encoding an in-memory RGB image cannot fail");
    out
}

impl ImageSource for MockImages {
    fn image<'a>(
        &'a self,
        artwork: &'a Artwork,
        target_px: u32,
    ) -> BoxFuture<'a, Result<Vec<u8>, ImageError>> {
        let result = match artwork {
            Artwork::Remote(thumbs) => pick_thumbnail(thumbs, target_px)
                .and_then(|t| t.url.strip_prefix(COVER_PREFIX))
                .and_then(|n| n.parse::<usize>().ok())
                .map(|n| {
                    // Never produce more pixels than the picked thumbnail has.
                    let px = target_px.clamp(64, 544);
                    let mut cache = self
                        .cache
                        .lock()
                        .unwrap_or_else(std::sync::PoisonError::into_inner);
                    Arc::clone(
                        cache
                            .entry((n, px))
                            .or_insert_with(|| Arc::new(cover_png(n, px))),
                    )
                })
                .map(|bytes| bytes.as_ref().clone())
                .ok_or(ImageError::Missing),
            Artwork::LocalFile(_) => Err(ImageError::Read("mock has no files".to_owned())),
            Artwork::None => Err(ImageError::Missing),
        };
        Box::pin(async move { result })
    }
}
