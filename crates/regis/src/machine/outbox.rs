//! What the interpreter has to tell the caller, held until it is collected.
//!
//! Everything queued here is bounded. The number of events is bounded for
//! one feeding call. Reply bytes and delay ticks are paid from budgets that
//! refill with the bytes fed, so the bound does not depend on how a sender
//! divides its bytes between calls (SOLVEDSECURITYISSUES 009).

use alloc::vec::Vec;

use crate::event::Event;
use crate::limits::{MAX_EVENTS, MAX_REPLY_BYTES, MAX_SAVED_WAIT_TICKS};

/// The events queued since the caller last collected them.
#[derive(Debug)]
pub(crate) struct Outbox {
    events: Vec<Event>,
    /// Reply bytes that may still be queued.
    reply_budget: usize,
    /// Delay ticks that may still be asked for.
    wait_budget: u32,
    /// Something was dropped at a limit.
    limited: bool,
}

impl Default for Outbox {
    fn default() -> Self {
        Self { events: Vec::new(), reply_budget: MAX_REPLY_BYTES, wait_budget: MAX_SAVED_WAIT_TICKS, limited: false }
    }
}

impl Outbox {
    /// Credits the budgets for one byte of new input.
    pub(crate) fn earn(&mut self) {
        self.reply_budget = self.reply_budget.saturating_add(1).min(MAX_REPLY_BYTES);
        self.wait_budget = self.wait_budget.saturating_add(1).min(MAX_SAVED_WAIT_TICKS);
    }

    /// Whether a reply of this many bytes could be queued now.
    pub(crate) const fn affords_reply(&self, bytes: usize) -> bool {
        bytes <= self.reply_budget
    }

    /// Queues an event; past the limit it is dropped.
    pub(crate) fn push(&mut self, event: Event) {
        if self.events.len() < MAX_EVENTS {
            self.events.push(event);
        } else {
            self.limited = true;
        }
    }

    /// Queues a reply to the host, within the bytes one call may queue
    /// (SOLVEDSECURITYISSUES 003).
    pub(crate) fn reply(&mut self, bytes: Vec<u8>) {
        if let Some(left) = self.reply_budget.checked_sub(bytes.len()) {
            self.reply_budget = left;
            self.push(Event::Reply(bytes));
        } else {
            self.limited = true;
        }
    }

    /// Queues a delay, within the ticks one call may ask for
    /// (SOLVEDSECURITYISSUES 006).
    pub(crate) fn wait(&mut self, ticks: u32) {
        let allowed = ticks.min(self.wait_budget);
        self.wait_budget = self.wait_budget.saturating_sub(allowed);
        if allowed > 0 {
            self.push(Event::Wait(allowed));
        }
        if allowed < ticks {
            self.limited = true;
        }
    }

    /// Notes that something was not done because a limit was reached.
    pub(crate) fn limit_reached(&mut self) {
        self.limited = true;
    }

    /// Hands over the queued events, ending with a limit notice if anything
    /// was dropped.
    pub(crate) fn take(&mut self) -> Vec<Event> {
        let mut events = core::mem::take(&mut self.events);
        if core::mem::take(&mut self.limited) {
            events.truncate(MAX_EVENTS.saturating_sub(1));
            events.push(Event::LimitReached);
        }
        events
    }
}
