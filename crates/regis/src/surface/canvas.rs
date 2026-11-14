//! One graphics page: a rectangle of colour-map addresses.

use alloc::vec;
use alloc::vec::Vec;

use super::{SCREEN_HEIGHT, SCREEN_WIDTH};
use crate::convert::{to_i32, to_usize};

/// The VT340 has four bit planes, so a pixel holds an address from 0 to 15
/// (manual chapter 2, "VT340 Monochrome and Color Maps").
pub(crate) const PLANE_MASK: u8 = 0x0F;

/// One 800 by 480 graphics page. Each pixel is a colour-map address, 0 to 15.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Canvas {
    pixels: Vec<u8>,
}

impl Default for Canvas {
    fn default() -> Self {
        Self::new()
    }
}

impl Canvas {
    /// A page with every pixel at colour-map address 0.
    #[must_use]
    pub fn new() -> Self {
        Self { pixels: vec![0; Self::area()] }
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

    /// Changes one pixel through `change`; positions off the page do nothing.
    pub(crate) fn update(&mut self, x: i32, y: i32, change: impl FnOnce(u8) -> Option<u8>) {
        if let Some(pixel) = Self::index(x, y).and_then(|index| self.pixels.get_mut(index))
            && let Some(value) = change(*pixel)
        {
            *pixel = value & PLANE_MASK;
        }
    }

    /// Sets every pixel to one colour-map address.
    pub(crate) fn clear(&mut self, value: u8) {
        self.pixels.fill(value & PLANE_MASK);
    }

    /// Moves the picture so the pixel at (`dx`, `dy`) lands on the top-left
    /// corner; what leaves the page is lost and what enters is `fill`
    /// (manual chapter 2, "Scroll Argument").
    pub(crate) fn scroll(&mut self, dx: i32, dy: i32, fill: u8) {
        let old = core::mem::replace(&mut self.pixels, vec![fill & PLANE_MASK; Self::area()]);
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
