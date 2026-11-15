//! Finding ReGIS in a terminal data stream.
//!
//! ReGIS travels in a device control string: `ESC P`, optional numeric
//! parameters, the letter `p`, the commands, and the string terminator
//! `ESC \` (manual chapter 1, "Entering and Exiting ReGIS"). The 8-bit
//! forms DCS (0x90) and ST (0x9C) are honoured only if the caller asks:
//! both bytes occur inside UTF-8 text (SOLVEDSECURITYISSUES 010).

use crate::limits::MAX_DCS_PARAMETER;

const ESCAPE: u8 = 0x1B;
const CANCEL: u8 = 0x18;
const SUBSTITUTE: u8 = 0x1A;
const DCS: u8 = 0x90;
const STRING_TERMINATOR: u8 = 0x9C;
/// Stands for a byte with no meaning to the framing.
const ORDINARY: u8 = b'?';

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
enum State {
    #[default]
    Ground,
    Escape,
    /// Reading the parameters of a device control string; only the first
    /// matters to ReGIS.
    Parameters {
        first: u32,
        later: bool,
    },
    /// A device control string that is not ReGIS: skipped to its end.
    Other,
    OtherEscape,
    Regis,
    RegisEscape,
}

/// What one byte of the stream is.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Step {
    /// Not part of ReGIS.
    Outside,
    /// A ReGIS string begins; `restart` for modes 1 and 3, which begin "at
    /// the command level".
    Enter { restart: bool },
    /// A byte of ReGIS commands.
    Regis,
}

/// The framing state.
#[derive(Clone, Copy, Debug, Default)]
pub(super) struct Stream {
    state: State,
    eight_bit: bool,
}

impl Stream {
    pub(super) const fn recognise_eight_bit_controls(&mut self, recognise: bool) {
        self.eight_bit = recognise;
    }

    /// Whether the last byte left the stream inside ReGIS commands.
    pub(super) const fn in_regis(&self) -> bool {
        matches!(self.state, State::Regis)
    }

    #[allow(
        clippy::match_same_arms,
        reason = "the arms are tried in order, so equal results cannot be merged across the arms between them"
    )]
    pub(super) fn step(&mut self, byte: u8) -> Step {
        // Unless asked for, the 8-bit controls are bytes like any other.
        let byte = if !self.eight_bit && matches!(byte, DCS | STRING_TERMINATOR) { ORDINARY } else { byte };
        let (state, step) = match (self.state, byte) {
            (State::Regis, ESCAPE) => (State::RegisEscape, Step::Outside),
            (State::Regis, STRING_TERMINATOR | CANCEL | SUBSTITUTE) => (State::Ground, Step::Outside),
            (State::Regis, _) => (State::Regis, Step::Regis),
            // An escape inside the string ends it whatever follows.
            (State::RegisEscape | State::Escape | State::OtherEscape, b'P') | (_, DCS) => {
                (State::Parameters { first: 0, later: false }, Step::Outside)
            }
            (State::Ground | State::RegisEscape | State::Escape, ESCAPE) => (State::Escape, Step::Outside),
            (State::Parameters { first, later }, b'0'..=b'9') if !later => {
                let digit = u32::from(byte.saturating_sub(b'0'));
                let first = first.saturating_mul(10).saturating_add(digit).min(MAX_DCS_PARAMETER);
                (State::Parameters { first, later }, Step::Outside)
            }
            (State::Parameters { first, .. }, b'0'..=b'9' | b';') => {
                (State::Parameters { first, later: true }, Step::Outside)
            }
            (State::Parameters { first, .. }, b'p') => {
                (State::Regis, Step::Enter { restart: first == 1 || first == 3 })
            }
            (State::Parameters { .. } | State::Other | State::OtherEscape, ESCAPE) => {
                (State::OtherEscape, Step::Outside)
            }
            (State::Parameters { .. } | State::Other, STRING_TERMINATOR | CANCEL | SUBSTITUTE)
            | (State::OtherEscape, b'\\') => (State::Ground, Step::Outside),
            (State::Parameters { .. } | State::Other | State::OtherEscape, _) => (State::Other, Step::Outside),
            (State::Ground | State::Escape | State::RegisEscape, _) => (State::Ground, Step::Outside),
        };
        self.state = state;
        step
    }
}

#[cfg(test)]
mod tests {
    use alloc::vec::Vec;

    use super::{Step, Stream};

    /// The ReGIS bytes of a stream, with `!` where a string restarted.
    fn regis(input: &[u8]) -> Vec<u8> {
        regis_with(input, false)
    }

    fn regis_with(input: &[u8], eight_bit: bool) -> Vec<u8> {
        let mut stream = Stream::default();
        stream.recognise_eight_bit_controls(eight_bit);
        let mut out = Vec::new();
        for &byte in input {
            match stream.step(byte) {
                Step::Regis => out.push(byte),
                Step::Enter { restart: true } => out.push(b'!'),
                Step::Enter { restart: false } | Step::Outside => {}
            }
        }
        out
    }

    #[test]
    fn commands_are_taken_from_inside_the_control_string_only() {
        assert_eq!(regis(b"text\x1bPpP[1,2]\x1b\\more"), b"P[1,2]");
        assert_eq!(regis(b"\x1bP0pV1\x1b\\\x1bP2pV2\x1b\\"), b"V1V2");
    }

    #[test]
    fn modes_one_and_three_restart() {
        assert_eq!(regis(b"\x1bP1pS\x1b\\\x1bP3pW\x1b\\"), b"!S!W");
        assert_eq!(regis(b"\x1bP1;5pS\x1b\\"), b"!S");
    }

    #[test]
    fn eight_bit_controls_work_only_when_asked_for() {
        assert_eq!(regis_with(b"\x90pV3\x9cV4", true), b"V3");
        assert_eq!(regis(b"\x90pV3\x9cV4"), b"");
        // Inside a string they are then ordinary bytes of the commands.
        assert_eq!(regis(b"\x1bPpT'\xd0\x9c'\x1b\\"), b"T'\xd0\x9c'");
    }

    #[test]
    fn other_control_strings_are_skipped_whole() {
        assert_eq!(regis(b"\x1bPqV5\x1b\\\x1bPpV6\x1b\\"), b"V6");
        assert_eq!(regis(b"\x1bP1$qV5\x1b\\"), b"");
    }

    #[test]
    fn an_escape_or_cancel_ends_the_string() {
        assert_eq!(regis(b"\x1bPpV1\x1b[0mV2"), b"V1");
        assert_eq!(regis(b"\x1bPpV1\x18V2"), b"V1");
        assert_eq!(regis(b"\x1bPpV1\x1bPpV2"), b"V1V2");
    }
}
