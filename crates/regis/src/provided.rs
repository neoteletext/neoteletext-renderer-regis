//! NeoTeletext's provided character sets, selected as alphabets 100 to 154
//! (`docs/regis-v2.md`, "Provided character sets"; renderer-parity.md
//! PAR-12 and PAR-13; DEC-6669 item 4).
//!
//! The library compiles no font (DEC-6346): the caller makes each set from
//! the project's own Noto Teletext glyphs and passes it in with
//! [`Terminal::set_provided_set`](crate::Terminal::set_provided_set).

mod joining;

use alloc::vec;
use alloc::vec::Vec;

use crate::Error;
use crate::font::Glyph;
use crate::limits::{MAX_PROVIDED_GLYPH_HEIGHT, MAX_PROVIDED_GLYPH_WIDTH, PROVIDED_SET_POSITIONS, held_allocation};
pub(crate) use joining::forms;

/// The first code of a set: 2/0.
const FIRST_CODE: u8 = 0x20;
/// The last code of a set: 7/15.
const LAST_CODE: u8 = 0x7F;
const _: () = assert!(PROVIDED_SET_POSITIONS == (LAST_CODE - FIRST_CODE + 1) as usize);

/// How a letter of a plain Arabic-script set joins its neighbours: the
/// classes of Unicode's `ArabicShaping.txt` as the teletext renderer uses
/// them (REQ-CHR-015). The engine's join-causing tatweel is given as
/// [`JoiningType::Dual`], which is how it joins.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum JoiningType {
    /// Joins neither neighbour: a space, a digit, a hamza.
    #[default]
    None,
    /// Joins only the letter before it in reading order, on its right.
    Right,
    /// Joins the letters on both sides.
    Dual,
    /// A mark: takes no part in joining and keeps its own glyph.
    Transparent,
}

impl JoiningType {
    /// Can join the letter before it in reading order (on its right).
    pub(crate) const fn joins_previous(self) -> bool {
        matches!(self, Self::Dual | Self::Right)
    }

    /// Can join the letter after it in reading order (on its left).
    pub(crate) const fn joins_next(self) -> bool {
        matches!(self, Self::Dual)
    }
}

/// A form of a joined letter other than its isolated one, which is the
/// set's own glyph (REQ-CHR-015).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum JoinedForm {
    /// Joined to the letter before it only.
    Final,
    /// Joined to the letter after it only.
    Initial,
    /// Joined on both sides.
    Medial,
}

impl JoinedForm {
    const COUNT: usize = 3;

    const fn index(self) -> usize {
        match self {
            Self::Final => 0,
            Self::Initial => 1,
            Self::Medial => 2,
        }
    }
}

/// The joining data of a set: each position's type and its three further
/// glyphs.
#[derive(Clone, Debug, PartialEq, Eq)]
struct Joining {
    kinds: [JoiningType; PROVIDED_SET_POSITIONS],
    /// `height` rows for each form of each code: code-major.
    rows: Vec<u16>,
    given: [[bool; JoinedForm::COUNT]; PROVIDED_SET_POSITIONS],
}

/// One provided character set: a glyph for each of the 96 positions 2/0 to
/// 7/15, each `width` pixels wide and `height` high, and for a plain
/// Arabic-script set (group 1001, sets 44 to 51) its joining data.
///
/// A glyph is its rows from the top, each a bit field whose most
/// significant of `width` bits is the leftmost pixel, as the engine's glyph
/// table and BDF files hold them. A code never given a glyph is blank. A set
/// is joined once any position is given a joining type.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProvidedSet {
    width: u8,
    height: u8,
    /// `height` rows for each code in turn.
    rows: Vec<u16>,
    joining: Option<Joining>,
}

impl ProvidedSet {
    /// The memory its glyphs and joining data hold beyond its structure.
    pub(crate) fn heap_bytes(&self) -> usize {
        let rows = |rows: &Vec<u16>| held_allocation(rows.capacity().saturating_mul(size_of::<u16>()));
        rows(&self.rows).saturating_add(self.joining.as_ref().map_or(0, |joining| rows(&joining.rows)))
    }

