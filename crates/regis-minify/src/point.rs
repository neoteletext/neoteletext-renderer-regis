//! Coordinates inside brackets: reading them and writing them short.

/// The largest coordinate rewritten; beyond it the interpreter clamps and
/// the arithmetic here would not match.
const MAX_COORDINATE: i32 = 32_767;
/// A coordinate with more digits than this is left alone.
const MAX_DIGITS: usize = 6;

/// One coordinate of a bracketed position.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Part {
    /// Left out: unchanged.
    Same,
    Absolute(i32),
    /// Signed: measured from the present position.
    Relative(i32),
}

impl Part {
    fn parse(text: &[u8]) -> Option<Self> {
        let (signed, digits) = match text.split_first() {
            None => return Some(Self::Same),
            Some((b'+' | b'-', digits)) => (true, digits),
            Some(_) => (false, text),
        };
        if digits.is_empty() || digits.len() > MAX_DIGITS || !digits.iter().all(u8::is_ascii_digit) {
            return None;
        }
        let magnitude = digits
            .iter()
            .fold(0_i32, |sum, digit| sum.saturating_mul(10).saturating_add(i32::from(digit.saturating_sub(b'0'))));
        if magnitude > MAX_COORDINATE {
            return None;
        }
        Some(if !signed {
            Self::Absolute(magnitude)
        } else if magnitude == 0 {
            // A step of nothing is the same as leaving the coordinate out.
            Self::Same
        } else if text.first() == Some(&b'-') {
            Self::Relative(magnitude.saturating_neg())
        } else {
            Self::Relative(magnitude)
        })
    }

    /// Where this coordinate leads from `from`, if that can be known.
    fn resolve(self, from: Option<i32>) -> Option<i32> {
        match self {
            Self::Same => from,
            Self::Absolute(value) => Some(value),
            Self::Relative(step) => from.and_then(|base| base.checked_add(step)),
        }
        .filter(|value| value.abs() <= MAX_COORDINATE)
    }
}

/// A bracketed position: its two coordinates.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Target {
    pub(crate) x: Part,
    pub(crate) y: Part,
}

/// A position on the screen, each coordinate known or not.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub(crate) struct Position {
    pub(crate) x: Option<i32>,
    pub(crate) y: Option<i32>,
}

impl Position {
    pub(crate) const UNKNOWN: Self = Self { x: None, y: None };
}

impl Target {
    /// Reads `x,y`, `x`, `,y` or nothing. None for anything else, such as
    /// a decimal number, which is then left exactly as written.
    pub(crate) fn parse(inside: &[u8]) -> Option<Self> {
        let mut parts = inside.split(|byte| *byte == b',');
        let x = Part::parse(parts.next().unwrap_or_default())?;
        let y = Part::parse(parts.next().unwrap_or_default())?;
        parts.next().is_none().then_some(Self { x, y })
    }

    /// Where this leads from `from`.
    pub(crate) fn resolve(self, from: Position) -> Position {
        Position { x: self.x.resolve(from.x), y: self.y.resolve(from.y) }
    }

    /// The shortest writing of the move from `from` to `to`, inside the
    /// brackets. A coordinate not known on both sides keeps its form.
    pub(crate) fn shortest(self, from: Position, to: Position) -> Vec<u8> {
        let x = shortest_part(self.x, from.x, to.x);
        let y = shortest_part(self.y, from.y, to.y);
        let mut out = x;
        if !y.is_empty() {
            out.push(b',');
            out.extend_from_slice(&y);
        }
        out
    }
}

fn written(part: Part) -> Vec<u8> {
    match part {
        Part::Same => Vec::new(),
        Part::Absolute(value) => value.to_string().into_bytes(),
        Part::Relative(step) => format!("{step:+}").into_bytes(),
    }
}

fn shortest_part(part: Part, from: Option<i32>, to: Option<i32>) -> Vec<u8> {
    let (Some(from), Some(to)) = (from, to) else { return written(part) };
    if from == to {
        return Vec::new();
    }
    let relative = to.checked_sub(from).map(|step| written(Part::Relative(step)));
    // A signed number is always relative, so only a position that is not
    // negative can be written absolute.
    let absolute = (to >= 0).then(|| written(Part::Absolute(to)));
    match (absolute, relative) {
        (Some(absolute), Some(relative)) if relative.len() < absolute.len() => relative,
        (Some(absolute), _) => absolute,
        (None, Some(relative)) => relative,
        (None, None) => written(part),
    }
}

#[cfg(test)]
mod tests {
    use super::{Part, Position, Target};

    fn at(x: i32, y: i32) -> Position {
        Position { x: Some(x), y: Some(y) }
    }

    fn rewrite(inside: &str, from: Position) -> String {
        let target = Target::parse(inside.as_bytes()).unwrap();
        String::from_utf8(target.shortest(from, target.resolve(from))).unwrap()
    }

    #[test]
    fn coordinates_are_absolute_relative_or_left_out() {
        assert_eq!(Target::parse(b"12,+3").unwrap(), Target { x: Part::Absolute(12), y: Part::Relative(3) });
        assert_eq!(Target::parse(b",-4").unwrap(), Target { x: Part::Same, y: Part::Relative(-4) });
        assert_eq!(Target::parse(b"").unwrap(), Target { x: Part::Same, y: Part::Same });
        assert!(Target::parse(b"1.5,2").is_none());
        assert!(Target::parse(b"1,2,3").is_none());
        assert!(Target::parse(b"99999,1").is_none());
    }

    #[test]
    fn the_shorter_of_absolute_and_relative_is_written() {
        assert_eq!(rewrite("400,300", at(398, 300)), "+2");
        assert_eq!(rewrite("+2,+0", at(5, 7)), "7");
        assert_eq!(rewrite("120,+0", at(118, 300)), "+2");
        assert_eq!(rewrite("5,255", at(400, 250)), "5,+5");
        // When both forms are as long, the absolute one is kept.
        assert_eq!(rewrite("5,300", at(400, 250)), "5,300");
        assert_eq!(rewrite("400,300", at(400, 300)), "");
    }

    #[test]
    fn a_negative_position_is_only_ever_written_relative() {
        assert_eq!(rewrite("-10,+0", at(4, 9)), "-10");
        assert_eq!(rewrite("-3", at(100, 9)), "97");
    }

    #[test]
    fn an_unknown_position_keeps_what_was_written() {
        assert_eq!(rewrite("+5,20", Position::UNKNOWN), "+5,20");
        assert_eq!(rewrite("+0,+3", Position::UNKNOWN), ",+3");
    }
}
