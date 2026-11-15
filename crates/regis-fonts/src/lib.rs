//! The fonts this repository makes, as the text [`regis::Font::parse`]
//! reads.
//!
//! The renderer takes its font as data and reads no file. A program that
//! builds the renderer in and has no file to read, such as a confined
//! process, takes the text from here.
//!
//! [`regis::Font::parse`]: https://docs.rs/regis
#![no_std]
#![deny(
    clippy::indexing_slicing,
    clippy::string_slice,
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::arithmetic_side_effects,
    clippy::unreachable,
    clippy::todo,
    clippy::unimplemented
)]

/// The VT340's own characters, made from its character generator ROM by
/// `tools/rom_to_regisfont.py` (`fonts/vt340.regisfont`).
pub const VT340: &str = include_str!("../../../fonts/vt340.regisfont");
