//! The text controls (manual chapter 7).
#![allow(
    clippy::arithmetic_side_effects,
    reason = "cell sizes are clamped to 2560 and directions to 0..8, so products stay far inside i32"
)]

use crate::font::CELL_WIDTH;
use crate::geometry::{direction, direction_of_angle};
use crate::limits::MAX_CELL_SIDE;

/// The largest standard character size (manual chapter 7, table 7-3).
const MAX_STANDARD_SIZE: i32 = 16;
/// The height multiplier counts in stored cell heights of ten pixels.
const HEIGHT_UNIT: i32 = 10;
/// Unit cell heights are multiples of five (manual chapter 7, "Unit Cell Size").
const UNIT_HEIGHT_STEP: i32 = 5;
/// The width factor of the size multiplier stops at 16.
const MAX_WIDTH_FACTOR: i32 = 16;
/// The height multiplier "is restricted to a range of 1 to 256".
const MAX_HEIGHT_FACTOR: i32 = 256;

/// A width and a height in pixels.
pub(crate) type Size = (i32, i32);

/// Everything the `T` command sets.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct TextControls {
    /// The character set: 0 is built in, 1 to 3 are loadable.
    pub(crate) alphabet: usize,
    /// The screen area each character takes.
    pub(crate) display: Size,
    /// The size the character shape is drawn at within it.
    pub(crate) unit: Size,
    /// The move after each character, if set outright; otherwise the
    /// display width along the string's direction.
    pub(crate) spacing: Option<(i32, i32)>,
    /// The string's baseline, as a pixel-vector direction.
    pub(crate) string_direction: i32,
    /// Each character's own baseline.
    pub(crate) character_direction: i32,
    /// The italic slant in pixels sideways per five pixels of height;
    /// positive leans the top to the left.
    pub(crate) slant: i32,
}

impl Default for TextControls {
    /// Standard size 1, no tilt (manual chapter 1, table 1-2).
    fn default() -> Self {
        Self {
            alphabet: 0,
            display: (9, 20),
            unit: (8, 20),
            spacing: None,
            string_direction: 0,
            character_direction: 0,
            slant: 0,
        }
    }
}

impl TextControls {
    /// Selects a standard size, 0 to 16; false if out of range.
    ///
    /// Manual table 7-3: size 0 is a 9 by 10 display cell, size 1 is 9 by
    /// 20, and size n from 2 up is 9n by 15n; the unit cell is one pixel in
    /// nine narrower.
    pub(crate) fn set_standard(&mut self, size: i32) -> bool {
        if !(0..=MAX_STANDARD_SIZE).contains(&size) {
            return false;
        }
        let columns = size.max(1);
        let height = match size {
            0 => 10,
            1 => 20,
            _ => 15 * size,
        };
        self.display = (9 * columns, height);
        self.unit = (CELL_WIDTH * columns, height);
        self.spacing = None;
        true
    }

    pub(crate) fn set_display(&mut self, width: Option<i32>, height: Option<i32>) {
        self.display = (clamp(width.unwrap_or(self.display.0)), clamp(height.unwrap_or(self.display.1)));
    }

    /// Sets the unit cell; a size between the allowed steps becomes "the
    /// next smaller size".
    pub(crate) fn set_unit(&mut self, width: Option<i32>, height: Option<i32>) {
        let width = clamp(width.unwrap_or(self.unit.0));
        let height = clamp(height.unwrap_or(self.unit.1));
        self.unit = ((width / CELL_WIDTH).max(1) * CELL_WIDTH, (height / UNIT_HEIGHT_STEP).max(1) * UNIT_HEIGHT_STEP);
    }

    /// `T(H n)`: both cells become n stored cells high.
    pub(crate) fn set_height_multiplier(&mut self, factor: i32) {
        let height = factor.clamp(1, MAX_HEIGHT_FACTOR) * HEIGHT_UNIT;
        self.display.1 = height;
        self.unit.1 = height;
    }

    /// `T(M[w,h])`: the unit cell as multiples of the stored cell.
    pub(crate) fn set_size_multiplier(&mut self, width: Option<i32>, height: Option<i32>) {
        if let Some(width) = width {
            self.unit.0 = width.clamp(1, MAX_WIDTH_FACTOR) * CELL_WIDTH;
        }
        if let Some(height) = height {
            self.unit.1 = height.clamp(1, MAX_HEIGHT_FACTOR) * HEIGHT_UNIT;
        }
    }

