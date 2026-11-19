//! Receiving a drawing's contents: everything between `D(B n)` and `D(E)`,
//! stored as received (`docs/regis-v2.md`, "Definitions").
//!
//! `D(E)` ends the contents only where ReGIS v1 reads `D` as a command
//! letter. A `D` in a quoted string, inside parentheses or brackets,
//! naming a macrograph (`@D`), inside a macrograph definition, or among
//! the hexadecimal digits of a character cell is part of the contents. So
//! the contents are scanned with the v1 lexer, in the mode v1 would be in.

use alloc::vec::Vec;

use super::Scene;
use crate::lexer::{DigitMode, Lexer, Token};

/// Where a `@` has left the scanner (manual chapter 9).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
enum At {
    #[default]
    None,
    /// `@` was read: the next byte names a macrograph or an operation.
    Seen,
    /// `@:` was read: the letter being defined is next.
    Letter,
    /// Inside a definition, which runs to `@;` whatever it holds.
    Defining { at_seen: bool },
}

/// How much of `D(E)` has been read; each holds where its `D` stands.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
enum Mark {
    #[default]
    None,
    Letter(usize),
    Open(usize),
    End(usize),
}

/// How far into loading a character the contents are (manual chapter 8).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
enum Load {
    #[default]
    No,
    /// The command in force is `L`.
    Command,
    /// A character cell is being loaded: letters A to F are then
    /// hexadecimal digits.
    Cell,
}

/// The bytes a definition's store first takes room for: a short drawing's
/// contents, so that it grows a few times and not at every byte.
const FIRST_CAPTURE_BYTES: usize = 64;

/// A definition being received.
#[derive(Debug)]
pub(super) struct Capture {
    target: Option<u16>,
    bytes: Vec<u8>,
    /// The contents passed the cap: no more are kept.
    overflowed: bool,
    lexer: Lexer,
    at: At,
    /// Parentheses open.
    depth: usize,
    load: Load,
    mark: Mark,
    /// Another `D` operation was read inside the definition.
    nested: bool,
}

/// A definition received whole.
pub(super) struct Captured {
    pub(super) target: Option<u16>,
    /// `None` if the contents passed the cap.
    pub(super) contents: Option<Vec<u8>>,
    pub(super) nested: bool,
}

/// The key letters of ReGIS v1 (manual chapter 1, table 1-1): each begins
/// a command, which ends a character cell being loaded.
const fn begins_command(letter: u8) -> bool {
    matches!(letter, b'S' | b'W' | b'P' | b'V' | b'C' | b'T' | b'L' | b'R' | b'F')
}

impl Capture {
    pub(super) fn new(target: Option<u16>) -> Self {
        Self {
            target,
            bytes: Vec::new(),
            overflowed: false,
            lexer: Lexer::default(),
            at: At::None,
            depth: 0,
            load: Load::No,
            mark: Mark::None,
            nested: false,
        }
    }

    pub(super) const fn target(&self) -> Option<u16> {
        self.target
    }

    /// The memory its kept bytes hold.
    pub(super) const fn held_bytes(&self) -> usize {
        self.bytes.capacity()
    }

    /// Takes one byte; the definition, once its `D(E)` has been read.
    pub(super) fn push(&mut self, byte: u8) -> Option<Captured> {
        let index = self.bytes.len();
        if index < Scene::contents_cap() && self.target.is_some() {
            if index == self.bytes.capacity() {
                // Grown as a vector grows, doubling, but never past the
                // cap: what a definition holds is at most the cap
                // (SOLVEDSECURITYISSUES 015).
                let more = index.max(FIRST_CAPTURE_BYTES).min(Scene::contents_cap().saturating_sub(index));
                self.bytes.reserve_exact(more);
            }
            self.bytes.push(byte);
        } else if self.target.is_some() {
            self.overflowed = true;
        }
        let end = self.scan(byte, index)?;
        let mut contents = core::mem::take(&mut self.bytes);
        contents.truncate(end);
        Some(Captured { target: self.target, contents: (!self.overflowed).then_some(contents), nested: self.nested })
    }

    /// Reads one byte as v1 would; where the contents end, if this byte
    /// closed `D(E)`.
    fn scan(&mut self, byte: u8, index: usize) -> Option<usize> {
        if !self.lexer.in_string() {
            match self.at {
                At::None if byte == b'@' => {
                    self.at = At::Seen;
                    self.leave_mark();
                    return None;
                }
                At::None => {}
                At::Seen => {
                    self.at = if byte == b':' { At::Letter } else { At::None };
                    return None;
                }
                At::Letter => {
                    self.at = if byte.is_ascii_alphabetic() { At::Defining { at_seen: false } } else { At::None };
                    return None;
                }
                At::Defining { at_seen } => {
                    self.at = if at_seen && byte == b';' { At::None } else { At::Defining { at_seen: byte == b'@' } };
                    return None;
                }
            }
        }
        loop {
            let lexed = self.lexer.push(byte, self.digit_mode());
            if let Some(token) = lexed.token
                && let Some(end) = self.token(&token, index)
            {
                return Some(end);
            }
            if lexed.consumed {
                return None;
            }
        }
    }

