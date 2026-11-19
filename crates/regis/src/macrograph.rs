//! Macrographs: stored command strings named by a letter (manual chapter 9).

use alloc::vec::Vec;

use crate::limits::{MACROGRAPHS, MAX_MACROGRAPH_BYTES};

/// The twenty-six macrographs.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(crate) struct Macrographs {
    bodies: [Vec<u8>; MACROGRAPHS],
}

/// The slot a letter names, in either case.
pub(crate) fn slot(letter: u8) -> Option<usize> {
    letter.is_ascii_alphabetic().then(|| usize::from(letter.to_ascii_uppercase().saturating_sub(b'A')))
}

impl Macrographs {
    /// The memory the bodies hold.
    pub(crate) fn heap_bytes(&self) -> usize {
        self.bodies.iter().map(crate::limits::vector_bytes).fold(0, usize::saturating_add)
    }

    /// The stored bytes of a macrograph; empty if never defined.
    pub(crate) fn body(&self, slot: usize) -> &[u8] {
        self.bodies.get(slot).map_or(&[], Vec::as_slice)
    }

    /// Bytes stored in all.
    pub(crate) fn used(&self) -> usize {
        self.bodies.iter().map(Vec::len).fold(0, usize::saturating_add)
    }

    /// Bytes still free.
    pub(crate) fn free(&self) -> usize {
        MAX_MACROGRAPH_BYTES.saturating_sub(self.used())
    }

    /// Replaces a macrograph. A definition that does not fit in the space
    /// left is not stored, and the old one is cleared: false then.
    pub(crate) fn define(&mut self, slot: usize, body: Vec<u8>) -> bool {
        let Some(stored) = self.bodies.get_mut(slot) else { return false };
        stored.clear();
        let fits = body.len() <= self.free();
        if fits && let Some(stored) = self.bodies.get_mut(slot) {
            *stored = body;
        }
        fits
    }

    /// `@.`: clears every macrograph.
    pub(crate) fn clear(&mut self) {
        self.bodies.iter_mut().for_each(Vec::clear);
    }
}

#[cfg(test)]
mod tests {
    use alloc::vec;

    use super::{Macrographs, slot};
    use crate::limits::MAX_MACROGRAPH_BYTES;

    #[test]
    fn letters_name_slots_in_either_case() {
        assert_eq!(slot(b'a'), Some(0));
        assert_eq!(slot(b'Z'), Some(25));
        assert_eq!(slot(b'1'), None);
    }

    #[test]
    fn storage_is_bounded_in_total() {
        let mut macros = Macrographs::default();
        assert!(macros.define(0, vec![b'x'; MAX_MACROGRAPH_BYTES]));
        assert!(!macros.define(1, vec![b'y'; 1]));
        assert_eq!(macros.body(1), b"");
        // Redefining frees the old body first.
        assert!(macros.define(0, vec![b'z'; 10]));
        assert_eq!(macros.free(), MAX_MACROGRAPH_BYTES - 10);
        macros.clear();
        assert_eq!(macros.used(), 0);
    }
}
