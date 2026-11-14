//! Draws points, lines and areas on a page under the write controls.
#![allow(
    clippy::arithmetic_side_effects,
    reason = "spans are clipped to the 800 by 480 page before any arithmetic on them"
)]

use super::Ink;
use super::line::walk;
use crate::convert::{to_u32, to_u64};
use crate::geometry::Point;
use crate::surface::Canvas;
use crate::work::Work;
use crate::write::{Shade, Tile, WriteControls};

/// A page being drawn on.
pub(crate) struct Painter<'a> {
    canvas: &'a mut Canvas,
    ink: Ink,
    controls: WriteControls,
    /// Points drawn since the command began: the place in pattern memory.
    phase: &'a mut u32,
    work: &'a mut Work,
}

impl<'a> Painter<'a> {
    pub(crate) fn new(
        canvas: &'a mut Canvas,
        controls: WriteControls,
        background: u8,
        phase: &'a mut u32,
        work: &'a mut Work,
    ) -> Self {
        Self { canvas, ink: Ink::new(&controls, background), controls, phase, work }
    }

    /// Draws one point of a line or curve: the next pattern bit, or with
    /// shading on, everything from the point to the reference line (manual
    /// chapter 3, "Shading Control"). False once the work allowance is spent.
    pub(crate) fn point(&mut self, at: Point) -> bool {
        if !self.work.spend(1) {
            return false;
        }
        if let Some(shade) = self.controls.shade {
            return self.shade_from(at, shade);
        }
        let on = self.controls.pattern_bit(*self.phase);
        *self.phase = self.phase.wrapping_add(1);
        let ink = self.ink;
        self.canvas.update(at.x, at.y, |old| ink.apply(old, on));
        true
    }

    /// Draws a line, both ends included unless `skip_first`.
    pub(crate) fn line(&mut self, from: Point, to: Point, skip_first: bool) -> bool {
        let mut complete = true;
        let mut first = skip_first;
        walk(from, to, |at| {
            if core::mem::take(&mut first) {
                return true;
            }
            complete = self.point(at);
            complete
        });
        complete
    }

    /// Shades from a point to the reference line, both included.
    fn shade_from(&mut self, at: Point, shade: Shade) -> bool {
        let (fixed, moving, limit, fixed_limit) = if shade.vertical {
            (at.y, at.x, Canvas::width_i32(), Canvas::height_i32())
        } else {
            (at.x, at.y, Canvas::height_i32(), Canvas::width_i32())
        };
        if !(0..fixed_limit).contains(&fixed) {
            return true;
        }
        let low = moving.min(shade.at).max(0);
        let high = moving.max(shade.at).min(limit - 1);
        if low > high {
            return true;
        }
        if !self.work.spend(to_u64(high - low + 1)) {
            return false;
        }
        for position in low..=high {
            let (x, y) = if shade.vertical { (position, fixed) } else { (fixed, position) };
            self.area(x, y, shade.tile);
        }
        true
    }

    /// Writes one pixel of a shaded or filled area. The pattern runs down
    /// the screen, one bit to a row, whichever way the reference line lies
    /// (manual chapter 3, figure 3-21); a shading character is tiled from
    /// the screen's origin.
    fn area(&mut self, x: i32, y: i32, tile: Option<Tile>) {
        let on = match tile {
            Some(tile) => tile_bit(tile, x, y),
            None => self.controls.pattern_bit(to_u32(y)),
        };
        let ink = self.ink;
        self.canvas.update(x, y, |old| ink.apply(old, on));
    }

    /// Writes one pixel of a filled polygon; the caller has paid for it.
    pub(crate) fn fill_pixel(&mut self, x: i32, y: i32) {
        let tile = self.controls.shade.and_then(|shade| shade.tile);
        self.area(x, y, tile);
    }

    /// Writes one pixel of a character cell; the caller has paid for it.
    pub(crate) fn cell_pixel(&mut self, at: Point, on: bool) {
        let ink = self.ink;
        self.canvas.update(at.x, at.y, |old| ink.apply(old, on));
    }

    /// Whether blank parts of a character cell change anything.
    pub(crate) const fn writes_off_pixels(&self) -> bool {
        self.ink.writes_off_pixels()
    }

    /// Pays for `units` of work.
    pub(crate) fn spend(&mut self, units: u64) -> bool {
        self.work.spend(units)
    }
}

/// Whether the shading character has a pixel at a screen position.
fn tile_bit(tile: Tile, x: i32, y: i32) -> bool {
    const SIDE: i32 = 8;
    if tile.width <= 0 || tile.height <= 0 {
        return false;
    }
    let column = x.rem_euclid(tile.width) * SIDE / tile.width;
    let row = y.rem_euclid(tile.height) * SIDE / tile.height;
    let bits = usize::try_from(row).ok().and_then(|row| tile.rows.get(row)).copied().unwrap_or(0);
    (0..SIDE).contains(&column) && (bits >> (SIDE - 1 - column)) & 1 == 1
}
