//! One graphics page: a rectangle of colour-map addresses.

use alloc::vec;
use alloc::vec::Vec;

use super::{SCREEN_HEIGHT, SCREEN_WIDTH};
use crate::convert::{to_i32, to_usize};

/// The VT340 has four bit planes, so a pixel holds an address from 0 to 15
/// (manual chapter 2, "VT340 Monochrome and Color Maps").
pub(crate) const PLANE_MASK: u8 = 0x0F;

/// Where on a page drawing lands. ReGIS v1 draws on the whole page, where
/// it says. A numbered drawing of ReGIS v2 is drawn moved to its screen
/// position, shows only inside its clip rectangles, and marks the pixels
/// it writes in the pick record (`docs/regis-v2.md`, "Attributes" and
/// "Picking").
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Window {
    /// Added to every position drawn.
    pub(crate) dx: i32,
    pub(crate) dy: i32,
    /// The pixels that may change, in page coordinates, both ends
    /// included; none when `left > right` or `top > bottom`.
    pub(crate) left: i32,
    pub(crate) top: i32,
    pub(crate) right: i32,
    pub(crate) bottom: i32,
    /// The drawing number written to the pick record with each pixel
    /// written; 0 leaves the record alone.
    pub(crate) pick: u16,
    /// Shaded and filled areas are paid for whole, not just the part on
    /// the page, so a numbered drawing's work is the same wherever it is
    /// placed and however it is clipped.
    pub(crate) whole_cost: bool,
}

impl Window {
    /// The whole page, unmoved: ReGIS v1.
    pub(crate) fn page() -> Self {
        Self {
            dx: 0,
            dy: 0,
            left: 0,
            top: 0,
            right: Canvas::width_i32().saturating_sub(1),
            bottom: Canvas::height_i32().saturating_sub(1),
            pick: 0,
            whole_cost: false,
        }
    }

    /// The pixels that may change, in the coordinates drawing uses: left,
    /// top, right, bottom, both ends included.
    pub(crate) const fn local(&self) -> (i32, i32, i32, i32) {
        (
            self.left.saturating_sub(self.dx),
            self.top.saturating_sub(self.dy),
            self.right.saturating_sub(self.dx),
            self.bottom.saturating_sub(self.dy),
        )
    }
}

/// A rectangle of pixels, both ends included.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Bounds {
    pub(crate) left: i32,
    pub(crate) top: i32,
    pub(crate) right: i32,
    pub(crate) bottom: i32,
}

impl Bounds {
    /// The whole page.
    pub(crate) fn page() -> Self {
        Self {
            left: 0,
            top: 0,
            right: Canvas::width_i32().saturating_sub(1),
            bottom: Canvas::height_i32().saturating_sub(1),
        }
    }

    const fn point(x: i32, y: i32) -> Self {
        Self { left: x, top: y, right: x, bottom: y }
    }

    /// The smallest rectangle holding both.
    pub(crate) fn with(self, other: Self) -> Self {
        Self {
            left: self.left.min(other.left),
            top: self.top.min(other.top),
            right: self.right.max(other.right),
            bottom: self.bottom.max(other.bottom),
        }
    }

    /// The smallest rectangle holding both, either of which may be empty.
    pub(crate) fn join(one: Option<Self>, other: Option<Self>) -> Option<Self> {
        match (one, other) {
            (Some(one), Some(other)) => Some(one.with(other)),
            (one, other) => one.or(other),
        }
    }

    /// The pixels in both, if any.
    pub(crate) fn within(self, other: Self) -> Option<Self> {
        let both = Self {
            left: self.left.max(other.left),
            top: self.top.max(other.top),
            right: self.right.min(other.right),
            bottom: self.bottom.min(other.bottom),
        };
        (both.left <= both.right && both.top <= both.bottom).then_some(both)
    }

    /// Moved by an offset.
    pub(crate) const fn moved(self, dx: i32, dy: i32) -> Self {
        Self {
            left: self.left.saturating_add(dx),
            top: self.top.saturating_add(dy),
            right: self.right.saturating_add(dx),
            bottom: self.bottom.saturating_add(dy),
        }
    }
}

/// One 800 by 480 graphics page. Each pixel is a colour-map address, 0 to 15.
/// Two pages are equal when their pixels are.
#[derive(Clone, Debug)]
pub struct Canvas {
    pixels: Vec<u8>,
    window: Window,
    /// The pick record: for each pixel the numbered drawing that picking
    /// reports there, 0 for none. Empty until a detectable drawing is
    /// painted.
    pick: Vec<u16>,
    /// Whether a pixel was written since this was last asked.
    changed: bool,
    /// Every position drawing reaches is being noted in `reach`.
    measuring: bool,
    /// The positions drawing has reached while measuring, in the
    /// coordinates drawing uses, on the page or off it.
    reach: Option<Bounds>,
}

