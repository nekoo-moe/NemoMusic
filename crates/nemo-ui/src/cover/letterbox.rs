//! The picture inside a letterboxed video thumbnail.
//!
//! YouTube's `hqdefault` and `sddefault` thumbnails are 4:3 with a 16:9 picture between two black
//! bars, each one eighth of the height. A 1:1 `cover` crop of the whole image would keep part of
//! the bars, so the bars are cut off first. Only that exact shape is touched: a 4:3 image whose
//! top and bottom eighths are (nearly) black.

use image::DynamicImage;

/// Channel value at or below which a pixel counts as black (JPEG noise stays under it).
const BLACK: u8 = 24;

/// Share of bar pixels, in percent, that must be black.
const BLACK_SHARE_PCT: u64 = 98;

/// `img` without letterbox bars, or `img` itself when it has none.
#[must_use]
pub fn strip(img: DynamicImage) -> DynamicImage {
    let (w, h) = (img.width(), img.height());
    // 4:3 within one pixel of rounding (480 x 360, 640 x 480).
    let four_by_three = h >= 8 && (u64::from(w) * 3).abs_diff(u64::from(h) * 4) <= 4;
    if !four_by_three {
        return img;
    }
    let bar = h / 8;
    let rgb = img.to_rgb8();
    let black_rows = |rows: std::ops::Range<u32>| {
        let mut black = 0_u64;
        let mut total = 0_u64;
        for y in rows {
            for x in 0..w {
                let p = rgb.get_pixel(x, y).0;
                total += 1;
                if p.iter().all(|&c| c <= BLACK) {
                    black += 1;
                }
            }
        }
        total > 0 && black * 100 >= total * BLACK_SHARE_PCT
    };
    if black_rows(0..bar) && black_rows(h - bar..h) {
        img.crop_imm(0, bar, w, h - 2 * bar)
    } else {
        img
    }
}

#[cfg(test)]
mod tests {
    use super::strip;
    use image::{DynamicImage, Rgb, RgbImage};

    fn letterboxed(w: u32, h: u32) -> DynamicImage {
        let bar = h / 8;
        DynamicImage::ImageRgb8(RgbImage::from_fn(w, h, |_, y| {
            if y < bar || y >= h - bar {
                Rgb([3, 2, 4])
            } else {
                Rgb([200, 40, 90])
            }
        }))
    }

    #[test]
    fn letterboxed_video_thumbnail_keeps_only_the_picture() {
        let out = strip(letterboxed(480, 360));
        assert_eq!((out.width(), out.height()), (480, 270));
        assert_eq!(out.to_rgb8().get_pixel(0, 0).0, [200, 40, 90]);
    }

    #[test]
    fn dark_or_other_shaped_images_are_left_alone() {
        // A square cover with black bands is art, not a letterbox.
        let square = strip(letterboxed(400, 400));
        assert_eq!((square.width(), square.height()), (400, 400));
        // A 4:3 picture that fills the frame.
        let full = DynamicImage::ImageRgb8(RgbImage::from_pixel(640, 480, Rgb([90, 90, 90])));
        assert_eq!(strip(full).height(), 480);
        // An all-black 4:3 image is cropped to its middle, still a valid picture.
        let black = DynamicImage::ImageRgb8(RgbImage::new(480, 360));
        assert_eq!(strip(black).height(), 270);
    }
}
