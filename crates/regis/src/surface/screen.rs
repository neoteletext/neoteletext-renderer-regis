//! The terminal's graphics memory: two pages, one of them on show.

use super::Canvas;

/// How many graphics pages a single session has (manual chapter 4, "Select
/// Graphics Page").
pub(crate) const PAGES: usize = 2;

/// The VT340's graphics memory: two pages of 800 by 480 pixels.
///
/// The caller owns it and lends it to every feeding call.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Screen {
    pages: [Canvas; PAGES],
    displayed: usize,
    /// The page on show was changed since the caller last asked.
    switched: bool,
}

impl Screen {
    /// Two blank pages, the first on show.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// The page the terminal is showing.
    #[must_use]
    pub fn displayed(&self) -> &Canvas {
        let [first, _] = &self.pages;
        self.page(self.displayed).unwrap_or(first)
    }

    /// Which page is on show: 0 or 1.
    #[must_use]
    pub const fn displayed_index(&self) -> usize {
        self.displayed
    }

    /// A page by number, 0 or 1.
    #[must_use]
    pub fn page(&self, number: usize) -> Option<&Canvas> {
        self.pages.get(number)
    }

    /// Whether drawing changed a pixel of either page, or which page is on
    /// show, since this was last asked: a caller that shows the screen
    /// needs a new frame when it did.
    pub fn take_changed(&mut self) -> bool {
        let mut changed = core::mem::replace(&mut self.switched, false);
        for page in &mut self.pages {
            changed |= page.take_changed();
        }
        changed
    }

    /// A screen whose first page alone has pixel memory: what numbered
    /// drawings are composed on.
    pub(crate) fn single() -> Self {
        Self { pages: [Canvas::new(), Canvas::absent()], displayed: 0, switched: false }
    }

    /// A screen with no pixel memory at all: drawing on it does nothing.
    pub(crate) fn absent() -> Self {
        Self { pages: [Canvas::absent(), Canvas::absent()], displayed: 0, switched: false }
    }

    pub(crate) fn page_mut(&mut self, number: usize) -> Option<&mut Canvas> {
        self.pages.get_mut(number)
    }

    /// Shows a page; numbers other than 0 and 1 are ignored.
    pub(crate) fn display(&mut self, number: usize) {
        if number < PAGES {
            self.switched |= self.displayed != number;
            self.displayed = number;
        }
    }
}
