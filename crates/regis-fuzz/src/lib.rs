//! The renderer's fuzz targets (neoteletext-common `docs/animation.md`,
//! ANIM-23), driven by property testing.
//!
//! The targets are the tests of this crate, in `tests/`; the library
//! itself holds nothing. They live apart from the renderer because the
//! library that drives them is a dependency, and the crate `regis` has
//! none: a program that builds the renderer in builds nothing of this
//! crate.
//!
//! Every input that ever found a fault is kept in `corpus/`, in a folder
//! named for its target, and replayed before anything else.
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
