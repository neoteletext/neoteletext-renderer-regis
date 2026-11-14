//! Interpolated curves.
//!
//! The manual does not give the terminal's equation, only its properties
//! (chapter 6, "How ReGIS Interpolates Curves"): the curve passes through
//! every point, is the same drawn from either end, and its slope at a point
//! is parallel to the line through the points either side. Those are the
//! properties of the Catmull-Rom spline, which is used here, four points at
//! a time as the manual describes.
#![allow(
    clippy::arithmetic_side_effects,
    reason = "pixel coordinates are within 65535 and the parameter within 64, so the cubic's terms fit i64"
)]

use crate::addressing::div_round;
use crate::convert::narrow;
use crate::geometry::Point;
use crate::limits::MAX_CURVE_PIECES;

/// Pixels of chord length to each straight piece.
const PIXELS_PER_PIECE: i32 = 6;
const MIN_PIECES: i32 = 4;

/// One coordinate of the spline through `b` and `c` (with neighbours `a`
/// and `d`) at parameter `step / pieces`.
fn coordinate(a: i32, b: i32, c: i32, d: i32, step: i64, pieces: i64) -> i32 {
    let (a, b, c, d) = (i64::from(a), i64::from(b), i64::from(c), i64::from(d));
    // 2·q(t) = 2b + (c − a)t + (2a − 5b + 4c − d)t² + (3b − a − 3c + d)t³
    let linear = (c - a) * step * pieces * pieces;
    let square = (2 * a - 5 * b + 4 * c - d) * step * step * pieces;
    let cube = (3 * b - a - 3 * c + d) * step * step * step;
    let doubled = 2 * b * pieces * pieces * pieces + linear + square + cube;
    narrow(div_round(doubled, 2 * pieces * pieces * pieces))
}

/// Calls `visit` with the points of the curve from `window[1]` (not passed)
/// to `window[2]` (passed last), given the four points around that span.
pub(crate) fn span_points(window: [Point; 4], mut visit: impl FnMut(Point)) {
    let [a, b, c, d] = window;
    let reach = (c.x - b.x).abs().max((c.y - b.y).abs());
    let pieces = i64::from((reach / PIXELS_PER_PIECE).clamp(MIN_PIECES, MAX_CURVE_PIECES));
    for step in 1..pieces {
        visit(Point::new(coordinate(a.x, b.x, c.x, d.x, step, pieces), coordinate(a.y, b.y, c.y, d.y, step, pieces)));
    }
    visit(c);
}

#[cfg(test)]
mod tests {
    use alloc::vec::Vec;

    use super::span_points;
    use crate::geometry::Point;

    fn span(points: [(i32, i32); 4]) -> Vec<Point> {
        let mut seen = Vec::new();
        span_points(points.map(|(x, y)| Point::new(x, y)), |point| seen.push(point));
        seen
    }

    #[test]
    fn the_span_ends_on_its_third_point() {
        let points = span([(0, 0), (100, 0), (200, 100), (300, 100)]);
        assert_eq!(points.last(), Some(&Point::new(200, 100)));
    }

    #[test]
    fn collinear_evenly_spaced_points_give_a_straight_line() {
        let points = span([(0, 0), (100, 50), (200, 100), (300, 150)]);
        assert!(points.iter().all(|point| (point.x - 2 * point.y).abs() <= 1), "{points:?}");
    }

    #[test]
    fn the_curve_is_the_same_from_either_end() {
        let forward = span([(0, 0), (60, 80), (160, 90), (240, 10)]);
        let mut backward = span([(240, 10), (160, 90), (60, 80), (0, 0)]);
        backward.pop();
        backward.reverse();
        let inner = forward.get(..forward.len() - 1).unwrap();
        assert_eq!(inner, &backward[..]);
    }
}