    /// A set of blank glyphs.
    ///
    /// # Errors
    ///
    /// [`Error::ProvidedSet`] when `width` is not 1 to
    /// [`MAX_PROVIDED_GLYPH_WIDTH`] or `height` not 1 to
    /// [`MAX_PROVIDED_GLYPH_HEIGHT`].
    pub fn new(width: u8, height: u8) -> Result<Self, Error> {
        if !(1..=MAX_PROVIDED_GLYPH_WIDTH).contains(&width) || !(1..=MAX_PROVIDED_GLYPH_HEIGHT).contains(&height) {
            return Err(Error::ProvidedSet);
        }
        Ok(Self {
            width,
            height,
            rows: vec![0; PROVIDED_SET_POSITIONS.saturating_mul(usize::from(height))],
            joining: None,
        })
    }

    /// Gives `code` (2/0 to 7/15) its glyph: `height` rows from the top.
    ///
    /// # Errors
    ///
    /// [`Error::ProvidedSet`] when the code is outside the set, the rows are
    /// not `height`, or a row has a bit set beyond the glyph's width; the
    /// set is unchanged then.
    pub fn set_glyph(&mut self, code: u8, rows: &[u16]) -> Result<(), Error> {
        let start = self.start(code).ok_or(Error::ProvidedSet)?;
        self.check(rows)?;
        let end = start.checked_add(rows.len()).ok_or(Error::ProvidedSet)?;
        self.rows.get_mut(start..end).ok_or(Error::ProvidedSet)?.copy_from_slice(rows);
        Ok(())
    }

    /// The glyph of `code`, its rows from the top; `None` outside 2/0 to
    /// 7/15.
    #[must_use]
    pub fn glyph(&self, code: u8) -> Option<&[u16]> {
        let start = self.start(code)?;
        self.rows.get(start..start.checked_add(usize::from(self.height))?)
    }

    /// Gives `code` its joining type, and so makes the set a joined one
    /// (PAR-13). Positions never given one are [`JoiningType::None`].
    ///
    /// # Errors
    ///
    /// [`Error::ProvidedSet`] when the code is outside the set.
    pub fn set_joining(&mut self, code: u8, kind: JoiningType) -> Result<(), Error> {
        let index = Self::index(code).ok_or(Error::ProvidedSet)?;
        let slot = self.joining_mut().kinds.get_mut(index).ok_or(Error::ProvidedSet)?;
        *slot = kind;
        Ok(())
    }

    /// Gives `code` the glyph of one of its joined forms, as
    /// [`ProvidedSet::set_glyph`] gives its isolated one. A letter whose
    /// form is not given draws its isolated glyph in that form.
    ///
    /// # Errors
    ///
    /// As [`ProvidedSet::set_glyph`]'s.
    pub fn set_form(&mut self, code: u8, form: JoinedForm, rows: &[u16]) -> Result<(), Error> {
        let index = Self::index(code).ok_or(Error::ProvidedSet)?;
        self.check(rows)?;
        let start = self.form_start(index, form).ok_or(Error::ProvidedSet)?;
        let end = start.checked_add(rows.len()).ok_or(Error::ProvidedSet)?;
        let joining = self.joining_mut();
        joining.rows.get_mut(start..end).ok_or(Error::ProvidedSet)?.copy_from_slice(rows);
        if let Some(given) = joining.given.get_mut(index).and_then(|given| given.get_mut(form.index())) {
            *given = true;
        }
        Ok(())
    }

    /// The joining type of `code`; `None` when the set is not joined or the
    /// code is outside it.
    #[must_use]
    pub fn joining(&self, code: u8) -> Option<JoiningType> {
        self.joining.as_ref()?.kinds.get(Self::index(code)?).copied()
    }

    /// The glyph of one joined form of `code`, if it was given.
    #[must_use]
    pub fn form(&self, code: u8, form: JoinedForm) -> Option<&[u16]> {
        let index = Self::index(code)?;
        let joining = self.joining.as_ref()?;
        if !*joining.given.get(index)?.get(form.index())? {
            return None;
        }
        let start = self.form_start(index, form)?;
        joining.rows.get(start..start.checked_add(usize::from(self.height))?)
    }

    /// Whether the set was given joining data.
    #[must_use]
    pub const fn is_joined(&self) -> bool {
        self.joining.is_some()
    }

    /// The glyphs' width in pixels.
    #[must_use]
    pub const fn width(&self) -> u8 {
        self.width
    }

    /// The glyphs' height in pixels.
    #[must_use]
    pub const fn height(&self) -> u8 {
        self.height
    }

