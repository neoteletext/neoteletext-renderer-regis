//! Filled polygons.
//!
//! The interior is found by the even-odd rule along each pixel row, and the
//! outline is added so that the figure covers the same pixels as its
//! vectors would. Every pixel is then written exactly once, which keeps
//! complement writing correct.
#![allow(
    clippy::arithmetic_side_effects,
    reason = "vertices are within 2^18 of the origin (see raster/arc.rs) and the mask is clipped to the 800 by 480 page"
)]

use alloc::vec;
use alloc::vec::Vec;

use super::Painter;
use super::line::walk;
use crate::addressing::div_round;
use crate::convert::{narrow, to_u64, to_usize};
use crate::geometry::Point;
use crate::surface::Canvas;

/// A polygon needs "at least three different vertices" (manual chapter 11).
const MIN_VERTICES: usize = 3;
/// Work units one vertex costs on each pixel row: the edge test and its
/// share of sorting the crossings.
const ROW_COST_PER_VERTEX: u64 = 4;

/// The part of the page a polygon's bounding box covers.
struct Mask {
    left: i32,
    top: i32,
    width: i32,
    height: i32,
    set: Vec<bool>,
}

impl Mask {
    /// The mask for a polygon, or `None` if it lies off the page. Nothing
    /// is allocated: the caller pays for the area first.
    fn covering(vertices: &[Point]) -> Option<Self> {
        let left = vertices.iter().map(|v| v.x).min()?.max(0);
        let right = vertices.iter().map(|v| v.x).max()?.min(Canvas::width_i32() - 1);
        let top = vertices.iter().map(|v| v.y).min()?.max(0);
        let bottom = vertices.iter().map(|v| v.y).max()?.min(Canvas::height_i32() - 1);
        if left > right || top > bottom {
            return None;
        }
        let (width, height) = (right - left + 1, bottom - top + 1);
        Some(Self { left, top, width, height, set: Vec::new() })
    }

    fn mark(&mut self, x: i32, y: i32) {
        let (column, row) = (x - self.left, y - self.top);
        if (0..self.width).contains(&column)
            && (0..self.height).contains(&row)
            && let Some(cell) = self.set.get_mut(to_usize(row) * to_usize(self.width) + to_usize(column))
        {
            *cell = true;
        }
    }

    fn mark_span(&mut self, from: i32, to: i32, y: i32) {
        for x in from.max(self.left)..=to.min(self.left + self.width - 1) {
            self.mark(x, y);
        }
    }

    fn area(&self) -> u64 {
        to_u64(self.width) * to_u64(self.height)
    }
}

/// Where the polygon's edges cross pixel row `y`, sorted. An edge covers
/// the rows from its upper end up to but not including its lower end, so a
/// vertex shared by two edges is counted once.
fn crossings(vertices: &[Point], y: i32, out: &mut Vec<i32>) {
    out.clear();
    let ends = vertices.iter().zip(vertices.iter().cycle().skip(1));
    for (a, b) in ends {
        let (upper, lower) = if a.y <= b.y { (a, b) } else { (b, a) };
        if y >= upper.y && y < lower.y {
            let run = i64::from(y - upper.y) * i64::from(lower.x - upper.x);
            out.push(upper.x + narrow(div_round(run, i64::from(lower.y - upper.y))));
        }
    }
    out.sort_unstable();
}

/// Fills the polygon whose corners are `vertices`, in pixels. False if the
/// work allowance ran out; nothing is drawn then.
pub(crate) fn fill_polygon(painter: &mut Painter<'_>, vertices: &[Point]) -> bool {
    if vertices.len() < MIN_VERTICES {
        return true;
    }
    let Some(mut mask) = Mask::covering(vertices) else { return true };
    // Each row looks at every edge and sorts its crossings.
    let edge_work = to_u64(mask.height) * u64::try_from(vertices.len()).unwrap_or(u64::MAX) * ROW_COST_PER_VERTEX;
    if !painter.spend(mask.area().saturating_mul(2).saturating_add(edge_work)) {
        return false;
    }
    mask.set = vec![false; to_usize(mask.width) * to_usize(mask.height)];

    let mut row = Vec::with_capacity(vertices.len());
    for y in mask.top..mask.top + mask.height {
        crossings(vertices, y, &mut row);
        for [from, to] in row.as_chunks::<2>().0 {
            mask.mark_span(*from, *to, y);
        }
    }
    for (a, b) in vertices.iter().zip(vertices.iter().cycle().skip(1)) {
        if !outline(painter, &mut mask, *a, *b) {
            return false;
        }
    }

    for (index, _) in mask.set.iter().enumerate().filter(|(_, set)| **set) {
        let width = to_usize(mask.width);
        let x = mask.left + i32::try_from(index % width).unwrap_or(0);
        let y = mask.top + i32::try_from(index / width).unwrap_or(0);
        painter.fill_pixel(x, y);
    }
    true
}

/// Marks the pixels of one edge; an edge can be far longer than the page,
/// so its length is paid for.
fn outline(painter: &mut Painter<'_>, mask: &mut Mask, a: Point, b: Point) -> bool {
    let length = (b.x - a.x).abs().max((b.y - a.y).abs());
    if !painter.spend(to_u64(length)) {
        return false;
    }
    walk(a, b, |at| {
        mask.mark(at.x, at.y);
        true
    });
    true
}
