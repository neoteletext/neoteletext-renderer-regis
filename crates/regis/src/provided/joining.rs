//! Joining the letters of a plain Arabic-script set inside one string
//! (renderer-parity.md PAR-13; the teletext renderer's REQ-CHR-015).
//!
//! A string is in visual order: the letter that follows a letter in reading
//! is the one on its left, earlier in the string, and the one before it in
//! reading is on its right, later in the string. No allocation; each letter
//! looks past the marks beside it only, so a string costs time in
//! proportion to its length.

use super::{JoinedForm, JoiningType, ProvidedSet};

/// The joining type of the nearest code that is not a mark, walking from
/// a letter through `codes`; `None` at a text control character or the
/// string's end, past which nothing joins.
fn nearest<'a>(set: &ProvidedSet, codes: impl Iterator<Item = &'a u8>, stops: impl Fn(u8) -> bool) -> JoiningType {
    for &code in codes {
        if stops(code) {
            return JoiningType::None;
        }
        match set.joining(code).unwrap_or(JoiningType::None) {
            JoiningType::Transparent => {}
            kind => return kind,
        }
    }
    JoiningType::None
}

/// The form the code at `index` of `text` takes in a joined set; `None` for
/// the isolated form, for a mark and in a set that is not joined. `stops`
/// names the text control characters, at which joining ends.
pub(crate) fn forms(set: &ProvidedSet, text: &[u8], index: usize, stops: impl Fn(u8) -> bool) -> Option<JoinedForm> {
    let kind = set.joining(*text.get(index)?)?;
    if kind == JoiningType::Transparent {
        return None;
    }
    let left = text.get(..index).unwrap_or(&[]);
    let right = text.get(index.saturating_add(1)..).unwrap_or(&[]);
    let next = nearest(set, left.iter().rev(), &stops);
    let previous = nearest(set, right.iter(), &stops);
    // PAR-13: leftwards when it is dual-joining and the nearest letter on its
    // left joins the letter before it; rightwards when it joins the letter
    // before it and the nearest letter on its right is dual-joining.
    let joins_next = kind.joins_next() && next.joins_previous();
    let joins_previous = kind.joins_previous() && previous.joins_next();
    match (joins_previous, joins_next) {
        (false, false) => None,
        (true, false) => Some(JoinedForm::Final),
        (false, true) => Some(JoinedForm::Initial),
        (true, true) => Some(JoinedForm::Medial),
    }
}

#[cfg(test)]
mod tests {
    use alloc::vec::Vec;

    use super::*;

    const DUAL: u8 = b'B';
    const RIGHT: u8 = b'A';
    const MARK: u8 = b'm';
    const NONE: u8 = b' ';

    fn set() -> ProvidedSet {
        let mut set = ProvidedSet::new(1, 1).unwrap();
        set.set_joining(DUAL, JoiningType::Dual).unwrap();
        set.set_joining(RIGHT, JoiningType::Right).unwrap();
        set.set_joining(MARK, JoiningType::Transparent).unwrap();
        set
    }

    fn all(text: &[u8]) -> Vec<Option<JoinedForm>> {
        let set = set();
        (0..text.len()).map(|index| forms(&set, text, index, |code| code == b'\r')).collect()
    }

    #[test]
    fn letters_take_their_forms_from_their_neighbours_in_visual_order() {
        use JoinedForm::{Final, Initial, Medial};
        // Read right to left: B B B is initial, medial, final.
        assert_eq!(all(b"BBB"), [Some(Final), Some(Medial), Some(Initial)]);
        // A right-joining letter ends the word: in "BA" (reading A then B) A
        // cannot join B, which follows it on its left.
        assert_eq!(all(b"BA"), [None, None]);
        assert_eq!(all(b"AB"), [Some(Final), Some(Initial)]);
        assert_eq!(all(b"B B"), [None, None, None]);
        assert_eq!(all(&[DUAL, NONE, RIGHT]), [None, None, None]);
    }

    #[test]
    fn marks_are_passed_over_and_keep_their_own_glyph() {
        use JoinedForm::{Final, Initial};
        assert_eq!(all(b"BmmB"), [Some(Final), None, None, Some(Initial)]);
    }

    #[test]
    fn a_text_control_character_ends_joining() {
        assert_eq!(all(b"B\rB"), [None, None, None]);
    }

    #[test]
    fn a_set_without_joining_data_joins_nothing() {
        let plain = ProvidedSet::new(1, 1).unwrap();
        assert_eq!(forms(&plain, b"BB", 0, |_| false), None);
    }
}
