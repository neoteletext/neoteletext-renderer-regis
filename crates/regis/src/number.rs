//! ReGIS numbers: decimal strings with an optional sign, fraction and
//! power of ten (manual chapter 1, "Numeric Values").

use crate::limits::{MAX_COORDINATE, MIN_COORDINATE};

/// Thousandths kept of a number, so half-degrees can be rounded.
const MILLI: i64 = 1000;
/// The largest magnitude carried while digits arrive; far above any use.
const MAGNITUDE_CAP: i64 = 1_000_000_000_000;
/// Exponents beyond this cannot change the clamped result.
const EXPONENT_CAP: i32 = 40;
const TEN: i64 = 10;

/// A parsed number.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct Number {
    /// The value in thousandths, clamped to the coordinate range.
    milli: i64,
    /// Whether a `+` or `-` was written: in an extent that makes it relative.
    pub(crate) signed: bool,
    /// How many digits the whole part had.
    pub(crate) digits: u32,
    /// The last sixteen whole-part digits as bits: 1 for any digit but 0.
    pub(crate) bits: u16,
    /// The value truncated to an integer before it is clamped to the
    /// coordinate range: drawing numbers run to 65,535, beyond it.
    wide: i64,
}

impl Number {
    /// The value truncated to an integer, as the terminal does.
    pub(crate) fn value(self) -> i32 {
        i32::try_from(self.milli.checked_div(MILLI).unwrap_or(0)).unwrap_or(0)
    }

    /// The value rounded to the nearest integer (arcs are drawn "to the
    /// closest degree", manual chapter 6).
    /// The value truncated to an integer, not clamped to the coordinate
    /// range (but never beyond a million million either way).
    pub(crate) const fn wide(self) -> i64 {
        self.wide
    }

