//! Straight lines, pixel by pixel.
#![allow(
    clippy::arithmetic_side_effects,
    reason = "positions are within 2^18 of the origin (a pixel within 65535, or an arc point at most a radius of 2^17 further), so differences and error terms fit i32"
)]

use crate::geometry::Point;

/// Calls `visit` for every pixel from `from` to `to`, both included, in
/// order; stops early if `visit` returns false.
///
/// An integer line: the error term carries the minor axis, so the same two
/// ends always give the same pixels.
pub(crate) fn walk(from: Point, to: Point, mut visit: impl FnMut(Point) -> bool) {
    let dx = (to.x - from.x).abs();
    let dy = -(to.y - from.y).abs();
    let step_x = if from.x < to.x { 1 } else { -1 };
    let step_y = if from.y < to.y { 1 } else { -1 };
    let mut error = dx + dy;
    let mut at = from;
    loop {
        if !visit(at) || at == to {
            return;
        }
        let doubled = 2 * error;
        if doubled >= dy {
            error += dy;
            at.x += step_x;
        }
        if doubled <= dx {
            error += dx;
            at.y += step_y;
        }
    }
}

#[cfg(test)]
mod tests {
    use alloc::vec::Vec;

    use super::walk;
    use crate::geometry::Point;

    fn pixels(from: (i32, i32), to: (i32, i32)) -> Vec<(i32, i32)> {
        let mut seen = Vec::new();
        walk(Point::new(from.0, from.1), Point::new(to.0, to.1), |point| {
            seen.push((point.x, point.y));
            true
        });
        seen
    }

    #[test]
    fn both_ends_are_drawn_and_a_point_is_one_pixel() {
        assert_eq!(pixels((2, 2), (2, 2)), [(2, 2)]);
        assert_eq!(pixels((0, 0), (3, 0)), [(0, 0), (1, 0), (2, 0), (3, 0)]);
        assert_eq!(pixels((0, 3), (0, 0)), [(0, 3), (0, 2), (0, 1), (0, 0)]);
    }

    #[test]
    fn a_diagonal_and_a_shallow_line_step_one_pixel_at_a_time() {
        assert_eq!(pixels((0, 0), (2, -2)), [(0, 0), (1, -1), (2, -2)]);
        assert_eq!(pixels((0, 0), (4, 2)), [(0, 0), (1, 1), (2, 1), (3, 2), (4, 2)]);
    }

    #[test]
    fn a_line_has_as_many_pixels_as_its_longer_side_plus_one() {
        assert_eq!(pixels((-5, 7), (20, -3)).len(), 26);
    }
}