    /// `T(I a)`: the manual's table of italic angles.
    pub(crate) fn set_italic(&mut self, degrees: i32) {
        const STEEP: i32 = 31;
        // 22 degrees is two pixels in five; 45 degrees is five in five.
        self.slant = match degrees {
            0 => 0,
            d if d >= STEEP => 5,
            d if d > 0 => 2,
            d if d <= -STEEP => -5,
            _ => -2,
        };
    }

    /// Sets a tilt; the first in a group of options turns the string and
    /// its characters, a later one the characters alone (manual chapter 7,
    /// "String/Character Tilt").
    pub(crate) fn set_tilt(&mut self, degrees: i32, characters_only: bool) {
        let direction = direction_of_angle(degrees);
        self.character_direction = direction;
        if !characters_only {
            self.string_direction = direction;
        }
    }

    /// The move from one character to the next, in pixels.
    pub(crate) fn advance(&self) -> (i32, i32) {
        self.spacing.unwrap_or_else(|| {
            let (x, y) = direction(self.string_direction);
            (x * self.display.0, y * self.display.0)
        })
    }

    /// The move down one line, in pixels.
    pub(crate) fn line_feed(&self) -> (i32, i32) {
        let (x, y) = direction(self.string_direction + 6);
        (x * self.display.1, y * self.display.1)
    }

    /// The move a pixel-vector digit makes in a text command: half a
    /// display cell, relative to the string's baseline (manual chapter 7,
    /// "PV Spacing"). Half of an odd side rounds towards the top and left
    /// of the screen, as the VT340 does: its nine-pixel cell moves four
    /// pixels right but five left.
    pub(crate) fn half_cell(&self, number: i32) -> (i32, i32) {
        let (along, down) = direction(number);
        let (ax, ay) = direction(self.string_direction);
        let (dx, dy) = direction(self.string_direction + 6);
        let (width, height) = self.display;
        let half = |side: i32| side.div_euclid(2);
        (
            half(along * width * ax).saturating_add(half(down * height * dx)),
            half(along * width * ay).saturating_add(half(down * height * dy)),
        )
    }
}

fn clamp(side: i32) -> i32 {
    side.clamp(1, MAX_CELL_SIDE)
}

#[cfg(test)]
mod tests {
    use super::TextControls;

    fn standard(size: i32) -> TextControls {
        let mut controls = TextControls::default();
        assert!(controls.set_standard(size));
        controls
    }

    #[test]
    fn standard_sizes_match_the_manuals_table() {
        assert_eq!((standard(0).display, standard(0).unit), ((9, 10), (8, 10)));
        assert_eq!((standard(1).display, standard(1).unit), ((9, 20), (8, 20)));
        assert_eq!((standard(2).display, standard(2).unit), ((18, 30), (16, 30)));
        assert_eq!((standard(16).display, standard(16).unit), ((144, 240), (128, 240)));
        assert!(!TextControls::default().set_standard(17));
        assert!(!TextControls::default().set_standard(-1));
    }

    #[test]
    fn a_unit_cell_between_steps_takes_the_next_smaller_size() {
        let mut controls = TextControls::default();
        controls.set_unit(Some(35), Some(38));
        assert_eq!(controls.unit, (32, 35));
    }

    #[test]
    fn multipliers_follow_the_manuals_arithmetic() {
        let mut controls = standard(3);
        controls.set_height_multiplier(7);
        assert_eq!((controls.display, controls.unit), ((27, 70), (24, 70)));
        controls.set_size_multiplier(Some(99), Some(2));
        assert_eq!(controls.unit, (128, 20));
    }

    #[test]
    fn italic_angles_snap_to_the_manuals_values() {
        let slant = |degrees| {
            let mut controls = TextControls::default();
            controls.set_italic(degrees);
            controls.slant
        };
        assert_eq!([slant(-45), slant(-10), slant(0), slant(22), slant(31)], [-5, -2, 0, 2, 5]);
    }

    #[test]
    fn spacing_follows_the_string_direction_and_pv_moves_are_half_cells() {
        let mut controls = standard(1);
        assert_eq!(controls.advance(), (9, 0));
        assert_eq!(controls.half_cell(6), (0, 10));
        assert_eq!(controls.half_cell(4), (-5, 0));
        assert_eq!(controls.half_cell(0), (4, 0));
        controls.set_tilt(90, false);
        assert_eq!(controls.advance(), (0, -9));
        assert_eq!(controls.line_feed(), (20, 0));
        controls.set_tilt(0, true);
        assert_eq!((controls.string_direction, controls.character_direction), (2, 0));
    }
}
