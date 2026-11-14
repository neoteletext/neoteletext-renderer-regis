//! The pieces a ReGIS command stream is made of.

use alloc::vec::Vec;

use super::Extent;
use crate::number::Number;

/// One lexical item.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum Token {
    /// A letter, folded to upper case.
    Letter(u8),
    /// `(`
    Open,
    /// `)`
    Close,
    /// `,` (and, between hex pairs, a space).
    Comma,
    /// `;`, the resynchronisation command.
    Resync,
    /// A bracketed extent.
    Extent(Extent),
    /// A quoted string, quotes removed.
    Text(Vec<u8>),
    /// A number inside an option.
    Number(Number),
    /// A bare digit at command level: a pixel vector.
    Digit(u8),
    /// A hexadecimal digit of a character cell being loaded.
    Hex(u8),
    /// Any other printable character.
    Symbol(u8),
}

/// What a digit means where the interpreter now stands.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum DigitMode {
    /// Each digit stands alone, as a pixel vector.
    Bare,
    /// Digits and the letters A to F are hexadecimal cell data.
    Hex,
    /// Digits build a number.
    Number,
}
