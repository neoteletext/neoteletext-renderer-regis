//! The paintings a receiver watches: the numbered drawings whose becoming
//! whole it is told of, so that a page's rule table can act when a
//! drawing has finished painting (DEC-7031 item 2; REQ-7026).
//!
//! Each watched drawing has an episode, open from the moment its painting
//! is owed (it is newly watched, new, redefined, given another rate in
//! force, shown, or under a drawing whose painting starts) until the
//! drawing is next whole, when it is reported once. Only watched drawings
//! are reported, so that a scene of thousands of drawings cannot flood
//! the receiver with reports nobody asked for.

use alloc::vec::Vec;

use super::Scene;
use super::animation::Rated;
use crate::limits::{MAX_DRAWING_DEPTH, MAX_WATCHED_PAINTINGS};

/// What is kept for one watched drawing.
#[derive(Clone, Copy, Debug)]
pub(super) struct Watch {
    /// An episode is open: the drawing is reported when it is next whole.
    pub(super) open: bool,
    /// It became whole and the receiver has not yet been told. Kept apart
    /// from `open`, so that a drawing that finishes and whose painting is
    /// owed again before the receiver asks is reported for both.
    finished: bool,
}

/// The watched drawings, in order of number. A list and not a map: it is
/// short, and an empty list holds nothing, where a map's accounting must
/// allow for a root node in every terminal and validator, watching or
/// not (SOLVEDSECURITYISSUES 015 counts what each reader holds).
pub(super) type Watched = Vec<(u16, Watch)>;

/// The record kept for a drawing, if it is watched.
pub(super) fn find(watched: &mut Watched, number: u16) -> Option<&mut Watch> {
    let at = watched.binary_search_by_key(&number, |(watched, _)| *watched).ok()?;
    watched.get_mut(at).map(|(_, watch)| watch)
}

impl Scene {
    /// Replaces the watched drawings. A drawing not watched before has an
    /// episode opened; one watched before keeps its episode and any report
    /// not yet taken. False, with nothing changed, when more than
    /// [`MAX_WATCHED_PAINTINGS`] different numbers are named.
    pub(crate) fn watch_painted(&mut self, numbers: &[u16]) -> bool {
        // The caller's list may be of any length: no more room is made
        // than the cap, and it is refused as soon as it is one too many.
        let mut watched = Watched::with_capacity(numbers.len().min(MAX_WATCHED_PAINTINGS));
        for number in numbers {
            let Err(at) = watched.binary_search_by_key(number, |(watched, _)| *watched) else { continue };
            if watched.len() >= MAX_WATCHED_PAINTINGS {
                return false;
            }
            let watch = find(&mut self.watched, *number).map_or(Watch { open: true, finished: false }, |watch| *watch);
            watched.insert(at, (*number, watch));
        }
        self.watched = watched;
        true
    }

    /// The watched drawings that became whole since the last call, in
    /// order of number.
    pub(crate) fn take_painted(&mut self) -> Vec<u16> {
        let taken = |(number, watch): &mut (u16, Watch)| core::mem::take(&mut watch.finished).then_some(*number);
        self.watched.iter_mut().filter_map(taken).collect()
    }

    /// The watched drawings as they stand, in order of number, before a
    /// commit or a change of the default rate.
    pub(super) fn watched_standing(&self) -> Vec<Rated> {
        let standing = |(number, _): &(u16, Watch)| Some(self.standing(*number, self.committed.get(*number)?));
        self.watched.iter().filter_map(standing).collect()
    }

    /// After a commit or a change of the default rate: a watched drawing
    /// that no longer exists has its episode dropped, and one that is
    /// new, redefined, at another rate in force or newly visible has one
    /// opened, by the rule that starts a gradual painting (DEC-6382 item
    /// 2), whether or not it has a rate: a drawing that appears at once
    /// is reported in the same call (REQ-7026).
    pub(super) fn open_changed(&mut self, before: &[Rated]) {
        let mut watched = core::mem::take(&mut self.watched);
        for (number, watch) in &mut watched {
            match self.committed.get(*number) {
                None => watch.open = false,
                Some(drawing) if self.starts_since(before, *number, drawing) => watch.open = true,
                Some(_) => {}
            }
        }
        self.watched = watched;
    }

    /// The watched drawings that are not shown, with every drawing up
    /// their parent chain, before a change of the animation state.
    pub(super) fn watched_hidden(&self) -> Vec<u16> {
        let watched = self.watched.iter().map(|(number, _)| *number);
        watched.filter(|number| !self.shown_up_the_chain(*number)).collect()
    }

    /// After a change of the animation state: each watched drawing it
    /// showed, itself or by showing a drawing above it, has an episode
    /// opened, as its painting starts then (ANIM-15); and every episode
    /// whose drawing is now whole is closed.
    pub(super) fn open_shown(&mut self, hidden_before: &[u16]) {
        for number in hidden_before {
            if self.shown_up_the_chain(*number)
                && let Some(watch) = find(&mut self.watched, *number)
            {
                watch.open = true;
            }
        }
        self.settle_watched();
    }

    /// Closes every open episode whose drawing is whole, noting it for
    /// the receiver: called when a commit, the passing of ticks, the
    /// animation state or the default rate may have made one so.
    pub(super) fn settle_watched(&mut self) {
        if self.watched.is_empty() {
            return;
        }
        let mut watched = core::mem::take(&mut self.watched);
        for (number, watch) in &mut watched {
            if watch.open && self.whole(*number) {
                *watch = Watch { open: false, finished: true };
            }
        }
        self.watched = watched;
    }

    /// Whether a drawing is whole: it exists, it is shown with every
    /// drawing up its parent chain (the animation state counted), and
    /// neither it nor a drawing above it is being painted gradually or is
    /// about to be. A drawing noted as starting takes with it every
    /// drawing below that has a rate in force (DEC-6382 item 3), so one
    /// found up the chain counts only when a rate in force lies between
    /// it and this drawing, either end included.
    fn whole(&self, number: u16) -> bool {
        let mut at = number;
        let mut rated = false;
        // The commit bounded the chain; a longer one is a fault there.
        for _ in 0..=MAX_DRAWING_DEPTH {
            let Some(drawing) = self.committed.get(at) else { return false };
            let shown = self.animation.get(&at).and_then(|state| state.shown).unwrap_or(drawing.visible);
            if !shown || self.painting.contains_key(&at) {
                return false;
            }
            rated |= drawing.rate_in_force(self.default_rate) != 0;
            if rated && self.starting.contains(&at) {
                return false;
            }
            if drawing.parent == 0 {
                return true;
            }
            at = drawing.parent;
        }
        false
    }
}
