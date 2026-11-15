//! Checking ReGIS v2 before it is sent (`docs/regis-v2.md`, "Checking
//! before sending").

use alloc::vec::Vec;

use crate::event::Event;
use crate::font::Font;
use crate::surface::Screen;
use crate::terminal::Terminal;

/// What a sender feeds its ReGIS to before sending it: a receiver's own
/// code and caps, with a screen of its own, so that a commit fails here
/// exactly when it would fail at the receiver.
///
/// Feed it every byte that will be sent, in order. An
/// [`Event::CommitFailed`] among the events returned means the receiver
/// would refuse that commit too.
#[derive(Debug, Default)]
pub struct Validator {
    terminal: Terminal,
    screen: Screen,
}

impl Validator {
    /// A validator in the state of a receiver that has been sent nothing.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Supplies the shapes of the built-in characters, as
    /// [`Terminal::set_font`] does.
    pub fn set_font(&mut self, font: Font) {
        self.terminal.set_font(font);
    }

    /// Feeds ReGIS commands, as [`Terminal::feed_regis`] does.
    #[must_use = "a failed commit is reported in the events"]
    pub fn feed_regis(&mut self, bytes: &[u8]) -> Vec<Event> {
        self.terminal.feed_regis(&mut self.screen, bytes)
    }

    /// Ends the input, as [`Terminal::finish`] does.
    #[must_use = "a failed commit is reported in the events"]
    pub fn finish(&mut self) -> Vec<Event> {
        self.terminal.finish(&mut self.screen)
    }

    /// The count of commits applied.
    #[must_use]
    pub fn commits(&self) -> u32 {
        self.terminal.commits()
    }
}
