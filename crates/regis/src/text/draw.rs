//! Drawing one character cell.
#![allow(
    clippy::arithmetic_side_effects,
    reason = "cell sides are at most 2560 and the origin within 65535, so pixel positions fit i32"
)]

use super::TextControls;
use crate::convert::to_u64;
use crate::font::Glyph;
use crate::geometry::{Point, direction};
use crate::raster::Painter;

/// The slant is in pixels sideways per this many pixels of height.
const SLANT_RISE: i32 = 5;

/// Draws a character with the top-left of its cell at `origin`, the pivot
/// for any tilt (manual chapter 7, "Orientation"). False if the work
/// allowance ran out; nothing is drawn then.
///
/// The display cell is the area written; the shape fills the unit cell at
/// its top-left, and the rest of the display cell is blank. At a diagonal
/// tilt each step along the character is a diagonal pixel step, which
/// stretches the character as the manual describes.
pub(crate) fn draw_character(painter: &mut Painter<'_>, glyph: &Glyph, origin: Point, controls: &TextControls) -> bool {
    let (display_width, display_height) = controls.display;
    let (unit_width, unit_height) = controls.unit;
    let blanks = painter.writes_off_pixels();
    let (columns, rows) = if blanks {
        (display_width, display_height)
    } else {
        (display_width.min(unit_width), display_height.min(unit_height))
    };
    if !painter.spend(to_u64(columns) * to_u64(rows)) {
        return false;
    }
    let along = direction(controls.character_direction);
    let down = direction(controls.character_direction + 6);
    for v in 0..rows {
        let lean = (unit_height - 1 - v).max(0) * controls.slant / SLANT_RISE;
        for u in 0..columns {
            let on = glyph.lit(u, v, unit_width, unit_height);
            if on || blanks {
                let across = u - lean;
                let at = Point::new(origin.x + across * along.0 + v * down.0, origin.y + across * along.1 + v * down.1);
                painter.cell_pixel(at, on);
            }
        }
    }
    true
}
