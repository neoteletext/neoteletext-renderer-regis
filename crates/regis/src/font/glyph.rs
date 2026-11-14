//! One character's shape, copied out of a font.

use super::{CELL_HEIGHT, CELL_WIDTH, TALL_HEIGHT};
use crate::convert::to_usize;

/// A character shape: up to twenty rows of eight pixels.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Glyph {
    rows: [u8; TALL_HEIGHT],
    height: usize,
}

impl Glyph {
    /// The solid block shown for a character the set does not have (manual
    /// chapter 7, "Selecting User-Defined (Loadable) Sets").
    pub(crate) const BLOCK: Self = Self { rows: [0xFF; TALL_HEIGHT], height: CELL_HEIGHT };

    pub(crate) fn from_rows(source: &[u8]) -> Self {
        let mut rows = [0; TALL_HEIGHT];
        let height = source.len().min(TALL_HEIGHT);
        for (row, value) in rows.iter_mut().zip(source) {
            *row = *value;
        }
        Self { rows, height }
    }

    /// Whether the pixel at (`u`, `v`) of a `width` by `height` unit cell is
    /// on: the stored shape is stretched over the cell by repeating pixels.
    pub(crate) fn lit(&self, u: i32, v: i32, width: i32, height: i32) -> bool {
        const LEFT_BIT: u32 = 7;
        if u < 0 || v < 0 || u >= width || v >= height {
            return false;
        }
        let column = scale(u, CELL_WIDTH, width);
        let row = scale(v, i32::try_from(self.height).unwrap_or(0), height);
        let bits = self.rows.get(to_usize(row)).copied().unwrap_or(0);
        u32::try_from(column)
            .is_ok_and(|column| column <= LEFT_BIT && (bits >> (LEFT_BIT.saturating_sub(column))) & 1 == 1)
    }

    /// The top eight rows, the part used for shading.
    pub(crate) fn top_rows(&self) -> [u8; 8] {
        let mut top = [0; 8];
        for (row, value) in top.iter_mut().zip(self.rows) {
            *row = value;
        }
        top
    }
}

/// `position` of `out_of` mapped onto `count` cells.
fn scale(position: i32, count: i32, out_of: i32) -> i32 {
    let scaled = i64::from(position).saturating_mul(i64::from(count));
    i32::try_from(scaled.checked_div(i64::from(out_of)).unwrap_or(0)).unwrap_or(0)
}
