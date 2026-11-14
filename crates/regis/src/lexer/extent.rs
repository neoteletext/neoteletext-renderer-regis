//! Bracketed extents: `[x,y]`, either part optional, either part relative.

use alloc::vec::Vec;

use crate::number;

/// One coordinate of an extent.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Axis {
    pub(crate) value: i32,
    /// Written with a sign, so measured from the current position.
    pub(crate) relative: bool,
}

impl Axis {
    /// The coordinate this names, given the current one.
    pub(crate) fn resolve(self, current: i32) -> i32 {
        if self.relative { current.saturating_add(self.value) } else { self.value }
    }
}

/// The contents of a pair of brackets (manual chapter 1, "X-Y Coordinate
/// System").
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct Extent {
    pub(crate) x: Option<Axis>,
    pub(crate) y: Option<Axis>,
    /// More than two values were written (error code 3, manual chapter 10).
    pub(crate) extra: bool,
}

impl Extent {
    /// Parses what stood between the brackets.
    pub(crate) fn parse(text: &[u8]) -> Self {
        let mut parts = text.split(|byte| *byte == b',');
        let mut axis = || {
            // A stray character inside the brackets is ignored, as the
            // terminal ignores it (error code 1, manual chapter 10): a
            // VT340 draws `V[+799',]` as `V[+799,]`.
            let digits: Vec<u8> = parts.next()?.iter().copied().filter(|byte| is_numeric(*byte)).collect();
            let number = number::parse(&digits)?;
            Some(Axis { value: number.value(), relative: number.signed })
        };
        let x = axis();
        let y = axis();
        Self { x, y, extra: parts.next().is_some() }
    }

    /// `[]`: no coordinate at all.
    pub(crate) const fn is_null(&self) -> bool {
        self.x.is_none() && self.y.is_none()
    }
}

/// The characters a number is written with.
const fn is_numeric(byte: u8) -> bool {
    matches!(byte, b'0'..=b'9' | b'+' | b'-' | b'.' | b'E' | b'e')
}

#[cfg(test)]
mod tests {
    use super::{Axis, Extent};

    fn axis(value: i32, relative: bool) -> Axis {
        Axis { value, relative }
    }

    #[test]
    fn every_form_in_the_manual_parses() {
        assert_eq!(
            Extent::parse(b"10,86"),
            Extent { x: Some(axis(10, false)), y: Some(axis(86, false)), extra: false }
        );
        assert_eq!(Extent::parse(b"52"), Extent { x: Some(axis(52, false)), y: None, extra: false });
        assert_eq!(Extent::parse(b",121"), Extent { x: None, y: Some(axis(121, false)), extra: false });
        assert_eq!(
            Extent::parse(b"+15,-10"),
            Extent { x: Some(axis(15, true)), y: Some(axis(-10, true)), extra: false }
        );
        assert_eq!(
            Extent::parse(b".1E3,1000E-1"),
            Extent { x: Some(axis(100, false)), y: Some(axis(100, false)), extra: false }
        );
    }

    #[test]
    fn empty_brackets_are_null_and_extra_values_are_flagged() {
        assert!(Extent::parse(b"").is_null());
        assert!(Extent::parse(b" , ").is_null());
        assert!(Extent::parse(b"1,2,3").extra);
    }

    #[test]
    fn stray_characters_inside_the_brackets_are_ignored() {
        assert_eq!(Extent::parse(b"+799',"), Extent { x: Some(axis(799, true)), y: None, extra: false });
        assert_eq!(
            Extent::parse(b" 1 0 , x2"),
            Extent { x: Some(axis(10, false)), y: Some(axis(2, false)), extra: false }
        );
    }
}
