//! The library's error type.

use core::fmt;

/// What can go wrong when the caller hands the library data of its own.
///
/// ReGIS input itself never produces an error: a terminal ignores what it
/// does not understand (manual chapter 10, error codes).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum Error {
    /// Font text is malformed.
    Font {
        /// The line at fault, counting from 1.
        line: usize,
    },
    /// Font text is longer than [`crate::limits::MAX_FONT_BYTES`].
    FontTooLong,
    /// A provided set's glyph is outside its bounds: a size past
    /// [`crate::limits::MAX_PROVIDED_GLYPH_WIDTH`] by
    /// [`crate::limits::MAX_PROVIDED_GLYPH_HEIGHT`], a code outside 2/0 to
    /// 7/15, or rows that are not the set's.
    ProvidedSet,
    /// A provided set's number is [`crate::limits::PROVIDED_SETS`] or more.
    ProvidedSetNumber,
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Font { line } => write!(f, "font text is malformed at line {line}"),
            Self::FontTooLong => f.write_str("font text is too long"),
            Self::ProvidedSet => f.write_str("a provided set's glyph is outside its bounds"),
            Self::ProvidedSetNumber => f.write_str("there is no provided set of that number"),
        }
    }
}

impl core::error::Error for Error {}
