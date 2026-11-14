//! The polygon fill command, `F` (manual chapter 11).

use alloc::vec::Vec;

use super::Machine;
use crate::geometry::Point;
use crate::limits::MAX_FILL_VERTICES;
use crate::raster::fill_polygon;
use crate::surface::Screen;

/// A polygon being collected.
#[derive(Clone, Debug)]
pub(crate) struct Fill {
    /// The active position when the fill began; it is restored at the end
    /// "whether or not any drawing takes place".
    start: Point,
    /// The corners, in pixels.
    vertices: Vec<Point>,
}

impl Fill {
    /// Adds a corner. Consecutive corners on the same pixel "count as one
    /// vertex"; corners beyond the limit are ignored.
    pub(crate) fn vertex(&mut self, point: Point) {
        if self.vertices.last() != Some(&point) && self.vertices.len() < MAX_FILL_VERTICES {
            self.vertices.push(point);
        }
    }
}

impl Machine {
    pub(super) fn begin_fill(&mut self) {
        self.fill = Some(Fill { start: self.position, vertices: Vec::new() });
        self.pen.temporary = None;
    }

    /// Fills the figure collected and puts the cursor back.
    pub(super) fn end_fill(&mut self, screen: &mut Screen) {
        let Some(fill) = self.fill.take() else { return };
        self.paint(screen, |painter| fill_polygon(painter, &fill.vertices));
        self.position = fill.start;
        self.pen.temporary = None;
        self.curve = None;
        self.pen.chain = None;
    }
}
