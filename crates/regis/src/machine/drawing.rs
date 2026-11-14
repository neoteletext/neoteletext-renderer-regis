//! The drawing primitives every command shares.

use super::Machine;
use crate::geometry::Point;
use crate::lexer::Extent;
use crate::limits::{MAX_COORDINATE, MIN_COORDINATE};
use crate::raster::Painter;
use crate::surface::Screen;

impl Machine {
    /// The position an extent names: parts left out keep the current value,
    /// signed parts are measured from it.
    pub(super) fn resolve(&self, extent: Extent) -> Point {
        let clamp = |value: i32| value.clamp(MIN_COORDINATE, MAX_COORDINATE);
        Point {
            x: clamp(extent.x.map_or(self.position.x, |axis| axis.resolve(self.position.x))),
            y: clamp(extent.y.map_or(self.position.y, |axis| axis.resolve(self.position.y))),
        }
    }

    /// The position a pixel-vector digit leads to: one step of the current
    /// multiplier in the digit's direction.
    pub(super) fn pixel_vector(&self, digit: u8) -> Point {
        let step = self.addressing.pv_step(i32::from(digit));
        let moved = self.position.stepped(step, self.controls().pv_multiplier);
        Point::new(moved.x.clamp(MIN_COORDINATE, MAX_COORDINATE), moved.y.clamp(MIN_COORDINATE, MAX_COORDINATE))
    }

    /// The active position in pixels.
    pub(super) fn pixel(&self) -> Point {
        self.addressing.to_pixel(self.position)
    }

    /// Runs `draw` with a painter for the page being drawn on.
    pub(super) fn paint<R>(&mut self, screen: &mut Screen, draw: impl FnOnce(&mut Painter<'_>) -> R) -> Option<R> {
        let controls = self.controls();
        let canvas = screen.page_mut(self.page)?;
        let mut painter = Painter::new(canvas, controls, self.pen.background, &mut self.pen.phase, &mut self.work);
        Some(draw(&mut painter))
    }

    /// Draws from one pixel to another, or inside a fill adds the far end
    /// as a vertex.
    pub(super) fn segment(&mut self, screen: &mut Screen, from: Point, to: Point) {
        if let Some(fill) = &mut self.fill {
            fill.vertex(to);
            return;
        }
        let continues = self.pen.chain == Some(from);
        self.paint(screen, |painter| painter.line(from, to, continues));
        self.pen.chain = Some(to);
    }

    /// Draws a line from the active position to `target` and moves there.
    pub(super) fn line_to(&mut self, screen: &mut Screen, target: Point) {
        let from = self.pixel();
        self.segment(screen, from, self.addressing.to_pixel(target));
        self.position = target;
    }

    /// Draws the single pixel at the active position (`V[]`), or inside a
    /// fill makes it a vertex.
    pub(super) fn dot(&mut self, screen: &mut Screen) {
        let at = self.pixel();
        if let Some(fill) = &mut self.fill {
            fill.vertex(at);
            return;
        }
        self.paint(screen, |painter| painter.point(at));
        self.pen.chain = Some(at);
    }
}
