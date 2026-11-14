//! Rewrites ReGIS shorter without changing what it draws.
//!
//! The rewriter follows the active position and the write controls
//! through a command string. It drops blanks, writes every coordinate in
//! its shortest form, leaves out settings that change nothing or are
//! replaced before anything is drawn, and joins commands that can share a
//! letter. Quoted strings, macrograph definitions, load commands and
//! anything it does not fully understand are copied as written.
//!
//! It knows nothing of pictures: it cannot turn a thousand strokes into
//! one fill. That has to be done where the picture is made.
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

mod command;
mod point;
mod rewrite;
mod stream;
mod token;
mod write;

/// How far the rewriting goes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Level {
    /// Only changes that cannot alter a single pixel.
    Exact,
    /// Also leaves out a move to where the position already is, so the
    /// lines either side join. With a line pattern or complement writing
    /// the joint can differ, so the result should be checked by drawing.
    Joined,
}

/// Rewriting stops once a pass saves nothing, or after this many passes.
const MAX_PASSES: usize = 4;

fn pass(input: &[u8], level: Level) -> Vec<u8> {
    let mut out = Vec::with_capacity(input.len());
    for part in stream::split(input) {
        match part {
            stream::Part::Other(bytes) => out.extend_from_slice(bytes),
            stream::Part::Regis(commands) => {
                rewrite::rewrite(command::group(token::Lexer::new(commands)), level, &mut out);
            }
        }
    }
    out
}

/// Rewrites a ReGIS data stream, or bare ReGIS commands, shorter.
#[must_use]
pub fn minify(input: &[u8], level: Level) -> Vec<u8> {
    let mut best = input.to_vec();
    for _ in 0..MAX_PASSES {
        let next = pass(&best, level);
        if next.len() >= best.len() {
            break;
        }
        best = next;
    }
    best
}
