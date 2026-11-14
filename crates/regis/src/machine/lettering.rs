//! The text state and the character sets.

use alloc::vec::Vec;

use super::load::LoadingCell;
use crate::font::{Font, Glyph, Loadable};
use crate::geometry::Point;
use crate::text::TextControls;

/// How many character sets a stream can load (manual chapter 8).
pub(crate) const LOADABLE_SETS: usize = 3;
/// Loadable sets hold seven-bit characters only (manual chapter 7).
const SEVEN_BITS: u8 = 0x7F;

/// The state of the text command.
#[derive(Debug, Default)]
pub(crate) struct Text {
    pub(crate) controls: TextControls,
    /// Controls saved by `T(B)`.
    pub(crate) saved: Vec<TextControls>,
    /// Where a carriage return goes back to.
    pub(crate) line_start: Point,
    /// A tilt was already given in this group of text options.
    pub(crate) tilted: bool,
}

/// The character shapes: the built-in set and the three loadable ones.
#[derive(Debug)]
pub(crate) struct Characters {
    pub(crate) font: Font,
    pub(crate) sets: [Loadable; LOADABLE_SETS],
    /// The set the load command works on: 0 to 3.
    pub(crate) loading: usize,
    /// The cell being loaded.
    pub(crate) cell: Option<LoadingCell>,
}

impl Default for Characters {
    /// Set 1 is selected for loading at power-up (manual table 1-2).
    fn default() -> Self {
        Self { font: Font::default(), sets: Default::default(), loading: 1, cell: None }
    }
}

impl Characters {
    /// The shape of a character in a set (0 built in, 1 to 3 loadable) at
    /// a unit cell height; the solid block if the set does not have it.
    pub(crate) fn glyph(&self, alphabet: usize, code: u8, unit_height: i32) -> Glyph {
        let found = match alphabet.checked_sub(1) {
            None => self.font.glyph(code, unit_height),
            Some(set) if code <= SEVEN_BITS => self.sets.get(set).and_then(|loadable| loadable.glyph(code)),
            Some(_) => None,
        };
        found.unwrap_or(Glyph::BLOCK)
    }

    /// The set being loaded; set 0 is built in and "cannot be loaded".
    pub(crate) fn loading_set(&mut self) -> Option<&mut Loadable> {
        self.sets.get_mut(self.loading.checked_sub(1)?)
    }

    /// The name of the set selected for loading, for the report command.
    pub(crate) fn loading_name(&self) -> &[u8] {
        let set = self.loading.checked_sub(1).and_then(|index| self.sets.get(index));
        set.map_or(&[], Loadable::name)
    }
}
