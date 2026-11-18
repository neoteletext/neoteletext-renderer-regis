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
use crate::surface::Window;

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
    fn covering(vertices: &[Point], window: Window) -> Option<Self> {
        let (first_column, first_row, last_column, last_row) = window.local();
        let left = vertices.iter().map(|v| v.x).min()?.max(first_column);
        let right = vertices.iter().map(|v| v.x).max()?.min(last_column);
        let top = vertices.iter().map(|v| v.y).min()?.max(first_row);
        let bottom = vertices.iter().map(|v| v.y).max()?.min(last_row);
        if left > right || top > bottom {
            return None;
        }
        let (width, height) = (right - left + 1, bottom - top + 1);
        Some(Self { left, top, width, height, set: Vec::new() })
    }

    /// The work the whole polygon costs wherever it lies: its bounding
    /// box, as [`fill_polygon`] charges for the part on the page.
    fn whole_cost(vertices: &[Point]) -> Option<u64> {
        let width = to_u64(vertices.iter().map(|v| v.x).max()? - vertices.iter().map(|v| v.x).min()? + 1);
        let height = to_u64(vertices.iter().map(|v| v.y).max()? - vertices.iter().map(|v| v.y).min()? + 1);
        Some(fill_cost(width, height, vertices.len()))
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
        let (from, to) = (from.max(self.left), to.min(self.left + self.width - 1));
        if from > to || !(0..self.height).contains(&(y - self.top)) {
            return;
        }
        let start = to_usize(y - self.top) * to_usize(self.width);
        let span = start + to_usize(from - self.left)..=start + to_usize(to - self.left);
        if let Some(cells) = self.set.get_mut(span) {
            cells.fill(true);
        }
    }

    /// The runs of marked pixels of each row, as (first, last, row) in
    /// the coordinates drawing uses.
    fn runs(&self) -> impl Iterator<Item = (i32, i32, i32)> + '_ {
        let width = to_usize(self.width).max(1);
        self.set.chunks(width).zip(self.top..).flat_map(move |(row, y)| {
            let mut at = 0;
            core::iter::from_fn(move || {
                let first = at + row.get(at..)?.iter().position(|set| *set)?;
                let length = row.get(first..)?.iter().position(|set| !*set).unwrap_or(row.len() - first);
                at = first + length;
                let left = self.left + i32::try_from(first).unwrap_or(0);
                Some((left, left + i32::try_from(length).unwrap_or(0) - 1, y))
            })
        })
    }
}

/// The work of filling a box of pixels with a polygon of `vertices`
/// corners: each pixel is marked and written, and each row looks at every
/// edge and sorts its crossings.
fn fill_cost(width: u64, height: u64, vertices: usize) -> u64 {
    let edge_work =
        height.saturating_mul(u64::try_from(vertices).unwrap_or(u64::MAX)).saturating_mul(ROW_COST_PER_VERTEX);
    width.saturating_mul(height).saturating_mul(2).saturating_add(edge_work)
}

/// Pays for one edge: an edge can be far longer than the page.
fn pay_edge(painter: &mut Painter<'_>, a: Point, b: Point) -> bool {
    let length = (b.x - a.x).abs().max((b.y - a.y).abs());
    painter.spend(to_u64(length))
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
    let window = painter.window();
    // The filled pixels and the outline all lie within the corners' box.
    for vertex in vertices {
        painter.reach(*vertex);
    }
    // A numbered drawing pays for the whole polygon, seen or not, so its
    // work does not depend on where it is placed.
    if window.whole_cost && !Mask::whole_cost(vertices).is_some_and(|cost| painter.spend(cost)) {
        return false;
    }
    let Some(mut mask) = Mask::covering(vertices, window) else {
        // Off the page: only the edges' lengths are paid for, as below.
        return !window.whole_cost
            || vertices.iter().zip(vertices.iter().cycle().skip(1)).all(|(a, b)| pay_edge(painter, *a, *b));
    };
    if !window.whole_cost && !painter.spend(fill_cost(to_u64(mask.width), to_u64(mask.height), vertices.len())) {
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

    // Each pixel is written once, a row's run of them at a time.
    for (from, to, y) in mask.runs() {
        painter.fill_run(from, to, y);
    }
    true
}

/// Marks the pixels of one edge; an edge can be far longer than the page,
/// so its length is paid for.
fn outline(painter: &mut Painter<'_>, mask: &mut Mask, a: Point, b: Point) -> bool {
    if !pay_edge(painter, a, b) {
        return false;
    }
    walk(a, b, |at| {
        mask.mark(at.x, at.y);
        true
    });
    true
}
