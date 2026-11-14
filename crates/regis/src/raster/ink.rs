//! What one written pixel becomes: the writing style, the negative flag,
//! the colours and the plane mask together.

use crate::write::{Style, WriteControls};

/// The pixel-level part of the write controls.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Ink {
    style: Style,
    negative: bool,
    foreground: u8,
    background: u8,
    planes: u8,
}

impl Ink {
    pub(crate) const fn new(controls: &WriteControls, background: u8) -> Self {
        Self {
            style: controls.style,
            negative: controls.negative,
            foreground: controls.foreground,
            background,
            planes: controls.planes,
        }
    }

    /// The new value of a pixel now holding `old`, where the pattern (or
    /// character shape) is `on`; `None` leaves the pixel alone.
    ///
    /// Manual chapter 3: overlay writes only the 1-bits; replace writes
    /// both; complement inverts under the 1-bits and ignores the foreground;
    /// erase ignores the pattern and writes the background, or with negative
    /// writing the foreground. Only the selected planes change.
    pub(crate) const fn apply(self, old: u8, on: bool) -> Option<u8> {
        let on = on != self.negative;
        let value = match (self.style, on) {
            (Style::Overlay | Style::Complement, false) => return None,
            (Style::Overlay | Style::Replace, true) => self.foreground,
            (Style::Complement, true) => !old,
            // Erase writes the background, or with negative writing the
            // foreground, whatever the pattern says.
            (Style::Erase, _) if self.negative => self.foreground,
            (Style::Replace, false) | (Style::Erase, _) => self.background,
        };
        Some((old & !self.planes) | (value & self.planes))
    }

    /// Whether a pixel whose pattern bit is off could change at all, so
    /// callers can skip blank areas.
    pub(crate) const fn writes_off_pixels(self) -> bool {
        self.apply(0, false).is_some()
    }
}

#[cfg(test)]
mod tests {
    use super::Ink;
    use crate::write::{Style, WriteControls};

    fn ink(style: Style, negative: bool, planes: u8) -> Ink {
        let controls = WriteControls { style, negative, planes, foreground: 5, ..WriteControls::default() };
        Ink::new(&controls, 2)
    }

    #[test]
    fn overlay_writes_only_where_the_pattern_is_on() {
        assert_eq!(ink(Style::Overlay, false, 15).apply(9, true), Some(5));
        assert_eq!(ink(Style::Overlay, false, 15).apply(9, false), None);
        assert_eq!(ink(Style::Overlay, true, 15).apply(9, false), Some(5));
    }

    #[test]
    fn replace_writes_the_background_where_the_pattern_is_off() {
        assert_eq!(ink(Style::Replace, false, 15).apply(9, false), Some(2));
        assert_eq!(ink(Style::Replace, true, 15).apply(9, false), Some(5));
    }

    #[test]
    fn complement_inverts_the_selected_planes_as_the_manual_tabulates() {
        // Manual table 3-5: I5 complemented is I10; under W(F1) it is I4.
        assert_eq!(ink(Style::Complement, false, 15).apply(5, true), Some(10));
        assert_eq!(ink(Style::Complement, false, 1).apply(5, true), Some(4));
        assert_eq!(ink(Style::Complement, false, 14).apply(6, true), Some(8));
    }

    #[test]
    fn erase_ignores_the_pattern() {
        assert_eq!(ink(Style::Erase, false, 15).apply(9, false), Some(2));
        assert_eq!(ink(Style::Erase, true, 15).apply(9, true), Some(5));
    }

    #[test]
    fn the_plane_mask_protects_the_other_planes() {
        assert_eq!(ink(Style::Replace, false, 0b0011).apply(0b1100, true), Some(0b1101));
        assert_eq!(ink(Style::Replace, false, 0).apply(0b1100, true), Some(0b1100));
    }
}
