//! Display addressing: the caller's coordinates onto the screen's pixels
//! (manual chapter 2, "Display Addressing").
#![allow(
    clippy::arithmetic_side_effects,
    reason = "coordinates are 16-bit and the screen is 800 by 480, so every product fits i64 with room to spare"
)]

use crate::convert::narrow;
use crate::geometry::{Point, direction};
use crate::limits::{MAX_COORDINATE, MAX_PIXEL_COORDINATE, MIN_COORDINATE};
use crate::surface::Canvas;

/// The mapping from user coordinates to pixels.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Addressing {
    left: i32,
    top: i32,
    /// +1 if x grows to the right, -1 if to the left; likewise y downward.
    sign_x: i32,
    sign_y: i32,
    /// Pixels per user unit, as a fraction; the same for both axes so that
    /// "squares are always square".
    scale_numerator: i64,
    scale_denominator: i64,
}

impl Default for Addressing {
    fn default() -> Self {
        Self { left: 0, top: 0, sign_x: 1, sign_y: 1, scale_numerator: 1, scale_denominator: 1 }
    }
}

impl Addressing {
    /// Addressing with the given corners, or `None` if they span nothing.
    pub(crate) fn new(top_left: Point, bottom_right: Point) -> Option<Self> {
        let span_x = i64::from(bottom_right.x) - i64::from(top_left.x);
        let span_y = i64::from(bottom_right.y) - i64::from(top_left.y);
        if span_x == 0 || span_y == 0 {
            return None;
        }
        let last_column = i64::from(Canvas::width_i32()) - 1;
        let last_row = i64::from(Canvas::height_i32()) - 1;
        // The smaller of the two scales keeps the whole area on the screen.
        let (scale_numerator, scale_denominator) = if last_column * span_y.abs() <= last_row * span_x.abs() {
            (last_column, span_x.abs())
        } else {
            (last_row, span_y.abs())
        };
        Some(Self {
            left: top_left.x,
            top: top_left.y,
            sign_x: if span_x < 0 { -1 } else { 1 },
            sign_y: if span_y < 0 { -1 } else { 1 },
            scale_numerator,
            scale_denominator,
        })
    }

    fn scaled(self, distance: i64) -> i32 {
        let pixel = div_round(distance * self.scale_numerator, self.scale_denominator);
        narrow(pixel).clamp(-MAX_PIXEL_COORDINATE, MAX_PIXEL_COORDINATE)
    }

    fn unscaled(self, pixels: i32) -> i32 {
        let distance = div_round(i64::from(pixels) * self.scale_denominator, self.scale_numerator);
        narrow(distance).clamp(MIN_COORDINATE, MAX_COORDINATE)
    }

    /// The pixel a user position falls on.
    pub(crate) fn to_pixel(self, user: Point) -> Point {
        Point {
            x: self.scaled((i64::from(user.x) - i64::from(self.left)) * i64::from(self.sign_x)),
            y: self.scaled((i64::from(user.y) - i64::from(self.top)) * i64::from(self.sign_y)),
        }
    }

    /// The user position of a pixel.
    pub(crate) fn to_user(self, pixel: Point) -> Point {
        Point {
            x: clamp_user(self.unscaled(pixel.x) * self.sign_x + self.left),
            y: clamp_user(self.unscaled(pixel.y) * self.sign_y + self.top),
        }
    }

    /// A length in user units as a length in pixels.
    pub(crate) fn length(self, user: i32) -> i32 {
        self.scaled(i64::from(user))
    }

    /// A displacement in user units as one in pixels, turned with the axes.
    pub(crate) fn offset(self, dx: i32, dy: i32) -> (i32, i32) {
        (self.scaled(i64::from(dx) * i64::from(self.sign_x)), self.scaled(i64::from(dy) * i64::from(self.sign_y)))
    }

    /// The user coordinates of the screen's top-left corner.
    pub(crate) const fn origin(self) -> Point {
        Point::new(self.left, self.top)
    }

    /// The user-coordinate step of a pixel-vector direction: directions are
    /// fixed on the screen whatever the addressing ("0 is always to the
    /// right").
    pub(crate) fn pv_step(self, number: i32) -> (i32, i32) {
        let (x, y) = direction(number);
        (x * self.sign_x, y * self.sign_y)
    }
}

fn clamp_user(value: i32) -> i32 {
    value.clamp(MIN_COORDINATE, MAX_COORDINATE)
}

/// `numerator / denominator` rounded to nearest; the denominator is positive.
pub(crate) fn div_round(numerator: i64, denominator: i64) -> i64 {
    let half = denominator / 2;
    if numerator >= 0 { (numerator + half) / denominator } else { -((-numerator + half) / denominator) }
}

#[cfg(test)]
mod tests {
    use super::Addressing;
    use crate::geometry::Point;

    #[test]
    fn default_addressing_is_one_pixel_to_the_unit() {
        let addressing = Addressing::default();
        assert_eq!(addressing.to_pixel(Point::new(799, 479)), Point::new(799, 479));
        assert_eq!(addressing.to_user(Point::new(10, 20)), Point::new(10, 20));
    }

    #[test]
    fn a_vt125_screen_maps_into_the_leftmost_768_columns() {
        // Manual chapter 2: "maps the default screen addressing of the VT125
        // into the leftmost 768 pixel columns".
        let addressing = Addressing::new(Point::new(0, 0), Point::new(767, 479)).unwrap();
        assert_eq!(addressing.to_pixel(Point::new(767, 479)), Point::new(767, 479));
    }

    #[test]
    fn a_larger_address_space_scales_down_evenly() {
        let addressing = Addressing::new(Point::new(0, 0), Point::new(1598, 958)).unwrap();
        assert_eq!(addressing.to_pixel(Point::new(1598, 958)), Point::new(799, 479));
        assert_eq!(addressing.to_pixel(Point::new(800, 400)), Point::new(400, 200));
        assert_eq!(addressing.length(100), 50);
    }

    #[test]
    fn reversed_corners_turn_the_axes_but_not_the_pixel_vectors() {
        let addressing = Addressing::new(Point::new(0, 479), Point::new(799, 0)).unwrap();
        assert_eq!(addressing.to_pixel(Point::new(0, 479)), Point::new(0, 0));
        assert_eq!(addressing.to_pixel(Point::new(0, 0)), Point::new(0, 479));
        // Direction 2 is up the screen, which here is +y in user terms.
        assert_eq!(addressing.pv_step(2), (0, 1));
        assert_eq!(addressing.to_user(Point::new(0, 479)), Point::new(0, 0));
    }

    #[test]
    fn corners_spanning_nothing_are_refused() {
        assert!(Addressing::new(Point::new(5, 0), Point::new(5, 100)).is_none());
    }
}
