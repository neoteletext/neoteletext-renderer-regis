//! A renderer for DEC ReGIS (Remote Graphics Instruction Set), the vector
//! graphics language of the VT125, VT240, VT330 and VT340 terminals.
//!
//! Bytes go in; colour-map addresses come out on a [`Screen`] the caller
//! owns. The library does no input or output, reads no clock and keeps no
//! pixel memory of its own. It follows the VT330/VT340 Programmer Reference
//! Manual, Volume 2 ("the manual" in comments, cited by chapter).
//!
//! ```
//! use regis::{Screen, Terminal};
//!
//! let mut screen = Screen::new();
//! let mut terminal = Terminal::new();
//! let _ = terminal.feed(&mut screen, b"\x1bPpP[100,100]V[200,200]\x1b\\");
//! let _ = terminal.finish(&mut screen);
//! assert_eq!(screen.displayed().pixel(150, 150), Some(7));
//! ```
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

extern crate alloc;

mod addressing;
mod convert;
mod error;
mod event;
mod font;
mod geometry;
mod lexer;
pub mod limits;
mod machine;
mod macrograph;
mod number;
mod palette;
mod raster;
mod scene;
mod surface;
mod terminal;
mod text;
mod validator;
mod work;
mod write;

pub use error::Error;
pub use event::{CommitError, Event};
pub use font::Font;
pub use palette::{Palette, Rgb};
pub use scene::{AnimationState, ForcedLook, Pick, Picture, Rectangle, ViewerState};
pub use surface::{Canvas, SCREEN_HEIGHT, SCREEN_WIDTH, Screen};
pub use terminal::Terminal;
pub use validator::Validator;
pub use work::RepaintBudget;
