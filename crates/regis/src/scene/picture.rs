//! The picture a caller shows: the base with the numbered drawings over
//! it, its pick record, and what it cost.

use alloc::collections::BTreeMap;
use alloc::vec::Vec;

use super::paint::Step;
use crate::convert::to_usize;
use crate::palette::Palette;
use crate::surface::{Bounds, Canvas, Screen, Window};

/// What a picture was painted from, kept so that the next one repaints
/// only what changed.
#[derive(Clone, Debug)]
pub(super) struct Painted {
    pub(super) steps: Vec<Step>,
    /// How many of the steps, the first ones, are the background's: the
    /// top-level drawings of layer 0 and what nests in them.
    pub(super) background: usize,
    /// The base's pixels.
    pub(super) base: Vec<u8>,
    pub(super) palette: Palette,
    pub(super) tables_version: u64,
}

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
    /// The pixels the rectangles being repainted held before, each
    /// rectangle's rows in turn, to tell what changed.
    previous: Vec<u8>,
    /// The rectangles being repainted.
    repainted: Vec<Bounds>,
    pub(super) painted: Option<Painted>,
    /// The background raster: the base with the drawings of layer 0 over
    /// it, repainted only when one of those changes, so that a drawing
    /// moving in a layer above never has them replayed (animation.md,
    /// ANIM-12). `None` while no drawing is in layer 0: the base is then
    /// the background.
    pub(super) background: Option<Screen>,
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
            repainted: Vec::new(),
            painted: None,
            background: None,
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

    /// The screen position of the origin of each detectable drawing that
    /// was painted, by drawing number: what [`Picture::pick`] subtracts.
    /// A caller that keeps the pick record apart from the picture, as one
    /// that passes it to another process does, keeps these with it.
    pub fn origins(&self) -> impl Iterator<Item = (u16, i32, i32)> + '_ {
        self.origins.iter().map(|(drawing, (x, y))| (*drawing, *x, *y))
    }

    /// The count of commits applied when the picture was composed.
    #[must_use]
    pub const fn commits(&self) -> u32 {
        self.commits
    }

    /// The work units making this picture cost: 64 for each drawing
    /// visited, and for each drawing repainted the pixels visited and 64
    /// for each byte of contents replayed. A picture repainted in part
    /// costs what was repainted.
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

    /// The pixels of each row of a rectangle, top to bottom.
    fn rows(&self, area: Bounds) -> impl Iterator<Item = &[u8]> + '_ {
        let width = to_usize(Canvas::width_i32());
        let columns = to_usize(area.left)..to_usize(area.right).saturating_add(1);
        let pixels = self.canvas().pixels();
        (to_usize(area.top)..=to_usize(area.bottom)).map(move |row| {
            let start = row.saturating_mul(width);
            pixels.get(start.saturating_add(columns.start)..start.saturating_add(columns.end)).unwrap_or(&[])
        })
    }

    /// Starts a picture: the background's pixels and pick record are put
    /// back in each rectangle to be repainted, or everywhere when the
    /// picture is painted whole; `bounds` holds them all, and its rows are
    /// kept to find which changed.
    pub(super) fn begin(
        &mut self,
        base: &Canvas,
        bounds: Option<Bounds>,
        rectangles: &[Bounds],
        whole: bool,
        origins: BTreeMap<u16, (i32, i32)>,
    ) {
        self.origins = origins;
        // Only what is repainted can change, so only it is kept: a key at
        // one side of the picture and a meter at the other keep their two
        // rectangles, not every row between them.
        self.repainted.clear();
        if rectangles.is_empty() {
            self.repainted.extend(bounds.and_then(|bounds| bounds.within(Bounds::page())));
        } else {
            self.repainted.extend(rectangles.iter().filter_map(|area| area.within(Bounds::page())));
        }
        let mut previous = core::mem::take(&mut self.previous);
        previous.clear();
        for &area in &self.repainted {
            for row in self.rows(area) {
                previous.extend_from_slice(row);
            }
        }
        self.previous = previous;
        if let Some(canvas) = self.screen.page_mut(0) {
            canvas.set_window(Window::page());
            if whole {
                canvas.copy_from(base);
            } else {
                for &area in rectangles {
                    canvas.restore(base, area);
                }
            }
        }
    }

    /// Finishes a picture: what it cost and which rows changed.
    pub(super) fn end(&mut self, commits: u32, work: u64) {
        self.commits = commits;
        self.work = work;
        if let Some(canvas) = self.screen.page_mut(0) {
            canvas.tidy_pick();
        }
        let mut changed: Option<(u32, u32)> = None;
        let mut previous = self.previous.as_slice();
        for &area in &self.repainted {
            for (row, now) in (area.top..).zip(self.rows(area)) {
                let (was, rest) = previous.split_at_checked(now.len()).unwrap_or((previous, &[]));
                previous = rest;
                if was != now {
                    let row = row.unsigned_abs();
                    changed = Some(changed.map_or((row, row), |(first, last)| (first.min(row), last.max(row))));
                }
            }
        }
        self.changed = changed;
    }
}
