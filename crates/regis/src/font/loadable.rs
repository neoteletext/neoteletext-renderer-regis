//! The character sets a ReGIS stream loads with the `L` command.

use alloc::vec::Vec;

use super::{CELL_HEIGHT, Glyph};

/// A loadable set holds the 96 seven-bit characters from space to DEL
/// (manual chapter 8).
const CHARACTERS: usize = 96;
const FIRST_CODE: u8 = 0x20;
/// A set's name is "up to 10 characters" (manual chapter 8, "Specify Name").
const NAME_LENGTH: usize = 10;

/// One loadable character set.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Loadable {
    name: Vec<u8>,
    cells: [[u8; CELL_HEIGHT]; CHARACTERS],
    loaded: [bool; CHARACTERS],
}

impl Default for Loadable {
    fn default() -> Self {
        Self { name: Vec::new(), cells: [[0; CELL_HEIGHT]; CHARACTERS], loaded: [false; CHARACTERS] }
    }
}

impl Loadable {
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
        if let (Some(cell), Some(loaded)) = (self.cells.get_mut(index), self.loaded.get_mut(index)) {
            *cell = [0; CELL_HEIGHT];
            for (row, value) in cell.iter_mut().zip(rows) {
                *row = *value;
            }
            *loaded = true;
        }
    }

    pub(crate) fn glyph(&self, code: u8) -> Option<Glyph> {
        let index = Self::index(code)?;
        if !self.loaded.get(index).copied().unwrap_or(false) {
            return None;
        }
        self.cells.get(index).map(|cell| Glyph::from_rows(cell))
    }
}
