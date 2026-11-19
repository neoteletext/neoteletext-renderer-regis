//! The load command, `L`: defining characters (manual chapter 8).

use alloc::vec::Vec;

use super::Machine;
use super::command::errors;
use super::frames::Context;
use super::lettering::LOADABLE_SETS;
use crate::font::CELL_HEIGHT;
use crate::lexer::Token;

/// A character cell being loaded.
#[derive(Clone, Debug)]
pub(crate) struct LoadingCell {
    /// The character that will call the cell up.
    code: u8,
    rows: Vec<u8>,
    /// The hexadecimal digits of the row in progress: value and count.
    pair: (u8, u8),
}

/// Bits in a hexadecimal digit.
const NIBBLE: u32 = 4;

impl Machine {
    pub(super) fn load_option(&mut self, token: &Token) {
        if self.inside_drawing() {
            // A numbered drawing loads nothing; its options are read and dropped.
            match *token {
                Token::Letter(letter) => self.syntax.set_option_letter(letter),
                Token::Open => self.syntax.open(Context::Skip),
                _ => {}
            }
            return;
        }
        match *token {
            Token::Letter(letter) => self.syntax.set_option_letter(letter),
            Token::Number(number) if self.syntax.option_letter() == b'A' => match usize::try_from(number.value()) {
                Ok(alphabet @ 0..=LOADABLE_SETS) => self.characters.loading = alphabet,
                _ => self.note_fault(errors::ALPHABET_RANGE, 0),
            },
            Token::Text(ref name) if self.syntax.option_letter() == b'A' => {
                if let Some(alphabet) = self.characters.loading_set() {
                    alphabet.set_name(name);
                }
            }
            Token::Open => self.syntax.open(Context::Skip),
            _ => {}
        }
    }

    /// A quoted call letter: the cell's rows follow as hexadecimal pairs.
    pub(super) fn begin_cell(&mut self, text: &[u8]) {
        self.finish_cell();
        if let Some(code) = text.first() {
            self.characters.cell = Some(LoadingCell { code: *code, rows: Vec::new(), pair: (0, 0) });
        }
    }

    /// One hexadecimal digit. Two make a row; a third begins the next row
    /// "as if you used a comma after each pair".
    pub(super) fn cell_digit(&mut self, value: u8) {
        let Some(cell) = &mut self.characters.cell else { return };
        if cell.pair.1 == 2 {
            cell.end_row();
        }
        cell.pair = ((cell.pair.0 << NIBBLE) | (value & 0x0F), cell.pair.1.saturating_add(1));
    }

    pub(super) fn cell_separator(&mut self) {
        if let Some(cell) = &mut self.characters.cell {
            cell.end_row();
        }
    }

    /// Stores the cell being loaded, if any.
    pub(super) fn finish_cell(&mut self) {
        let Some(mut cell) = self.characters.cell.take() else { return };
        if self.inside_drawing() {
            // The cell was read only so that its hexadecimal digits were
            // not taken for commands.
            return;
        }
        cell.end_row();
        if let Some(alphabet) = self.characters.loading_set() {
            alphabet.load(cell.code, &cell.rows);
        }
    }
}

impl LoadingCell {
    /// The memory its rows hold.
    pub(crate) fn heap_bytes(&self) -> usize {
        crate::limits::vector_bytes(&self.rows)
    }

    /// Ends the row in progress. A single digit is the row's right half:
    /// "ReGIS assumes the first hex value is 0".
    fn end_row(&mut self) {
        if self.pair.1 > 0 && self.rows.len() < CELL_HEIGHT {
            self.rows.push(self.pair.0);
        }
        self.pair = (0, 0);
    }
}
