//! The work allowance that bounds what hostile input can cost.
//!
//! A two-byte command can ask for hundreds of thousands of pixels. Every
//! pixel the rasteriser visits is paid for from an allowance that grows with
//! the bytes fed, so total work is linear in input size.

use crate::limits::{REPAINT_WORK_HELD, REPAINT_WORK_PER_SECOND, WORK_CAP, WORK_PER_BYTE};

/// A token bucket of work units: filled by input bytes, drained by drawing.
#[derive(Debug)]
pub(crate) struct Work {
    left: u64,
    exhausted: bool,
    /// Every unit paid since the start, for a caller that reports it.
    spent: u64,
}

impl Work {
    pub(crate) const fn new() -> Self {
        Self { left: WORK_CAP, exhausted: false, spent: 0 }
    }

    /// An allowance of exactly `units`, for painting a numbered drawing.
    pub(crate) const fn with(units: u64) -> Self {
        Self { left: units, exhausted: false, spent: 0 }
    }

    /// The units not yet spent.
    pub(crate) const fn left(&self) -> u64 {
        self.left
    }

    /// Credits the allowance for `bytes` of new input.
    pub(crate) fn earn(&mut self, bytes: usize) {
        let bytes = u64::try_from(bytes).unwrap_or(u64::MAX);
        self.left = self.left.saturating_add(bytes.saturating_mul(WORK_PER_BYTE)).min(WORK_CAP);
    }

    /// Pays for `units` of work; false (and nothing paid) if they cannot be afforded.
    pub(crate) fn spend(&mut self, units: u64) -> bool {
        if let Some(left) = self.left.checked_sub(units) {
            self.left = left;
            self.spent = self.spent.saturating_add(units);
            true
        } else {
            self.exhausted = true;
            false
        }
    }

    /// The units paid since the start.
    pub(crate) const fn spent(&self) -> u64 {
        self.spent
    }

    /// Whether a spend was refused since this was last asked; clears the mark.
    pub(crate) fn take_exhausted(&mut self) -> bool {
        core::mem::take(&mut self.exhausted)
    }
}

/// The milliseconds in a second.
const MILLISECONDS: u64 = 1000;

/// What bounds how much repainting one picture may cost a receiver in a
/// second (`docs/regis-v2.md`, "Repaint budget").
///
/// A commit of a dozen bytes, or a movement of the viewer's pointer, can
/// cause a repaint of up to
/// [`MAX_REBUILD_WORK`](crate::limits::MAX_REBUILD_WORK), so repaint work
/// cannot be bounded by the bytes received. It is bounded by time, which
/// the library does not have: the receiver says how much has passed.
///
/// A receiver keeps one for each picture. Before each
/// [`Terminal::compose`](crate::Terminal::compose) it calls
/// [`earn`](Self::earn) for the time since it last did and asks
/// [`may_repaint`](Self::may_repaint). If it may, it composes and
/// [`spend`](Self::spend)s the picture's work; if not, it keeps showing
/// the picture it has and composes once when
/// [`milliseconds_until_repaint`](Self::milliseconds_until_repaint) have
/// passed, however many changes arrived meanwhile. Over any `t` seconds a
/// picture's repaints then cost at most
/// [`REPAINT_WORK_HELD`] + `t` × [`REPAINT_WORK_PER_SECOND`] +
/// `MAX_REBUILD_WORK` work units.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RepaintBudget {
    /// Work units in hand. A repaint that was allowed to start is paid
    /// for in full, so this goes below zero and is earned back.
    allowance: i64,
    /// Thousandths of a work unit earned and not yet counted, so that
    /// time fed in small pieces earns what it would in one.
    thousandths: u64,
}

impl Default for RepaintBudget {
    fn default() -> Self {
        Self::new()
    }
}

impl RepaintBudget {
    /// A full budget.
    #[must_use]
    pub const fn new() -> Self {
        Self { allowance: Self::held(), thousandths: 0 }
    }

    const fn held() -> i64 {
        // The constant is far below `i64::MAX`; the cast cannot wrap.
        REPAINT_WORK_HELD.cast_signed()
    }

    /// Credits the budget for time passed.
    pub fn earn(&mut self, milliseconds: u64) {
        let thousandths = milliseconds.saturating_mul(REPAINT_WORK_PER_SECOND).saturating_add(self.thousandths);
        let units = thousandths.checked_div(MILLISECONDS).unwrap_or(0);
        self.thousandths = thousandths.checked_rem(MILLISECONDS).unwrap_or(0);
        self.allowance = self.allowance.saturating_add(i64::try_from(units).unwrap_or(i64::MAX));
        if self.allowance >= Self::held() {
            self.allowance = Self::held();
            self.thousandths = 0;
        }
    }

    /// Whether a repaint may start now.
    #[must_use]
    pub const fn may_repaint(&self) -> bool {
        self.allowance > 0
    }

    /// Pays for a repaint: what [`Picture::work`](crate::Picture::work)
    /// reported.
    pub fn spend(&mut self, work: u64) {
        self.allowance = self.allowance.saturating_sub(i64::try_from(work).unwrap_or(i64::MAX));
    }

    /// The least time after which [`may_repaint`](Self::may_repaint) is
    /// true: what a receiver that owes a repaint waits for. 0 when it may
    /// repaint now.
    #[must_use]
    pub fn milliseconds_until_repaint(&self) -> u64 {
        if self.may_repaint() {
            return 0;
        }
        // One unit above zero, less what is already part-earned.
        let units = self.allowance.unsigned_abs().saturating_add(1);
        let thousandths = units.saturating_mul(MILLISECONDS).saturating_sub(self.thousandths);
        thousandths.div_ceil(REPAINT_WORK_PER_SECOND)
    }
}
