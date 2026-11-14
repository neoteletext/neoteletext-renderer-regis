//! Finding the ReGIS inside a terminal data stream.

const ESCAPE: u8 = 0x1B;

/// One stretch of a data stream.
#[derive(Debug, PartialEq, Eq)]
pub(crate) enum Part<'a> {
    /// Bytes that are not ReGIS commands: copied as they are.
    Other(&'a [u8]),
    /// The commands of one ReGIS device control string.
    Regis(&'a [u8]),
}

/// The length of a ReGIS introducer at the front: `ESC P`, parameters,
/// then `p`.
fn introducer(bytes: &[u8]) -> Option<usize> {
    let rest = bytes.strip_prefix(&[ESCAPE, b'P'])?;
    let parameters = rest.iter().take_while(|byte| byte.is_ascii_digit() || **byte == b';').count();
    (rest.get(parameters) == Some(&b'p')).then(|| parameters.saturating_add(3))
}

/// Splits a stream. A stream with no device control string at all is
/// taken to be bare ReGIS commands.
pub(crate) fn split(input: &[u8]) -> Vec<Part<'_>> {
    if !input.windows(2).any(|pair| pair == [ESCAPE, b'P']) {
        return vec![Part::Regis(input)];
    }
    let mut parts = Vec::new();
    let mut rest = input;
    while !rest.is_empty() {
        let found = (0..rest.len()).find_map(|at| Some((at, introducer(rest.get(at..)?)?)));
        let Some((at, length)) = found else {
            parts.push(Part::Other(rest));
            break;
        };
        let (before, from) = rest.split_at(at.saturating_add(length).min(rest.len()));
        parts.push(Part::Other(before));
        // The commands run to the next escape, which ends the string.
        let end = from.iter().position(|byte| *byte == ESCAPE).unwrap_or(from.len());
        let (commands, after) = from.split_at(end);
        parts.push(Part::Regis(commands));
        rest = after;
    }
    parts
}

#[cfg(test)]
mod tests {
    use super::{Part, split};

    #[test]
    fn bare_commands_are_all_regis() {
        assert_eq!(split(b"P[1,2]"), [Part::Regis(b"P[1,2]")]);
    }

    #[test]
    fn only_the_inside_of_a_regis_string_is_regis() {
        assert_eq!(
            split(b"hi\x1bP0pP[1]\x1b\\bye\x1bP1;2pV[2]"),
            [Part::Other(b"hi\x1bP0p"), Part::Regis(b"P[1]"), Part::Other(b"\x1b\\bye\x1bP1;2p"), Part::Regis(b"V[2]"),]
        );
    }

    #[test]
    fn another_kind_of_control_string_is_left_alone() {
        assert_eq!(split(b"\x1bPq#1~~\x1b\\"), [Part::Other(b"\x1bPq#1~~\x1b\\")]);
    }
}
