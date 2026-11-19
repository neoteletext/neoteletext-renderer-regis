//! Character shapes: the built-in set the caller supplies, and the three
//! sets a ReGIS stream can load.

mod glyph;
mod loadable;
mod parse;

use alloc::vec::Vec;

pub(crate) use glyph::Glyph;
pub(crate) use loadable::Loadable;

use crate::Error;

/// A stored character cell is eight pixels wide (manual chapter 7,
/// "Character Format").
pub(crate) const CELL_WIDTH: i32 = 8;
/// A stored character cell is ten pixels high.
pub(crate) const CELL_HEIGHT: usize = 10;
/// The tallest strike a font may carry: the VT340 draws size-1 text from a
/// twenty-row form of its font.
pub(crate) const TALL_HEIGHT: usize = 20;

/// The codes a font can hold: one byte each.
const CODES: usize = 256;

/// One size of a font: every glyph the same height.
#[derive(Clone, Debug)]
pub(crate) struct Strike {
    height: usize,
    /// Rows for all 256 codes, `height` bytes each; bit 7 is the left pixel.
    rows: Vec<u8>,
    present: [bool; CODES],
}

impl Strike {
    /// The memory its rows hold.
    fn heap_bytes(&self) -> usize {
        crate::limits::held_allocation(self.rows.capacity())
    }

    fn new(height: usize) -> Self {
        Self { height, rows: alloc::vec![0; CODES.saturating_mul(height)], present: [false; CODES] }
    }

    fn glyph(&self, code: u8) -> Option<Glyph> {
        let index = usize::from(code);
        if !self.present.get(index).copied().unwrap_or(false) {
            return None;
        }
        let start = index.saturating_mul(self.height);
        let rows = self.rows.get(start..start.saturating_add(self.height))?;
        Some(Glyph::from_rows(rows))
    }

    fn set(&mut self, code: u8, rows: &[u8]) {
        let index = usize::from(code);
        let start = index.saturating_mul(self.height);
        if let Some(slot) = self.rows.get_mut(start..start.saturating_add(self.height))
            && slot.len() == rows.len()
        {
            slot.copy_from_slice(rows);
            if let Some(present) = self.present.get_mut(index) {
                *present = true;
            }
        }
    }
}

/// The shapes of the terminal's built-in characters (alphabet 0).
///
/// The library compiles no font in: the caller supplies one as text, see
/// [`Font::parse`]. Without a font every character is the solid block the
/// VT340 shows for a character it does not have.
#[derive(Clone, Debug, Default)]
pub struct Font {
    /// The ten-row strike: the stored cell of the manual.
    short: Option<Strike>,
    /// The twenty-row strike, used when the cell height is a multiple of 20.
    tall: Option<Strike>,
}

impl Font {
    /// The memory its strikes hold beyond its structure.
    pub(crate) fn heap_bytes(&self) -> usize {
        [&self.short, &self.tall].into_iter().flatten().map(Strike::heap_bytes).fold(0, usize::saturating_add)
    }

    /// Reads a font from its text form.
    ///
    /// Lines are a `#` comment, a blank, a strike header `[strike 10]` or
    /// `[strike 20]`, or a glyph: a two-digit hexadecimal character code and
    /// then one two-digit hexadecimal byte per row, top row first, the left
    /// pixel in the high bit.
    ///
    /// # Errors
    ///
    /// [`Error::Font`] names the first malformed line; [`Error::FontTooLong`]
    /// refuses text over [`crate::limits::MAX_FONT_BYTES`].
    pub fn parse(text: &str) -> Result<Self, Error> {
        parse::parse(text)
    }

    /// The glyph for a character at a cell of `unit_height` pixels: the tall
    /// strike when the height is a whole number of its rows, else the short.
    pub(crate) fn glyph(&self, code: u8, unit_height: i32) -> Option<Glyph> {
        let fits_tall = usize::try_from(unit_height).is_ok_and(|height| height.checked_rem(TALL_HEIGHT) == Some(0));
        let tall = self.tall.as_ref().and_then(|strike| strike.glyph(code));
        let short = self.short.as_ref().and_then(|strike| strike.glyph(code));
        if fits_tall { tall.or(short) } else { short.or(tall) }
    }
}