    /// The shape a code draws in a form: a code outside 2/0 to 7/14 draws
    /// the empty cell (PAR-12: 7/15 is not reachable from a string).
    pub(crate) fn shape(&self, code: u8, form: Option<JoinedForm>) -> Glyph {
        if code >= LAST_CODE {
            return Glyph::EMPTY;
        }
        let rows = form.and_then(|form| self.form(code, form)).or_else(|| self.glyph(code));
        rows.map_or(Glyph::EMPTY, |rows| Glyph::from_wide_rows(rows, self.width))
    }

    fn check(&self, rows: &[u16]) -> Result<(), Error> {
        let outside = u16::MAX.checked_shl(u32::from(self.width)).unwrap_or(0);
        if rows.len() != usize::from(self.height) || rows.iter().any(|row| row & outside != 0) {
            return Err(Error::ProvidedSet);
        }
        Ok(())
    }

    fn joining_mut(&mut self) -> &mut Joining {
        let length = PROVIDED_SET_POSITIONS.saturating_mul(JoinedForm::COUNT).saturating_mul(usize::from(self.height));
        self.joining.get_or_insert_with(|| Joining {
            kinds: [JoiningType::None; PROVIDED_SET_POSITIONS],
            rows: vec![0; length],
            given: [[false; JoinedForm::COUNT]; PROVIDED_SET_POSITIONS],
        })
    }

    fn index(code: u8) -> Option<usize> {
        if !(FIRST_CODE..=LAST_CODE).contains(&code) {
            return None;
        }
        Some(usize::from(code.checked_sub(FIRST_CODE)?))
    }

    fn start(&self, code: u8) -> Option<usize> {
        Self::index(code)?.checked_mul(usize::from(self.height))
    }

    fn form_start(&self, index: usize, form: JoinedForm) -> Option<usize> {
        index.checked_mul(JoinedForm::COUNT)?.checked_add(form.index())?.checked_mul(usize::from(self.height))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn glyphs_are_bounded() {
        assert_eq!(ProvidedSet::new(0, 20), Err(Error::ProvidedSet));
        assert_eq!(ProvidedSet::new(MAX_PROVIDED_GLYPH_WIDTH + 1, 20), Err(Error::ProvidedSet));
        assert_eq!(ProvidedSet::new(12, MAX_PROVIDED_GLYPH_HEIGHT + 1), Err(Error::ProvidedSet));
        assert_eq!(ProvidedSet::new(12, 0), Err(Error::ProvidedSet));
        let mut set = ProvidedSet::new(12, 2).unwrap();
        assert_eq!(set.set_glyph(0x1F, &[0, 0]), Err(Error::ProvidedSet));
        assert_eq!(set.set_glyph(0x80, &[0, 0]), Err(Error::ProvidedSet));
        assert_eq!(set.set_glyph(0x41, &[0]), Err(Error::ProvidedSet));
        assert_eq!(set.set_glyph(0x41, &[0x1000, 0]), Err(Error::ProvidedSet), "a bit beyond 12 columns");
        set.set_glyph(0x7F, &[0x800, 0x001]).unwrap();
        assert_eq!(set.glyph(0x7F), Some(&[0x800, 0x001][..]));
        assert_eq!(set.glyph(0x20), Some(&[0, 0][..]));
        assert!(!set.is_joined());
        assert_eq!(set.joining(0x41), None);
    }

    #[test]
    fn joining_data_is_kept_per_position() {
        let mut set = ProvidedSet::new(4, 1).unwrap();
        set.set_glyph(0x41, &[0b1000]).unwrap();
        assert_eq!(set.set_joining(0x80, JoiningType::Dual), Err(Error::ProvidedSet));
        set.set_joining(0x41, JoiningType::Dual).unwrap();
        assert!(set.is_joined());
        assert_eq!(set.joining(0x41), Some(JoiningType::Dual));
        assert_eq!(set.joining(0x42), Some(JoiningType::None));
        assert_eq!(set.set_form(0x41, JoinedForm::Final, &[0b1_0000]), Err(Error::ProvidedSet));
        set.set_form(0x41, JoinedForm::Medial, &[0b0110]).unwrap();
        assert_eq!(set.form(0x41, JoinedForm::Medial), Some(&[0b0110][..]));
        assert_eq!(set.form(0x41, JoinedForm::Final), None);
        // A form not given draws the isolated glyph.
        assert_eq!(set.shape(0x41, Some(JoinedForm::Final)), Glyph::from_wide_rows(&[0b1000], 4));
        assert_eq!(set.shape(0x41, Some(JoinedForm::Medial)), Glyph::from_wide_rows(&[0b0110], 4));
        assert_eq!(set.shape(0x7F, None), Glyph::EMPTY);
    }
}
