//! Sines and cosines of whole degrees, in integers.
#![allow(
    clippy::arithmetic_side_effects,
    reason = "offsets are within 131071 and sines within 65536, so products fit i64"
)]

use crate::addressing::div_round;
use crate::convert::narrow;

/// One in the fixed-point form of the table: sixteen fraction bits.
const ONE: i64 = 65_536;
const QUARTER: i32 = 90;
const HALF: i32 = 180;
const CIRCLE: i32 = 360;

/// `sin(d) * 65536` rounded, for d from 0 to 90 degrees. A test checks the
/// table against the identity `sin² + cos² = 1`.
const SINES: [i64; 91] = [
    0, 1144, 2287, 3430, 4572, 5712, 6850, 7987, 9121, 10252, 11380, 12505, 13626, 14742, 15855, 16962, 18064, 19161,
    20252, 21336, 22415, 23486, 24550, 25607, 26656, 27697, 28729, 29753, 30767, 31772, 32768, 33754, 34729, 35693,
    36647, 37590, 38521, 39441, 40348, 41243, 42126, 42995, 43852, 44695, 45525, 46341, 47143, 47930, 48703, 49461,
    50203, 50931, 51643, 52339, 53020, 53684, 54332, 54963, 55578, 56175, 56756, 57319, 57865, 58393, 58903, 59396,
    59870, 60326, 60764, 61183, 61584, 61966, 62328, 62672, 62997, 63303, 63589, 63856, 64104, 64332, 64540, 64729,
    64898, 65048, 65177, 65287, 65376, 65446, 65496, 65526, 65536,
];

/// The sine of a whole number of degrees, times 65536.
pub(crate) fn sine(degrees: i32) -> i64 {
    let degrees = degrees.rem_euclid(CIRCLE);
    let (within, sign) = if degrees < HALF { (degrees, 1) } else { (degrees - HALF, -1) };
    let folded = if within <= QUARTER { within } else { HALF - within };
    let index = usize::try_from(folded).unwrap_or(0);
    sign * SINES.get(index).copied().unwrap_or(0)
}

/// The cosine of a whole number of degrees, times 65536.
pub(crate) fn cosine(degrees: i32) -> i64 {
    sine(degrees + QUARTER)
}

/// The offset (`dx`, `dy`) turned counterclockwise on the screen by a whole
/// number of degrees. The screen's y axis points down, so a counterclockwise
/// turn takes "right" to "up", which is negative y.
pub(crate) fn rotate(dx: i32, dy: i32, degrees: i32) -> (i32, i32) {
    let (sin, cos) = (sine(degrees), cosine(degrees));
    let x = i64::from(dx) * cos + i64::from(dy) * sin;
    let y = -i64::from(dx) * sin + i64::from(dy) * cos;
    (narrow(div_round(x, ONE)), narrow(div_round(y, ONE)))
}

#[cfg(test)]
mod tests {
    use super::{ONE, cosine, rotate, sine};

    #[test]
    fn the_table_satisfies_the_pythagorean_identity() {
        for degrees in -360..=720 {
            let sum = sine(degrees) * sine(degrees) + cosine(degrees) * cosine(degrees);
            let error = (sum - ONE * ONE).abs();
            // Each entry is rounded to one part in 65536.
            assert!(error <= 2 * ONE, "{degrees} degrees: error {error}");
        }
    }

    #[test]
    fn the_cardinal_angles_are_exact() {
        assert_eq!((sine(0), sine(90), sine(180), sine(270)), (0, ONE, 0, -ONE));
        assert_eq!((cosine(0), cosine(90), cosine(180), cosine(270)), (ONE, 0, -ONE, 0));
        assert_eq!(sine(30), ONE / 2);
    }

    #[test]
    fn a_positive_turn_is_counterclockwise_on_a_screen_whose_y_points_down() {
        assert_eq!(rotate(100, 0, 90), (0, -100));
        assert_eq!(rotate(100, 0, -90), (0, 100));
        assert_eq!(rotate(100, 0, 180), (-100, 0));
        assert_eq!(rotate(0, -50, 90), (-50, 0));
    }
}
