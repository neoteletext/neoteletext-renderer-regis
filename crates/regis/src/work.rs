//! The work allowance that bounds what hostile input can cost.
//!
//! A two-byte command can ask for hundreds of thousands of pixels. Every
//! pixel the rasteriser visits is paid for from an allowance that grows with
//! the bytes fed, so total work is linear in input size.

use crate::limits::{WORK_CAP, WORK_PER_BYTE};

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
