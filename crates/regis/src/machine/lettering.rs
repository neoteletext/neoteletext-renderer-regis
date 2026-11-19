//! The text state and the character sets.

use alloc::vec::Vec;

use super::load::LoadingCell;
use crate::font::{Font, Glyph, Loadable};
use crate::geometry::Point;
use crate::limits::{FIRST_PROVIDED_ALPHABET, PROVIDED_SETS};
use crate::provided::{JoinedForm, ProvidedSet};
use crate::text::TextControls;

/// How many character sets a stream can load (manual chapter 8).
pub(crate) const LOADABLE_SETS: usize = 3;
/// Loadable sets hold seven-bit characters only (manual chapter 7).
const SEVEN_BITS: u8 = 0x7F;

/// The state of the text command.
#[derive(Debug, Default)]
pub(crate) struct Text {
    pub(crate) controls: TextControls,
    /// Controls saved by `T(B)`.
    pub(crate) saved: Vec<TextControls>,
    /// Where a carriage return goes back to.
    pub(crate) line_start: Point,
    /// A tilt was already given in this group of text options.
    pub(crate) tilted: bool,
}

// Saved text controls and positions are entries of small stacks the memory
// bound counts.
const _: () = assert!(size_of::<TextControls>() <= crate::limits::SMALL_STACK_ENTRY_BYTES);
const _: () = assert!(size_of::<Option<Point>>() <= crate::limits::SMALL_STACK_ENTRY_BYTES);

/// The provided sets the caller gave, by number; one never given is `None`
/// and draws empty cells (renderer-parity.md PAR-12).
pub(crate) type ProvidedSets = Vec<Option<ProvidedSet>>;

/// The set number of a provided alphabet, 100 to 154; `None` for any other.
pub(crate) fn provided_number(alphabet: usize) -> Option<usize> {
    let number = alphabet.checked_sub(usize::from(FIRST_PROVIDED_ALPHABET))?;
    (number < usize::from(PROVIDED_SETS)).then_some(number)
}

/// The character shapes: the built-in set, the three loadable ones and the
/// provided ones.
#[derive(Debug)]
pub(crate) struct Characters {
    pub(crate) font: Font,
    pub(crate) sets: [Loadable; LOADABLE_SETS],
    pub(crate) provided: ProvidedSets,
    /// The set the load command works on: 0 to 3.
    pub(crate) loading: usize,
    /// The cell being loaded.
    pub(crate) cell: Option<LoadingCell>,
}

impl Default for Characters {
    /// Set 1 is selected for loading at power-up (manual table 1-2).
    fn default() -> Self {
        Self { font: Font::default(), sets: Default::default(), provided: Vec::new(), loading: 1, cell: None }
    }
}

impl Characters {
    /// The provided set of an alphabet, if it is one and was given.
    pub(crate) fn provided_set(&self, alphabet: usize) -> Option<&ProvidedSet> {
        self.provided.get(provided_number(alphabet)?)?.as_ref()
    }

    /// Gives a provided set by number, 0 to 54.
    /// The memory the font and the provided sets the caller gave hold
    /// beyond this structure.
    pub(crate) fn given_heap_bytes(&self) -> usize {
        let slots =
            crate::limits::held_allocation(self.provided.capacity().saturating_mul(size_of::<Option<ProvidedSet>>()));
        let sets = self.provided.iter().flatten().map(ProvidedSet::heap_bytes).fold(0, usize::saturating_add);
        self.font.heap_bytes().saturating_add(slots).saturating_add(sets)
    }

    pub(crate) fn set_provided(&mut self, number: usize, set: ProvidedSet) {
        if number >= usize::from(PROVIDED_SETS) {
            return;
        }
        if self.provided.len() <= number {
            self.provided.resize_with(number.saturating_add(1), || None);
        }
        if let Some(slot) = self.provided.get_mut(number) {
            *slot = Some(set);
        }
    }

    /// The shape of a character in a set (0 built in, 1 to 3 loadable, 100
    /// to 154 provided) at a unit cell height, in a joined form if one is
    /// named: the solid block if a built-in or loadable set does not have
    /// it, the empty cell if a provided one does not (PAR-12).
    pub(crate) fn glyph(&self, alphabet: usize, code: u8, form: Option<JoinedForm>, unit_height: i32) -> Glyph {
        if provided_number(alphabet).is_some() {
            return self.provided_set(alphabet).map_or(Glyph::EMPTY, |set| set.shape(code, form));
        }
        let found = match alphabet.checked_sub(1) {
            None => self.font.glyph(code, unit_height),
            Some(set) if code <= SEVEN_BITS => self.sets.get(set).and_then(|loadable| loadable.glyph(code)),
            Some(_) => None,
        };
        found.unwrap_or(Glyph::BLOCK)
    }

    /// The set being loaded; set 0 is built in and "cannot be loaded".
    pub(crate) fn loading_set(&mut self) -> Option<&mut Loadable> {
        self.sets.get_mut(self.loading.checked_sub(1)?)
    }

    /// The name of the set selected for loading, for the report command.
    pub(crate) fn loading_name(&self) -> &[u8] {
        let set = self.loading.checked_sub(1).and_then(|index| self.sets.get(index));
        set.map_or(&[], Loadable::name)
    }
}
