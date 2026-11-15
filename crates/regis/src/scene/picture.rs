//! The picture a caller shows: the base with the numbered drawings over
//! it, its pick record, and what it cost.

use alloc::collections::BTreeMap;
use alloc::vec::Vec;

use crate::convert::{to_i32, to_usize};
use crate::surface::{Canvas, Screen, Window};

/// What lies under one position of a [`Picture`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Pick {
    /// The numbered drawing picked: the nearest detectable drawing at or
    /// above the last drawing to write the pixel. 0 for none.
    pub drawing: u16,
    /// The position within that drawing, which may be negative; (0, 0)
    /// when no drawing is picked.
    pub x: i32,
    /// See `x`.
    pub y: i32,
}

/// A composed picture of 800 by 480 pixels: what
/// [`Terminal::compose`](crate::Terminal::compose) makes.
///
/// The caller owns it and lends it to each call, so its memory is reused.
/// Its pixels, its pick record and its commit count always belong to the
/// same commit.
#[derive(Clone, Debug)]
pub struct Picture {
    screen: Screen,
    /// The pixels of the picture before this one, to tell what changed.
    previous: Vec<u8>,
    /// The screen position of each detectable drawing painted.
    origins: BTreeMap<u16, (i32, i32)>,
    commits: u32,
    work: u64,
    changed: Option<(u32, u32)>,
}

impl Default for Picture {
    fn default() -> Self {
        Self::new()
    }
}

impl Picture {
    /// A blank picture.
    #[must_use]
    pub fn new() -> Self {
        Self {
            screen: Screen::single(),
            previous: Vec::new(),
            origins: BTreeMap::new(),
            commits: 0,
            work: 0,
            changed: None,
        }
    }

    /// The pixels: colour-map addresses 0 to 15.
    #[must_use]
    pub fn canvas(&self) -> &Canvas {
        self.screen.displayed()
    }

    /// The pick record, row by row from the top-left: for each pixel the
    /// numbered drawing that picking reports there, 0 for none. It is
    /// empty when no detectable drawing wrote a pixel.
    #[must_use]
    pub fn pick_record(&self) -> &[u16] {
        self.canvas().pick()
    }

    /// What lies under a position; positions outside the picture pick
    /// nothing.
    #[must_use]
    pub fn pick(&self, x: u32, y: u32) -> Pick {
        let nothing = Pick { drawing: 0, x: 0, y: 0 };
        if x >= self.canvas().width() || y >= self.canvas().height() {
            return nothing;
        }
        let (x, y) = (i32::try_from(x).unwrap_or(0), i32::try_from(y).unwrap_or(0));
        let index = to_usize(y).saturating_mul(to_usize(Canvas::width_i32())).saturating_add(to_usize(x));
        let drawing = self.pick_record().get(index).copied().unwrap_or(0);
        match self.origins.get(&drawing) {
            Some((left, top)) if drawing != 0 => {
                Pick { drawing, x: x.saturating_sub(*left), y: y.saturating_sub(*top) }
            }
            _ => nothing,
        }
    }

    /// The count of commits applied when the picture was composed.
    #[must_use]
    pub const fn commits(&self) -> u32 {
        self.commits
    }

    /// The work units painting the numbered drawings cost: the pixels
    /// visited, 64 for each byte of contents replayed and 64 for each
    /// drawing painted.
    #[must_use]
    pub const fn work(&self) -> u64 {
        self.work
    }

    /// The first and last rows whose pixels differ from the picture
    /// composed before this one; `None` if none do.
    #[must_use]
    pub const fn changed_rows(&self) -> Option<(u32, u32)> {
        self.changed
    }

    pub(super) fn screen_mut(&mut self) -> &mut Screen {
        &mut self.screen
    }

    pub(super) fn note_origin(&mut self, drawing: u16, x: i32, y: i32) {
        self.origins.insert(drawing, (x, y));
    }

    /// Starts a picture from the base's pixels.
    pub(super) fn begin(&mut self, base: &Canvas) {
        self.previous.clear();
        self.previous.extend_from_slice(self.screen.displayed().pixels());
        self.origins.clear();
        if let Some(canvas) = self.screen.page_mut(0) {
            canvas.set_window(Window::page());
            canvas.copy_from(base);
        }
    }

    /// Finishes a picture: what it cost and which rows changed.
    pub(super) fn end(&mut self, commits: u32, work: u64) {
        self.commits = commits;
        self.work = work;
        let width = to_usize(Canvas::width_i32()).max(1);
        let differs = |(old, new): (&[u8], &[u8])| old != new;
        let rows = || self.previous.chunks(width).zip(self.canvas().pixels().chunks(width));
        let first = rows().position(differs);
        let last = rows().rposition(differs);
        self.changed = first.zip(last).map(|(first, last)| (to_i32(first).unsigned_abs(), to_i32(last).unsigned_abs()));
    }
}
