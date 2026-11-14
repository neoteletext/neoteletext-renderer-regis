//! The report command, `R` (manual chapter 10).

use alloc::vec::Vec;

use super::Machine;
use super::frames::Context;
use crate::event::Event;
use crate::lexer::Token;
use crate::limits::MAX_MACROGRAPH_BYTES;
use crate::macrograph::slot;

const CARRIAGE_RETURN: u8 = b'\r';
/// The bytes of a macrograph report around its contents: `@=x`, `@;`, CR.
const REPORT_FRAMING: usize = 6;

impl Machine {
    pub(super) fn report_option(&mut self, token: &Token) {
        match *token {
            Token::Letter(letter) => {
                // A report is sent when its option ends: at the next letter
                // or the closing parenthesis.
                self.finish_report(self.syntax.option_letter());
                self.syntax.set_option_letter(letter);
            }
            Token::Open => {
                let context = match self.syntax.option_letter() {
                    b'M' => Context::ReportMacrograph,
                    b'P' => Context::ReportPosition,
                    _ => Context::Skip,
                };
                self.syntax.set_option_letter(0);
                self.syntax.open(context);
            }
            _ => {}
        }
    }

    /// Sends the report a plain option letter asks for.
    pub(super) fn finish_report(&mut self, letter: u8) {
        match letter {
            b'P' => {
                let mut text = Vec::from(*b"[");
                push_decimal(&mut text, i64::from(self.position.x));
                text.push(b',');
                push_decimal(&mut text, i64::from(self.position.y));
                text.push(b']');
                self.reply(text);
            }
            b'L' => {
                let mut text = Vec::from(*b"A'");
                text.extend_from_slice(self.characters.loading_name());
                text.push(b'\'');
                self.reply(text);
            }
            // "When the terminal receives R(I), it returns a carriage
            // return" (manual chapter 10). There is no locator to show.
            b'I' => self.reply(Vec::new()),
            b'E' => {
                let mut text = Vec::from(*b"\"");
                push_decimal(&mut text, i64::from(self.fault.code));
                text.push(b',');
                push_decimal(&mut text, i64::from(self.fault.character));
                text.push(b'"');
                self.reply(text);
            }
            _ => {}
        }
        self.syntax.set_option_letter(0);
    }

    /// `R(M(letter))` and `R(M(=))`.
    pub(super) fn report_macrograph(&mut self, token: &Token) {
        match *token {
            Token::Letter(letter) => {
                // The contents are sent only if the caller allowed it: the
                // manual calls this report "a security risk".
                let body = match slot(letter) {
                    Some(slot) if self.macrograph_reports => self.macrographs.body(slot),
                    _ => &[],
                };
                // Nothing is copied for a reply that would be refused, and
                // only printable characters are sent back, so a reply is
                // always one line (SOLVEDSECURITYISSUES 011).
                if !self.outbox.affords_reply(body.len().saturating_add(REPORT_FRAMING)) {
                    self.outbox.limit_reached();
                    return;
                }
                let mut text = Vec::from(*b"@=");
                text.push(letter);
                text.extend(body.iter().copied().filter(|byte| matches!(*byte, 0x20..=0x7E)));
                text.extend_from_slice(b"@;");
                self.reply(text);
            }
            Token::Symbol(b'=') => {
                let mut text = Vec::from(*b"\"");
                push_decimal(&mut text, i64::try_from(self.macrographs.free()).unwrap_or(0));
                text.push(b',');
                push_decimal(&mut text, i64::try_from(MAX_MACROGRAPH_BYTES).unwrap_or(0));
                text.push(b'"');
                self.reply(text);
            }
            Token::Open => self.syntax.open(Context::Skip),
            _ => {}
        }
    }

    /// `R(P(I))`: the graphics input cursor is the caller's to report.
    pub(super) fn report_position(&mut self, token: &Token) {
        match *token {
            Token::Letter(b'I') => self.outbox.push(Event::LocatorRequest),
            Token::Open => self.syntax.open(Context::Skip),
            _ => {}
        }
    }

    /// Queues a report; every report "ends with a carriage return".
    fn reply(&mut self, mut text: Vec<u8>) {
        text.push(CARRIAGE_RETURN);
        self.outbox.reply(text);
    }
}

/// Appends a number in decimal.
fn push_decimal(out: &mut Vec<u8>, value: i64) {
    const TEN: u64 = 10;
    if value < 0 {
        out.push(b'-');
    }
    let mut magnitude = value.unsigned_abs();
    let start = out.len();
    loop {
        let digit = u8::try_from(magnitude.checked_rem(TEN).unwrap_or(0)).unwrap_or(0);
        out.push(b'0'.saturating_add(digit));
        magnitude = magnitude.checked_div(TEN).unwrap_or(0);
        if magnitude == 0 {
            break;
        }
    }
    if let Some(digits) = out.get_mut(start..) {
        digits.reverse();
    }
}
