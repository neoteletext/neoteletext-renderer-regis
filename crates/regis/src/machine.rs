//! The ReGIS interpreter: tokens in, drawing and state changes out.
//!
//! Each command of the language has a file here. The interpreter keeps an
//! explicit stack of the parentheses it is inside, so nesting costs no
//! recursion.

mod colour;
mod command;
mod curve;
mod drawing;
mod fill;
mod frames;
mod lettering;
mod load;
mod numbered;
mod outbox;
mod pen;
mod position;
mod report;
mod screen;
mod state;
mod syntax;
mod text;
mod write;

pub(crate) use lettering::LOADABLE_SETS;
pub(crate) use numbered::{Attribute, Clip, Frames, Operation};
pub(crate) use state::{Machine, Reading, Role};