    pub(crate) fn rounded(self) -> i32 {
        let half = if self.milli < 0 { -(MILLI / 2) } else { MILLI / 2 };
        i32::try_from(self.milli.saturating_add(half).checked_div(MILLI).unwrap_or(0)).unwrap_or(0)
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
enum Stage {
    #[default]
    Start,
    Whole,
    Fraction,
    /// An `E` was seen; it is an exponent only if a digit follows.
    ExponentMark,
    ExponentSign,
    Exponent,
}

/// Reads a number one byte at a time.
#[derive(Clone, Copy, Debug, Default)]
pub(crate) struct Scanner {
    stage: Stage,
    negative: bool,
    signed: bool,
    magnitude: i64,
    /// Fraction digits taken into `magnitude` (at most three).
    fraction_digits: u32,
    digits: u32,
    bits: u16,
    exponent: i32,
    exponent_negative: bool,
    /// The bytes of an exponent mark that turned out not to be one.
    held: [u8; 2],
    held_len: usize,
}

impl Scanner {
    /// Takes the next byte if it continues the number.
    pub(crate) fn accept(&mut self, byte: u8) -> bool {
        match (self.stage, byte) {
            (Stage::Start, b'+' | b'-') if !self.signed => {
                self.signed = true;
                self.negative = byte == b'-';
            }
            (Stage::Start | Stage::Whole, b'0'..=b'9') => {
                self.stage = Stage::Whole;
                self.whole_digit(byte);
            }
            (Stage::Start | Stage::Whole, b'.') => self.stage = Stage::Fraction,
            (Stage::Fraction, b'0'..=b'9') => self.fraction_digit(byte),
            (Stage::Whole | Stage::Fraction, b'E' | b'e') => {
                self.stage = Stage::ExponentMark;
                self.hold(byte);
            }
            (Stage::ExponentMark, b'+' | b'-') => {
                self.stage = Stage::ExponentSign;
                self.exponent_negative = byte == b'-';
                self.hold(byte);
            }
            (Stage::ExponentMark | Stage::ExponentSign | Stage::Exponent, b'0'..=b'9') => {
                self.stage = Stage::Exponent;
                self.held_len = 0;
                let digit = i32::from(byte.saturating_sub(b'0'));
                self.exponent = self.exponent.saturating_mul(10).saturating_add(digit).min(EXPONENT_CAP);
            }
            _ => return false,
        }
        true
    }

    fn hold(&mut self, byte: u8) {
        if let Some(slot) = self.held.get_mut(self.held_len) {
            *slot = byte;
            self.held_len = self.held_len.saturating_add(1);
        }
    }

    fn whole_digit(&mut self, byte: u8) {
        let digit = i64::from(byte.saturating_sub(b'0'));
        self.magnitude = self.magnitude.saturating_mul(TEN).saturating_add(digit).min(MAGNITUDE_CAP);
        self.digits = self.digits.saturating_add(1);
        self.bits = (self.bits << 1) | u16::from(digit != 0);
    }

    fn fraction_digit(&mut self, byte: u8) {
        const KEPT: u32 = 3;
        if self.fraction_digits < KEPT {
            let digit = i64::from(byte.saturating_sub(b'0'));
            self.magnitude = self.magnitude.saturating_mul(TEN).saturating_add(digit).min(MAGNITUDE_CAP);
            self.fraction_digits = self.fraction_digits.saturating_add(1);
        }
    }

    /// The bytes read after the number proper that must be read again as
    /// ordinary input: an `E` (and sign) that no digit followed.
    pub(crate) fn held(&self) -> &[u8] {
        self.held.get(..self.held_len).unwrap_or(&[])
    }

    /// The number read, or `None` if no digit was.
    pub(crate) fn finish(&self) -> Option<Number> {
        if self.digits == 0 && self.fraction_digits == 0 {
            return None;
        }
        // `magnitude` holds the digits with `fraction_digits` of them after
        // the point; bring it to thousandths, then apply the exponent.
        let mut milli = self.magnitude;
        for _ in self.fraction_digits..3 {
            milli = milli.saturating_mul(TEN);
        }
        let exponent = if self.stage == Stage::Exponent { self.exponent } else { 0 };
        for _ in 0..exponent {
            milli = if self.exponent_negative {
                milli.checked_div(TEN).unwrap_or(0)
            } else {
                milli.saturating_mul(TEN).min(MAGNITUDE_CAP)
            };
        }
        if self.negative {
            milli = milli.saturating_neg();
        }
        let low = i64::from(MIN_COORDINATE).saturating_mul(MILLI);
        let high = i64::from(MAX_COORDINATE).saturating_mul(MILLI);
        let wide = milli.checked_div(MILLI).unwrap_or(0);
        Some(Number { milli: milli.clamp(low, high), signed: self.signed, digits: self.digits, bits: self.bits, wide })
    }
}

/// Parses a whole byte string as one number; surrounding spaces are allowed.
pub(crate) fn parse(text: &[u8]) -> Option<Number> {
    let mut scanner = Scanner::default();
    for &byte in text.trim_ascii() {
        if !scanner.accept(byte) {
            return None;
        }
    }
    if scanner.held().is_empty() { scanner.finish() } else { None }
}

#[cfg(test)]
mod tests {
    use super::parse;

    #[test]
    fn integers_fractions_and_exponents_truncate_to_integers() {
        assert_eq!(parse(b"100").map(super::Number::value), Some(100));
        assert_eq!(parse(b"6.25").map(super::Number::value), Some(6));
        assert_eq!(parse(b".1E3").map(super::Number::value), Some(100));
        assert_eq!(parse(b"1000E-1").map(super::Number::value), Some(100));
        assert_eq!(parse(b"-7.9").map(super::Number::value), Some(-7));
    }

    #[test]
    fn a_sign_is_remembered_and_rounding_is_to_nearest() {
        let number = parse(b"+27.5").unwrap();
        assert!(number.signed);
        assert_eq!(number.rounded(), 28);
        assert_eq!(parse(b"27.4").unwrap().rounded(), 27);
        assert_eq!(parse(b"-27.5").unwrap().rounded(), -28);
    }

    #[test]
    fn values_are_clamped_to_sixteen_bits() {
        assert_eq!(parse(b"99999999999999999999").map(super::Number::value), Some(32_767));
        assert_eq!(parse(b"-1E30").map(super::Number::value), Some(-32_768));
        assert_eq!(parse(b"1E-30").map(super::Number::value), Some(0));
    }

    #[test]
    fn digit_bits_record_a_binary_pattern() {
        let number = parse(b"11110000").unwrap();
        assert_eq!(number.digits, 8);
        assert_eq!(number.bits & 0xFF, 0b1111_0000);
    }

    #[test]
    fn text_that_is_not_a_number_is_refused() {
        assert_eq!(parse(b""), None);
        assert_eq!(parse(b"+"), None);
        assert_eq!(parse(b"12x"), None);
        assert_eq!(parse(b"3E"), None);
    }
}
