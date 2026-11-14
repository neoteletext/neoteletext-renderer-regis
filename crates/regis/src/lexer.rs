//! Turns ReGIS bytes into tokens, one byte at a time.
//!
//! The lexer never looks ahead: it can be fed a stream cut at any byte.

mod extent;
mod scan;
mod token;

pub(crate) use extent::{Axis, Extent};
pub(crate) use scan::Lexer;
pub(crate) use token::{DigitMode, Token};