impl PartialEq for Canvas {
    fn eq(&self, other: &Self) -> bool {
        self.pixels == other.pixels
    }
}

impl Eq for Canvas {}

impl Default for Canvas {
    fn default() -> Self {
        Self::new()
    }
}

impl Canvas {
    /// A page with every pixel at colour-map address 0.
    #[must_use]
    pub fn new() -> Self {
        Self {
            pixels: vec![0; Self::area()],
            window: Window::page(),
            pick: Vec::new(),
            changed: false,
            measuring: false,
            reach: None,
        }
    }

    /// A page with no pixel memory: drawing on it does nothing. The second
    /// page of a composed picture, which numbered drawings never draw on.
    pub(crate) fn absent() -> Self {
        Self {
            pixels: Vec::new(),
            window: Window::page(),
            pick: Vec::new(),
            changed: false,
            measuring: false,
            reach: None,
        }
    }

    pub(crate) const fn window(&self) -> Window {
        self.window
    }

    pub(crate) const fn set_window(&mut self, window: Window) {
        self.window = window;
    }

    /// Starts noting every position drawing reaches, whether or not it
    /// lies in the window: how a numbered drawing's extent is measured.
    pub(crate) const fn measure_reach(&mut self) {
        self.measuring = true;
        self.reach = None;
    }

    /// Stops noting; the rectangle of the positions reached, in the
    /// coordinates drawing uses, or `None` if there were none.
    pub(crate) const fn take_reach(&mut self) -> Option<Bounds> {
        self.measuring = false;
        self.reach.take()
    }

    /// Notes a position drawing may write, before the window cuts it.
    /// Shading and filling clip their runs to the window before writing,
    /// so they note the ends of what they would cover themselves.
    pub(crate) fn reach(&mut self, x: i32, y: i32) {
        if self.measuring {
            self.reach = Bounds::join(self.reach, Some(Bounds::point(x, y)));
        }
    }

    /// Puts back another page's pixels and pick record inside a rectangle:
    /// the start of repainting that rectangle. The other page is the base,
    /// which has no pick record, or the background raster, whose record
    /// holds what the drawings of layer 0 wrote (animation.md, ANIM-12).
    pub(crate) fn restore(&mut self, other: &Self, area: Bounds) {
        let Some(area) = area.within(Bounds::page()) else { return };
        if self.pick.is_empty() && !other.pick.is_empty() {
            self.pick = vec![0; Self::area()];
        }
        let width = to_usize(Self::width_i32());
        let columns = to_usize(area.left)..to_usize(area.right).saturating_add(1);
        for row in to_usize(area.top)..=to_usize(area.bottom) {
            let start = row.saturating_mul(width);
            let span = start.saturating_add(columns.start)..start.saturating_add(columns.end);
            if let (Some(to), Some(from)) = (self.pixels.get_mut(span.clone()), other.pixels.get(span.clone())) {
                to.copy_from_slice(from);
            }
            match (self.pick.get_mut(span.clone()), other.pick.get(span)) {
                (Some(to), Some(from)) => to.copy_from_slice(from),
                (Some(to), None) => to.fill(0),
                (None, _) => {}
            }
        }
    }

    /// Forgets a pick record that names no drawing, so that it is empty
    /// exactly when no detectable drawing has a pixel.
    pub(crate) fn tidy_pick(&mut self) {
        if self.pick.iter().all(|drawing| *drawing == 0) {
            self.pick.clear();
        }
    }

    /// Replaces every pixel and the pick record with another page's.
    pub(crate) fn copy_from(&mut self, other: &Self) {
        self.pixels.clear();
        self.pixels.extend_from_slice(&other.pixels);
        self.pick.clear();
        self.pick.extend_from_slice(&other.pick);
    }

    /// The pick record, row by row; empty when no pixel has a drawing.
    pub(crate) fn pick(&self) -> &[u16] {
        &self.pick
    }

    /// Whether drawing wrote a pixel since this was last asked.
    pub(crate) const fn take_changed(&mut self) -> bool {
        core::mem::replace(&mut self.changed, false)
    }

    fn area() -> usize {
        let side = |value: u32| usize::try_from(value).unwrap_or(0);
        side(SCREEN_WIDTH).saturating_mul(side(SCREEN_HEIGHT))
    }

    /// The width in pixels, always [`SCREEN_WIDTH`].
    #[must_use]
    pub const fn width(&self) -> u32 {
        SCREEN_WIDTH
    }

    /// The height in pixels, always [`SCREEN_HEIGHT`].
    #[must_use]
    pub const fn height(&self) -> u32 {
        SCREEN_HEIGHT
    }

    /// Every pixel, row by row from the top-left, one byte each.
    #[must_use]
    pub fn pixels(&self) -> &[u8] {
        &self.pixels
    }

