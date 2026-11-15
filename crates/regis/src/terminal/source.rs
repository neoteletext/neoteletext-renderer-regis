//! Macrographs at the level of the byte stream.
//!
//! A macrograph's contents are inserted "into the command stream" (manual
//! chapter 9), so they are handled before the lexer: an invocation can
//! supply any part of a command. Expansion uses an explicit stack, not
//! recursion.

use alloc::vec::Vec;

use crate::lexer::Lexer;
use crate::limits::{MAX_MACROGRAPH_BYTES, MAX_MACROGRAPH_DEPTH};
use crate::machine::{Machine, Reading};
use crate::macrograph::slot;
use crate::surface::Screen;

/// Work units charged for each byte executed out of a macrograph, so that
/// a short invocation cannot buy unbounded parsing.
const EXPANSION_COST: u64 = 64;

#[derive(Debug, Default)]
enum At {
    /// No `@` pending.
    #[default]
    None,
    /// `@` was read.
    Seen,
    /// `@:` was read; the call letter is next.
    Letter,
    /// Inside a definition, collecting up to `@;`.
    Defining { slot: usize, body: Vec<u8>, at_seen: bool, overflowed: bool },
}

/// A macrograph being executed.
#[derive(Clone, Copy, Debug)]
struct Call {
    slot: usize,
    /// How far into its body execution has come.
    at: usize,
}

/// The macrograph state of the input.
#[derive(Debug, Default)]
pub(super) struct Source {
    at: At,
    calls: Vec<Call>,
}

impl Source {
    pub(super) fn reset(&mut self) {
        *self = Self::default();
    }
}

/// What reads ReGIS commands and carries them out: macrograph expansion,
/// the lexer and the machine. The terminal has one for the stream; ReGIS
/// v2 has a second that replays the contents of numbered drawings.
#[derive(Debug)]
pub(crate) struct Interpreter {
    source: Source,
    lexer: Lexer,
    pub(crate) machine: Machine,
}

impl Interpreter {
    pub(crate) fn new() -> Self {
        Self { source: Source::default(), lexer: Lexer::default(), machine: Machine::new() }
    }

    /// Ends the input: acts on a number or string that was only waiting
    /// for the byte after it.
    pub(crate) fn finish(&mut self, screen: &mut Screen) {
        if let Some(token) = self.lexer.finish() {
            self.machine.token(screen, token);
        }
    }

    /// `ESC P 1 p`: "ReGIS begins at a new command".
    pub(crate) fn restart(&mut self, screen: &mut Screen) {
        self.lexer.reset();
        self.source.reset();
        self.machine.resynchronise(screen);
    }

    /// Forgets everything read but not acted on, without acting on it.
    pub(crate) fn forget_input(&mut self) {
        self.lexer.reset();
        self.source.reset();
    }

    /// Takes one byte of ReGIS commands from the stream, then runs any
    /// macrographs it invoked to their ends.
    pub(crate) fn command_byte(&mut self, screen: &mut Screen, byte: u8) {
        self.machine.reading = Reading::Stream;
        self.source_byte(screen, byte);
        self.machine.reading = Reading::Macrograph;
        while let Some(call) = self.source.calls.last_mut() {
            let Some(next) = self.machine.macrographs.body(call.slot).get(call.at).copied() else {
                self.source.calls.pop();
                continue;
            };
            call.at = call.at.saturating_add(1);
            if !self.machine.work.spend(EXPANSION_COST) {
                // The rest of the macrograph is dropped, perhaps from the
                // middle of a string or bracket, so the parser starts
                // afresh: what follows in the stream is read as commands
                // (SOLVEDSECURITYISSUES 012).
                self.machine.outbox.limit_reached();
                self.restart(screen);
                break;
            }
            self.source_byte(screen, next);
        }
        self.machine.reading = Reading::Stream;
    }

    /// One byte, from the stream or from a macrograph.
    fn source_byte(&mut self, screen: &mut Screen, byte: u8) {
        if self.lexer.in_string() {
            // "ReGIS does not recognize any commands in a quoted text string."
            self.lex(screen, byte);
            return;
        }
        match core::mem::take(&mut self.source.at) {
            At::None if byte == b'@' => self.source.at = At::Seen,
            At::None => self.lex(screen, byte),
            At::Seen => self.at_command(byte),
            At::Letter => {
                if let Some(slot) = slot(byte) {
                    self.source.at = At::Defining { slot, body: Vec::new(), at_seen: false, overflowed: false };
                }
            }
            At::Defining { slot, body, at_seen, overflowed } => self.define(slot, body, at_seen, overflowed, byte),
        }
    }

    /// The byte after `@`.
    fn at_command(&mut self, byte: u8) {
        match byte {
            // "You cannot define a macrograph while inside another."
            b':' if self.source.calls.is_empty() => self.source.at = At::Letter,
            // A numbered drawing neither clears nor defines macrographs
            // (`docs/regis-v2.md`, "Rules for contents").
            b'.' if self.machine.inside_drawing() => {}
            b'.' => self.machine.macrographs.clear(),
            _ => match slot(byte) {
                Some(slot) => self.invoke(slot),
                None => self.machine_fault(byte),
            },
        }
    }

    /// `@letter`: begins executing a macrograph. One already being
    /// executed is not begun again ("a macrograph cannot call itself"), and
    /// nesting stops at sixteen.
    fn invoke(&mut self, slot: usize) {
        let running = self.source.calls.iter().any(|call| call.slot == slot);
        if running || self.source.calls.len() >= MAX_MACROGRAPH_DEPTH {
            self.machine_fault(b'@');
        } else {
            self.source.calls.push(Call { slot, at: 0 });
        }
    }

    /// One byte of a definition; `@;` ends it.
    fn define(&mut self, slot: usize, mut body: Vec<u8>, at_seen: bool, mut overflowed: bool, byte: u8) {
        if at_seen && byte == b';' {
            if self.machine.inside_drawing() {
                return;
            }
            // A definition too long to keep is dropped, not cut short.
            self.machine.macrographs.define(slot, if overflowed { Vec::new() } else { body });
            return;
        }
        let mut keep = |value: u8| {
            if body.len() < MAX_MACROGRAPH_BYTES {
                body.push(value);
            } else {
                overflowed = true;
            }
        };
        if at_seen {
            keep(b'@');
        }
        if byte != b'@' {
            keep(byte);
        }
        self.source.at = At::Defining { slot, body, at_seen: byte == b'@', overflowed };
    }

    fn machine_fault(&mut self, byte: u8) {
        self.machine.note_ignored(byte);
    }

    /// Lexes one byte and interprets the tokens it completes.
    fn lex(&mut self, screen: &mut Screen, byte: u8) {
        loop {
            let lexed = self.lexer.push(byte, self.machine.digit_mode());
            if let Some(token) = lexed.token {
                self.machine.token(screen, token);
            }
            if lexed.consumed {
                break;
            }
        }
    }
}
