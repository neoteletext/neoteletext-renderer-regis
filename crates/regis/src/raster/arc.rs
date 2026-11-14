//! Circles and arcs as points one degree apart.
//!
//! "The VT300 draws arcs in 1 degree increments" (manual chapter 6): an arc
//! is the chain of chords between the points found by turning the starting
//! point about the centre one degree at a time.
#![allow(
    clippy::arithmetic_side_effects,
    reason = "the centre and start are within 65535 of the origin, so an offset is within 2^17 and a point within 2^18"
)]

use super::trig::rotate;
use crate::geometry::Point;

/// A whole circle.
pub(crate) const CIRCLE_DEGREES: i32 = 360;

/// Calls `visit` with each point of an arc about `centre` from `start`
/// through `degrees` (positive is counterclockwise), the start first.
/// Points that fall on the same pixel as the one before are passed over.
pub(crate) fn arc_points(centre: Point, start: Point, degrees: i32, mut visit: impl FnMut(Point)) {
    let degrees = degrees.clamp(-CIRCLE_DEGREES, CIRCLE_DEGREES);
    let (dx, dy) = (start.x - centre.x, start.y - centre.y);
    let turn = if degrees < 0 { -1 } else { 1 };
    let mut last = start;
    visit(start);
    for step in 1..=degrees.abs() {
        // A whole circle ends exactly where it began.
        let (x, y) = if step == CIRCLE_DEGREES { (dx, dy) } else { rotate(dx, dy, turn * step) };
        let point = Point::new(centre.x + x, centre.y + y);
        if point != last {
            visit(point);
            last = point;
        }
    }
}

#[cfg(test)]
mod tests {
    use alloc::vec::Vec;

    use super::arc_points;
    use crate::geometry::Point;

    fn points(centre: (i32, i32), start: (i32, i32), degrees: i32) -> Vec<Point> {
        let mut seen = Vec::new();
        arc_points(Point::new(centre.0, centre.1), Point::new(start.0, start.1), degrees, |point| seen.push(point));
        seen
    }

    #[test]
    fn a_circle_closes_on_its_starting_point() {
        let circle = points((400, 240), (500, 240), 360);
        assert_eq!(circle.first(), circle.last());
        assert_eq!(circle.len(), 361);
        assert!(circle.contains(&Point::new(400, 140)));
        assert!(circle.contains(&Point::new(300, 240)));
    }

    #[test]
    fn a_positive_arc_runs_counterclockwise_and_a_negative_one_clockwise() {
        assert_eq!(points((0, 0), (100, 0), 90).last(), Some(&Point::new(0, -100)));
        assert_eq!(points((0, 0), (100, 0), -90).last(), Some(&Point::new(0, 100)));
    }

    #[test]
    fn a_tiny_circle_yields_each_pixel_once_in_a_row() {
        let circle = points((10, 10), (11, 10), 360);
        assert!(circle.len() < 12);
        assert!(circle.windows(2).all(|pair| pair[0] != pair[1]));
    }

    #[test]
    fn a_radius_of_nothing_is_one_point() {
        assert_eq!(points((5, 5), (5, 5), 360), [Point::new(5, 5)]);
    }
}
