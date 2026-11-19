//! The character sets a ReGIS stream loads with the `L` command.

use alloc::boxed::Box;
use alloc::vec::Vec;

use super::{CELL_HEIGHT, Glyph};

/// A loadable set holds the 96 seven-bit characters from space to DEL
/// (manual chapter 8).
const CHARACTERS: usize = 96;
const FIRST_CODE: u8 = 0x20;
/// A set's name is "up to 10 characters" (manual chapter 8, "Specify Name").
const NAME_LENGTH: usize = 10;

/// The cells of a loadable set and which of them were loaded.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Cells {
    rows: [[u8; CELL_HEIGHT]; CHARACTERS],
    loaded: [bool; CHARACTERS],
}

/// What one loadable set's cells hold once the stream loads a character:
/// one allocation, with the allocator's bookkeeping.
pub(crate) const LOADED_CELLS_BYTES: usize = crate::limits::held_allocation(size_of::<Cells>());
/// The most one loadable set holds beyond its structure: its cells and its
/// name, whose vector grows by doubling to at most the next power of two
/// above [`NAME_LENGTH`].
pub(crate) const MOST_LOADABLE_HEAP_BYTES: usize =
    LOADED_CELLS_BYTES.saturating_add(crate::limits::held_allocation(NAME_LENGTH.next_power_of_two()));

/// One loadable character set. Its cells are made when the stream loads
/// the first character into it, so that a reader of a picture that loads
/// none, as nearly every picture is, holds none: three sets in each of the
/// stream's interpreter, the one that replays drawings and the committed
/// tables were 9,720 bytes of every reader (TODO-6895).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(crate) struct Loadable {
    name: Vec<u8>,
    cells: Option<Box<Cells>>,
}

impl Loadable {
    /// The memory it holds beyond its structure: the name and the cells,
    /// once loaded.
    pub(crate) fn heap_bytes(&self) -> usize {
        let name = match self.name.capacity() {
            0 => 0,
            bytes => crate::limits::held_allocation(bytes),
        };
        name.saturating_add(if self.cells.is_some() { LOADED_CELLS_BYTES } else { 0 })
    }

    fn index(code: u8) -> Option<usize> {
        code.checked_sub(FIRST_CODE).map(usize::from).filter(|index| *index < CHARACTERS)
    }

    pub(crate) fn name(&self) -> &[u8] {
        &self.name
    }

    /// Names the set. The name is sent back to the host by the report
    /// command, so it keeps only printable characters and no quote mark:
    /// the reply is then always one line (SOLVEDSECURITYISSUES 004).
    pub(crate) fn set_name(&mut self, name: &[u8]) {
        let safe = |byte: &u8| matches!(*byte, 0x20..=0x7E) && !matches!(*byte, b'\'' | b'"');
        self.name = name.iter().copied().filter(safe).take(NAME_LENGTH).collect();
    }

    /// Stores a cell; rows not given are blank.
    pub(crate) fn load(&mut self, code: u8, rows: &[u8]) {
        let Some(index) = Self::index(code) else { return };
        let cells = self.cells.get_or_insert_with(|| {
            Box::new(Cells { rows: [[0; CELL_HEIGHT]; CHARACTERS], loaded: [false; CHARACTERS] })
        });
        if let (Some(cell), Some(loaded)) = (cells.rows.get_mut(index), cells.loaded.get_mut(index)) {
            *cell = [0; CELL_HEIGHT];
            for (row, value) in cell.iter_mut().zip(rows) {
                *row = *value;
            }
            *loaded = true;
        }
    }

    pub(crate) fn glyph(&self, code: u8) -> Option<Glyph> {
        let index = Self::index(code)?;
        let cells = self.cells.as_ref()?;
        if !cells.loaded.get(index).copied().unwrap_or(false) {
            return None;
        }
        cells.rows.get(index).map(|cell| Glyph::from_rows(cell))
    }
}
