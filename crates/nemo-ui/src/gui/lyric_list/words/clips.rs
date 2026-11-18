//! A rectangle with holes, as a short list of disjoint rectangles in a fixed buffer, so a draw
//! allocates nothing.

use iced::Rectangle;

/// Most rectangles kept: one hole splits a rectangle into at most four.
const CAPACITY: usize = 40;
/// Most holes cut: each adds at most three rectangles.
const MAX_HOLES: usize = 10;

/// A rectangle minus the holes cut out of it.
pub(super) struct Clips {
    items: [Rectangle; CAPACITY],
    len: usize,
    holes: usize,
}

impl Clips {
    /// The whole of `whole`, no holes yet.
    pub(super) fn new(whole: Rectangle) -> Self {
        let mut items = [Rectangle::default(); CAPACITY];
        items[0] = whole;
        Self {
            items,
            len: 1,
            holes: 0,
        }
    }

    /// Cut `hole` out of every rectangle. Holes past [`MAX_HOLES`] are ignored.
    pub(super) fn cut(&mut self, hole: Rectangle) {
        if self.holes >= MAX_HOLES {
            return;
        }
        self.holes += 1;
        let mut next = [Rectangle::default(); CAPACITY];
        let mut n = 0;
        let mut push = |r: Rectangle, n: &mut usize| {
            if r.width > 0.0 && r.height > 0.0 && *n < CAPACITY {
                next[*n] = r;
                *n += 1;
            }
        };
        for piece in &self.items[..self.len] {
            let Some(cut) = piece.intersection(&hole) else {
                push(*piece, &mut n);
                continue;
            };
            // Above and below the hole span the whole piece; left and right only its height.
            let (top, bottom) = (
                cut.y - piece.y,
                piece.y + piece.height - (cut.y + cut.height),
            );
            let (left, right) = (cut.x - piece.x, piece.x + piece.width - (cut.x + cut.width));
            push(
                Rectangle {
                    height: top,
                    ..*piece
                },
                &mut n,
            );
            push(
                Rectangle {
                    y: cut.y + cut.height,
                    height: bottom,
                    ..*piece
                },
                &mut n,
            );
            push(
                Rectangle {
                    y: cut.y,
                    height: cut.height,
                    width: left,
                    ..*piece
                },
                &mut n,
            );
            push(
                Rectangle {
                    x: cut.x + cut.width,
                    y: cut.y,
                    width: right,
                    height: cut.height,
                },
                &mut n,
            );
        }
        self.items = next;
        self.len = n;
    }

    /// The rectangles left.
    pub(super) fn iter(&self) -> impl Iterator<Item = Rectangle> + '_ {
        self.items[..self.len].iter().copied()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rect(x: f32, y: f32, width: f32, height: f32) -> Rectangle {
        Rectangle {
            x,
            y,
            width,
            height,
        }
    }

    fn area(c: &Clips) -> f32 {
        c.iter().map(|r| r.width * r.height).sum()
    }

    #[test]
    fn no_hole_leaves_the_whole_rectangle() {
        let c = Clips::new(rect(0.0, 0.0, 100.0, 50.0));
        assert_eq!(c.iter().count(), 1);
        assert!((area(&c) - 5000.0).abs() < 1e-3);
    }

    #[test]
    fn holes_leave_disjoint_pieces_of_the_right_total_area() {
        let mut c = Clips::new(rect(0.0, 0.0, 100.0, 50.0));
        c.cut(rect(10.0, 10.0, 20.0, 20.0));
        c.cut(rect(50.0, -5.0, 10.0, 70.0));
        // 100 x 50 minus 20 x 20 minus the 10 x 50 strip (the part outside the whole is no hole).
        assert!(
            (area(&c) - (5000.0 - 400.0 - 500.0)).abs() < 1e-2,
            "{}",
            area(&c)
        );
        let pieces: Vec<Rectangle> = c.iter().collect();
        for (i, a) in pieces.iter().enumerate() {
            for b in &pieces[i + 1..] {
                let overlap = a.intersection(b).map_or(0.0, |o| o.width * o.height);
                assert!(overlap < 1e-3, "{a:?} overlaps {b:?}");
            }
        }
    }

    #[test]
    fn a_hole_that_misses_changes_nothing_and_a_covering_hole_leaves_nothing() {
        let mut c = Clips::new(rect(0.0, 0.0, 10.0, 10.0));
        c.cut(rect(50.0, 50.0, 5.0, 5.0));
        assert_eq!(c.iter().count(), 1);
        c.cut(rect(-1.0, -1.0, 20.0, 20.0));
        assert_eq!(c.iter().count(), 0);
    }
}