    fn digit_mode(&self) -> DigitMode {
        if self.depth > 0 || matches!(self.mark, Mark::Open(_) | Mark::End(_)) {
            DigitMode::Number
        } else if self.load == Load::Cell {
            DigitMode::Hex
        } else {
            DigitMode::Bare
        }
    }

    /// A `D` being followed turned out not to be `D(E)`. If its
    /// parenthesis was open, it stays open.
    fn leave_mark(&mut self) {
        if matches!(self.mark, Mark::Open(_) | Mark::End(_)) {
            self.depth = 1;
            self.nested = true;
        }
        self.mark = Mark::None;
    }

    fn token(&mut self, token: &Token, index: usize) -> Option<usize> {
        match (self.mark, token) {
            (Mark::Letter(start), Token::Open) => {
                self.mark = Mark::Open(start);
                return None;
            }
            (Mark::Open(start), Token::Letter(b'E')) => {
                self.mark = Mark::End(start);
                return None;
            }
            (Mark::End(start), Token::Close) => return Some(start),
            // `D()` is no operation.
            (Mark::Open(_), Token::Close) => {
                self.mark = Mark::None;
                return None;
            }
            (Mark::Open(_) | Mark::End(_), Token::Resync) | (Mark::Letter(_) | Mark::None, _) => self.mark = Mark::None,
            // Any other `D` operation: read on inside its parenthesis.
            (Mark::Open(_) | Mark::End(_), _) => self.leave_mark(),
        }
        match *token {
            Token::Resync => {
                self.depth = 0;
                self.load = Load::No;
            }
            Token::Open => self.depth = self.depth.saturating_add(1),
            Token::Close => self.depth = self.depth.saturating_sub(1),
            Token::Letter(b'D') if self.depth == 0 => {
                self.mark = Mark::Letter(index);
                self.load = Load::No;
            }
            Token::Letter(letter) if self.depth == 0 && begins_command(letter) => {
                self.load = if letter == b'L' { Load::Command } else { Load::No };
            }
            Token::Text(ref text) if self.depth == 0 && self.load != Load::No => {
                self.load = if text.is_empty() { Load::Command } else { Load::Cell };
            }
            _ => {}
        }
        None
    }
}

#[cfg(test)]
mod tests {
    use alloc::vec::Vec;

    use super::Capture;

    /// The contents taken from `input`, and how many bytes were read.
    fn contents(input: &[u8]) -> Option<(Vec<u8>, usize)> {
        let mut capture = Capture::new(Some(1));
        for (index, byte) in input.iter().enumerate() {
            if let Some(captured) = capture.push(*byte) {
                return Some((captured.contents?, index.saturating_add(1)));
            }
        }
        None
    }

    fn taken(input: &[u8]) -> Option<Vec<u8>> {
        contents(input).map(|(contents, _)| contents)
    }

    #[test]
    fn the_contents_run_to_the_end_marker_in_either_case_with_spaces() {
        assert_eq!(contents(b"P[1,2]V[3,4]D(E)V[9,9]"), Some((b"P[1,2]V[3,4]".to_vec(), 16)));
        assert_eq!(taken(b"V[3,4] d ( e )"), Some(b"V[3,4] ".to_vec()));
        assert_eq!(taken(b"D(E)"), Some(Vec::new()));
    }

    #[test]
    fn a_d_that_is_not_a_command_letter_is_contents() {
        // In a string, in an option, in a bracket, naming a macrograph.
        assert_eq!(taken(b"T'D(E)'D(E)"), Some(b"T'D(E)'".to_vec()));
        assert_eq!(taken(b"T(D90)'x'D(E)"), Some(b"T(D90)'x'".to_vec()));
        assert_eq!(taken(b"T(D(E))D(E)"), Some(b"T(D(E))".to_vec()));
        assert_eq!(taken(b"@D(E)D(E)"), Some(b"@D(E)".to_vec()));
    }

    #[test]
    fn hexadecimal_cell_data_is_not_an_end_marker() {
        // After L"a" the letters A to F are digits: DE is a row of the cell.
        assert_eq!(taken(b"L'a'DE,FF;D(E)"), Some(b"L'a'DE,FF;".to_vec()));
        // A new command ends the cell.
        assert_eq!(taken(b"L'a'FF P[1,1]D(E)"), Some(b"L'a'FF P[1,1]".to_vec()));
    }

    #[test]
    fn a_macrograph_definition_is_skipped_as_a_unit() {
        assert_eq!(taken(b"@:AD(E)@;V[1,1]D(E)"), Some(b"@:AD(E)@;V[1,1]".to_vec()));
    }

    #[test]
    fn a_semicolon_resynchronises_inside_parentheses() {
        assert_eq!(taken(b"W(I(;D(E)"), Some(b"W(I(;".to_vec()));
    }

    #[test]
    fn another_d_operation_is_noted_and_the_contents_run_on() {
        let mut capture = Capture::new(Some(1));
        let mut done = None;
        for byte in b"V[1,1]D(B2)V[2,2]D(E)" {
            done = done.or(capture.push(*byte));
        }
        let captured = done.unwrap();
        assert!(captured.nested);
        assert_eq!(captured.contents, Some(b"V[1,1]D(B2)V[2,2]".to_vec()));
    }
}
