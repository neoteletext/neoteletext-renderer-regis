//! The write controls: how drawing changes pixels (manual chapter 3).

/// The four writing styles (manual chapter 3, "Writing Styles").
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) enum Style {
    /// Only pattern 1-bits are written.
    #[default]
    Overlay,
    /// Pattern 1-bits write the foreground and 0-bits the background.
    Replace,
    /// Pattern 1-bits invert the pixel's bits.
    Complement,
    /// Every pixel is written with the background.
    Erase,
}

/// Where shading reaches to (manual chapter 3, "Shading Control").
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Shade {
    /// A vertical reference line at pixel column `at`; otherwise a
    /// horizontal one at pixel row `at`.
    pub(crate) vertical: bool,
    pub(crate) at: i32,
    /// The character to shade with, if not the pattern.
    pub(crate) tile: Option<Tile>,
}

/// A shading character: the top eight rows of its cell, repeated over the
/// screen at the size of the text unit cell in force when it was chosen.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Tile {
    pub(crate) rows: [u8; 8],
    pub(crate) width: i32,
    pub(crate) height: i32,
}

/// The standard patterns (manual chapter 3, table 3-1).
const STANDARD_PATTERNS: [u8; 10] = [
    0b0000_0000,
    0b1111_1111,
    0b1111_0000,
    0b1110_0100,
    0b1010_1010,
    0b1110_1010,
    0b1000_1000,
    0b1000_0100,
    0b1100_1000,
    0b1000_0110,
];

/// Pattern memory holds eight bits (manual chapter 3, "Pattern Control").
pub(crate) const PATTERN_BITS: u32 = 8;

/// Everything the `W` command sets.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct WriteControls {
    pub(crate) pv_multiplier: i32,
    pub(crate) pattern: u8,
    pub(crate) pattern_multiplier: u32,
    pub(crate) negative: bool,
    /// The bit planes writing may change.
    pub(crate) planes: u8,
    pub(crate) foreground: u8,
    pub(crate) style: Style,
    pub(crate) shade: Option<Shade>,
}

impl Default for WriteControls {
    /// The power-up values for the VT340 (manual chapter 1, table 1-2).
    fn default() -> Self {
        Self {
            pv_multiplier: 1,
            pattern: 0b1111_1111,
            pattern_multiplier: 2,
            negative: false,
            planes: 0b1111,
            foreground: 7,
            style: Style::Overlay,
            shade: None,
        }
    }
}

impl WriteControls {
    /// Sets the pattern from a number as written: one digit picks a
    /// standard pattern, more digits are the bits themselves, repeated to
    /// fill the eight (manual chapter 3, "Specify Binary Pattern").
    pub(crate) fn set_pattern(&mut self, digits: u32, bits: u16, value: i32) {
        if digits <= 1 {
            let standard = usize::try_from(value).ok().and_then(|index| STANDARD_PATTERNS.get(index));
            if let Some(pattern) = standard {
                self.pattern = *pattern;
            }
            return;
        }
        let length = digits.min(PATTERN_BITS);
        let mut pattern = 0u8;
        for position in 0..PATTERN_BITS {
            let within = position.checked_rem(length).unwrap_or(0);
            let shift = length.saturating_sub(1).saturating_sub(within);
            let bit = u8::from((bits >> shift) & 1 == 1);
            pattern = (pattern << 1) | bit;
        }
        self.pattern = pattern;
    }

    /// The pattern bit for the `step`-th point drawn: the eight bits are
    /// read from the left, each held for the multiplier's count of points.
    pub(crate) fn pattern_bit(&self, step: u32) -> bool {
        let index =
            step.checked_div(self.pattern_multiplier.max(1)).unwrap_or(0).checked_rem(PATTERN_BITS).unwrap_or(0);
        (self.pattern >> (PATTERN_BITS.saturating_sub(1).saturating_sub(index))) & 1 == 1
    }
}

#[cfg(test)]
mod tests {
    use super::WriteControls;

    fn pattern_of(digits: u32, bits: u16, value: i32) -> u8 {
        let mut controls = WriteControls::default();
        controls.set_pattern(digits, bits, value);
        controls.pattern
    }

    #[test]
    fn one_digit_selects_a_standard_pattern() {
        assert_eq!(pattern_of(1, 1, 2), 0b1111_0000);
        assert_eq!(pattern_of(1, 1, 9), 0b1000_0110);
        assert_eq!(pattern_of(1, 0, 0), 0);
    }

    #[test]
    fn binary_patterns_repeat_to_fill_eight_bits() {
        assert_eq!(pattern_of(2, 0b01, 1), 0b0101_0101);
        assert_eq!(pattern_of(3, 0b110, 110), 0b1101_1011);
        assert_eq!(pattern_of(8, 0b1100_0011, 0), 0b1100_0011);
        // More than eight digits: the last eight count.
        assert_eq!(pattern_of(10, 0b11_0000_1111, 0), 0b0000_1111);
    }

    #[test]
    fn the_pattern_is_read_from_the_left_at_the_multiplier() {
        let mut controls = WriteControls::default();
        controls.set_pattern(1, 1, 2);
        controls.pattern_multiplier = 2;
        let bits: alloc::vec::Vec<bool> = (0..16).map(|step| controls.pattern_bit(step)).collect();
        assert_eq!(bits, [[true; 8], [false; 8]].concat());
    }
}
