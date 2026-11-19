//! The byte-at-a-time scanner.

use alloc::vec::Vec;

use super::{DigitMode, Extent, Token};
use crate::limits::{MAX_EXTENT_BYTES, MAX_STRING_BYTES};
use crate::number::Scanner;

#[derive(Debug, Default)]
enum State {
    #[default]
    Idle,
    Number(Scanner),
    Extent(Vec<u8>),
    /// Inside a string opened by this quote mark.
    Quote(u8, Vec<u8>),
    /// A closing quote was read; a second one right after means a literal quote.
    QuoteEnd(u8, Vec<u8>),
}

/// What one call to [`Lexer::push`] produced.
#[derive(Debug, Default)]
pub(crate) struct Lexed {
    pub(crate) token: Option<Token>,
    /// False when the byte must be offered again (it ended one token and may
    /// begin another, or held-back bytes were read first).
    pub(crate) consumed: bool,
}

/// The lexer's state between bytes.
#[derive(Debug, Default)]
pub(crate) struct Lexer {
    state: State,
    /// Bytes to read again before any new input.
    replay: [u8; 2],
    replay_len: usize,
}

impl Lexer {
    /// The memory the string or extent being read holds.
    pub(crate) fn heap_bytes(&self) -> usize {
        match &self.state {
            State::Extent(bytes) | State::Quote(_, bytes) | State::QuoteEnd(_, bytes) => {
                crate::limits::vector_bytes(bytes)
            }
            State::Idle | State::Number(_) => 0,
        }
    }

    /// Whether the lexer is inside a quoted string, where `@` and `;` are text.
    pub(crate) const fn in_string(&self) -> bool {
        matches!(self.state, State::Quote(..))
    }

    /// Forgets any token in progress.
    pub(crate) fn reset(&mut self) {
        *self = Self::default();
    }

    /// Offers one byte. Call again with the same byte until it is consumed.
    pub(crate) fn push(&mut self, byte: u8, mode: DigitMode) -> Lexed {
        if let Some(first) = self.take_replay() {
            return Lexed { token: self.start(first, mode), consumed: false };
        }
        match core::mem::take(&mut self.state) {
            State::Idle => Lexed { token: self.start(byte, mode), consumed: true },
            State::Number(scanner) => self.number(scanner, byte),
            State::Extent(buffer) => self.extent(buffer, byte),
            State::Quote(quote, buffer) => self.quote(quote, buffer, byte),
            State::QuoteEnd(quote, mut buffer) if byte == quote => {
                push_bounded(&mut buffer, byte, MAX_STRING_BYTES);
                self.state = State::Quote(quote, buffer);
                Lexed { token: None, consumed: true }
            }
            State::QuoteEnd(_, buffer) => Lexed { token: Some(Token::Text(buffer)), consumed: false },
        }
    }

    /// Ends the input: yields a token that was only waiting for its end.
    pub(crate) fn finish(&mut self) -> Option<Token> {
        match core::mem::take(&mut self.state) {
            State::Number(scanner) => scanner.finish().map(Token::Number),
            State::QuoteEnd(_, buffer) => Some(Token::Text(buffer)),
            other => {
                self.state = other;
                None
            }
        }
    }

    fn take_replay(&mut self) -> Option<u8> {
        let first = *self.replay.first()?;
        if self.replay_len == 0 {
            return None;
        }
        self.replay.rotate_left(1);
        self.replay_len = self.replay_len.saturating_sub(1);
        Some(first)
    }

    fn number(&mut self, mut scanner: Scanner, byte: u8) -> Lexed {
        if scanner.accept(byte) {
            self.state = State::Number(scanner);
            return Lexed { token: None, consumed: true };
        }
        for (slot, held) in self.replay.iter_mut().zip(scanner.held()) {
            *slot = *held;
        }
        self.replay_len = scanner.held().len().min(self.replay.len());
        Lexed { token: scanner.finish().map(Token::Number), consumed: false }
    }

    fn extent(&mut self, mut buffer: Vec<u8>, byte: u8) -> Lexed {
        let token = match byte {
            b']' => Some(Token::Extent(Extent::parse(&buffer))),
            // A lost bracket must not swallow the stream (manual chapter 1,
            // "Resynchronization Command").
            b';' => Some(Token::Resync),
            _ => {
                push_bounded(&mut buffer, byte, MAX_EXTENT_BYTES);
                self.state = State::Extent(buffer);
                None
            }
        };
        Lexed { token, consumed: true }
    }

    fn quote(&mut self, quote: u8, mut buffer: Vec<u8>, byte: u8) -> Lexed {
        if byte == quote {
            self.state = State::QuoteEnd(quote, buffer);
        } else {
            if is_text(byte) {
                push_bounded(&mut buffer, byte, MAX_STRING_BYTES);
            }
            self.state = State::Quote(quote, buffer);
        }
        Lexed { token: None, consumed: true }
    }

    /// Reads a byte that begins a token (or is one, or is ignored).
    fn start(&mut self, byte: u8, mode: DigitMode) -> Option<Token> {
        match byte {
            b'(' => Some(Token::Open),
            b')' => Some(Token::Close),
            b',' => Some(Token::Comma),
            b';' => Some(Token::Resync),
            b'[' => {
                self.state = State::Extent(Vec::new());
                None
            }
            b'\'' | b'"' => {
                self.state = State::Quote(byte, Vec::new());
                None
            }
            b'0'..=b'9' | b'+' | b'-' | b'.' => self.digit(byte, mode),
            b'a'..=b'z' | b'A'..=b'Z' => Some(letter(byte.to_ascii_uppercase(), mode)),
            b' ' if mode == DigitMode::Hex => Some(Token::Comma),
            0x21..=0x7E => Some(Token::Symbol(byte)),
            // Control characters, space and eight-bit characters mean
            // nothing outside a string (manual chapter 1, "Control Characters").
            _ => None,
        }
    }

    fn digit(&mut self, byte: u8, mode: DigitMode) -> Option<Token> {
        let value = byte.checked_sub(b'0').filter(|value| *value <= 9);
        match (mode, value) {
            (DigitMode::Bare, Some(value)) => Some(Token::Digit(value)),
            (DigitMode::Hex, Some(value)) => Some(Token::Hex(value)),
            (DigitMode::Number, _) => {
                let mut scanner = Scanner::default();
                scanner.accept(byte);
                self.state = State::Number(scanner);
                None
            }
            // A sign or point where no number can stand.
            (DigitMode::Bare | DigitMode::Hex, None) => Some(Token::Symbol(byte)),
        }
    }
}

fn letter(upper: u8, mode: DigitMode) -> Token {
    const FIRST_HEX_LETTER_VALUE: u8 = 10;
    match upper {
        b'A'..=b'F' if mode == DigitMode::Hex => {
            Token::Hex(upper.saturating_sub(b'A').saturating_add(FIRST_HEX_LETTER_VALUE))
        }
        _ => Token::Letter(upper),
    }
}

/// Characters a string keeps: printable ones and the four controls ReGIS
/// knows (manual chapter 7, "Control Characters in Text Strings").
const fn is_text(byte: u8) -> bool {
    matches!(byte, 0x08 | 0x09 | 0x0A | 0x0D | 0x20..=0x7E | 0xA0..=0xFF)
}

fn push_bounded(buffer: &mut Vec<u8>, byte: u8, limit: usize) {
    if buffer.len() < limit {
        buffer.push(byte);
    }
}

#[cfg(test)]
mod tests;