    /// The pixel at a position, or `None` outside the page.
    #[must_use]
    pub fn pixel(&self, x: u32, y: u32) -> Option<u8> {
        let x = i32::try_from(x).ok()?;
        let y = i32::try_from(y).ok()?;
        self.get(x, y)
    }

    pub(crate) fn width_i32() -> i32 {
        i32::try_from(SCREEN_WIDTH).unwrap_or(i32::MAX)
    }

    pub(crate) fn height_i32() -> i32 {
        i32::try_from(SCREEN_HEIGHT).unwrap_or(i32::MAX)
    }

    fn index(x: i32, y: i32) -> Option<usize> {
        let inside = (0..Self::width_i32()).contains(&x) && (0..Self::height_i32()).contains(&y);
        inside.then(|| to_usize(y).saturating_mul(to_usize(Self::width_i32())).saturating_add(to_usize(x)))
    }

    pub(crate) fn get(&self, x: i32, y: i32) -> Option<u8> {
        self.pixels.get(Self::index(x, y)?).copied()
    }

    /// Changes one pixel through `change`; positions outside the window do
    /// nothing.
    pub(crate) fn update(&mut self, x: i32, y: i32, change: impl FnOnce(u8) -> Option<u8>) {
        self.reach(x, y);
        let window = self.window;
        let (x, y) = (x.saturating_add(window.dx), y.saturating_add(window.dy));
        if x < window.left || x > window.right || y < window.top || y > window.bottom {
            return;
        }
        let Some(index) = Self::index(x, y) else { return };
        if let Some(pixel) = self.pixels.get_mut(index)
            && let Some(value) = change(*pixel)
        {
            *pixel = value & PLANE_MASK;
            self.changed = true;
            if window.pick != 0 {
                // The record is made when the first detectable drawing
                // writes a pixel (`docs/regis-v2.md`, "Picking").
                if self.pick.is_empty() {
                    self.pick = vec![0; Self::area()];
                }
                if let Some(target) = self.pick.get_mut(index) {
                    *target = window.pick;
                }
            }
        }
    }

    /// Changes the pixels of one row from `from` to `to`, both included,
    /// through `change`, as [`update`](Self::update) would one by one: the
    /// window is applied to the run once, not to each pixel, which is what
    /// makes a filled area cost its pixels and no more.
    pub(crate) fn update_run(&mut self, from: i32, to: i32, y: i32, change: impl Fn(u8) -> Option<u8>) {
        if from > to {
            return;
        }
        self.reach(from, y);
        self.reach(to, y);
        let window = self.window;
        let y = y.saturating_add(window.dy);
        if y < window.top.max(0) || y > window.bottom.min(Self::height_i32().saturating_sub(1)) {
            return;
        }
        let left = from.saturating_add(window.dx).max(window.left).max(0);
        let right = to.saturating_add(window.dx).min(window.right).min(Self::width_i32().saturating_sub(1));
        let (Some(start), Some(end)) = (Self::index(left, y), Self::index(right, y)) else { return };
        if start > end {
            return;
        }
        let Some(pixels) = self.pixels.get_mut(start..=end) else { return };
        let mut written = false;
        for pixel in pixels {
            if let Some(value) = change(*pixel) {
                *pixel = value & PLANE_MASK;
                written = true;
            }
        }
        if !written {
            return;
        }
        self.changed = true;
        if window.pick != 0 {
            // The record is made when the first detectable drawing writes
            // a pixel (`docs/regis-v2.md`, "Picking").
            if self.pick.is_empty() {
                self.pick = vec![0; Self::area()];
            }
            if let Some(targets) = self.pick.get_mut(start..=end) {
                targets.fill(window.pick);
            }
        }
    }

    /// Sets every pixel to one colour-map address.
    pub(crate) fn clear(&mut self, value: u8) {
        self.pixels.fill(value & PLANE_MASK);
        self.changed = true;
    }

    /// Moves the picture so the pixel at (`dx`, `dy`) lands on the top-left
    /// corner; what leaves the page is lost and what enters is `fill`
    /// (manual chapter 2, "Scroll Argument").
    pub(crate) fn scroll(&mut self, dx: i32, dy: i32, fill: u8) {
        let old = core::mem::replace(&mut self.pixels, vec![fill & PLANE_MASK; Self::area()]);
        self.changed = true;
        let width = to_usize(Self::width_i32());
        for (index, pixel) in self.pixels.iter_mut().enumerate() {
            let x = to_i32(index.checked_rem(width).unwrap_or(0)).saturating_add(dx);
            let y = to_i32(index.checked_div(width).unwrap_or(0)).saturating_add(dy);
            if let Some(value) = Self::index(x, y).and_then(|from| old.get(from)) {
                *pixel = *value;
            }
        }
    }
}
