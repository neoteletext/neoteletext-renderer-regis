//! One character's shape, copied out of a font or a provided set.

use super::{CELL_HEIGHT, CELL_WIDTH, TALL_HEIGHT};
use crate::convert::to_usize;
use crate::limits::{MAX_PROVIDED_GLYPH_HEIGHT, MAX_PROVIDED_GLYPH_WIDTH};

/// The most rows a glyph holds: a provided set's tallest, which is taller
/// than the built-in font's tall strike.
const MAX_ROWS: usize = MAX_PROVIDED_GLYPH_HEIGHT as usize;
const _: () = assert!(MAX_ROWS >= TALL_HEIGHT);

/// The columns of the shading tile a glyph gives (manual chapter 3,
/// "Shading": "the top 8 × 8 matrix").
const TILE_COLUMNS: u32 = 8;

/// A character shape: up to 32 rows of up to 16 pixels, each row a bit
/// field whose most significant of `width` bits is the left pixel.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Glyph {
    rows: [u16; MAX_ROWS],
    width: u8,
    height: usize,
}

impl Glyph {
    /// The solid block shown for a character the set does not have (manual
    /// chapter 7, "Selecting User-Defined (Loadable) Sets").
    pub(crate) const BLOCK: Self = Self { rows: [0xFF; MAX_ROWS], width: BUILT_IN_WIDTH, height: CELL_HEIGHT };

    /// The empty cell a provided alphabet draws for a code it has no glyph
    /// for (renderer-parity.md PAR-12).
    pub(crate) const EMPTY: Self = Self { rows: [0; MAX_ROWS], width: BUILT_IN_WIDTH, height: CELL_HEIGHT };

    /// A glyph of the built-in font or a loadable set: eight pixels wide.
    pub(crate) fn from_rows(source: &[u8]) -> Self {
        let mut rows = [0; MAX_ROWS];
        let height = source.len().min(TALL_HEIGHT);
        for (row, value) in rows.iter_mut().zip(source) {
            *row = u16::from(*value);
        }
        Self { rows, width: BUILT_IN_WIDTH, height }
    }

    /// A glyph of a provided set, `width` pixels wide; the caller checked
    /// the bounds when the set was made.
    pub(crate) fn from_wide_rows(source: &[u16], width: u8) -> Self {
        let mut rows = [0; MAX_ROWS];
        let height = source.len().min(MAX_ROWS);
        for (row, value) in rows.iter_mut().zip(source) {
            *row = *value;
        }
        Self { rows, width: width.clamp(1, MAX_PROVIDED_GLYPH_WIDTH), height }
    }

    /// Whether the pixel at (`u`, `v`) of a `width` by `height` unit cell is
    /// on: the stored shape is stretched over the cell by repeating pixels,
    /// or shrunk onto it by leaving them out, whatever its own size.
    pub(crate) fn lit(&self, u: i32, v: i32, width: i32, height: i32) -> bool {
        if u < 0 || v < 0 || u >= width || v >= height {
            return false;
        }
        let column = scale(u, i32::from(self.width), width);
        let row = scale(v, i32::try_from(self.height).unwrap_or(0), height);
        let bits = self.rows.get(to_usize(row)).copied().unwrap_or(0);
        let left_bit = u32::from(self.width).saturating_sub(1);
        u32::try_from(column).is_ok_and(|column| {
            column <= left_bit && bits.checked_shr(left_bit.saturating_sub(column)).is_some_and(|bits| bits & 1 == 1)
        })
    }

    /// The top eight rows, the part used for shading: the left eight
    /// columns of a wider glyph.
    pub(crate) fn top_rows(&self) -> [u8; 8] {
        let width = u32::from(self.width);
        let mut top = [0; 8];
        for (row, value) in top.iter_mut().zip(self.rows) {
            let left = if width >= TILE_COLUMNS {
                value.checked_shr(width.saturating_sub(TILE_COLUMNS))
            } else {
                value.checked_shl(TILE_COLUMNS.saturating_sub(width))
            };
            *row = u8::try_from(left.unwrap_or(0) & 0xFF).unwrap_or(0);
        }
        top
    }
}

/// The built-in font's and the loadable sets' glyphs are the stored cell's
/// width.
const BUILT_IN_WIDTH: u8 = 8;
const _: () = assert!(CELL_WIDTH == BUILT_IN_WIDTH as i32);

/// `position` of `out_of` mapped onto `count` cells.
fn scale(position: i32, count: i32, out_of: i32) -> i32 {
    let scaled = i64::from(position).saturating_mul(i64::from(count));
    i32::try_from(scaled.checked_div(i64::from(out_of)).unwrap_or(0)).unwrap_or(0)
}
