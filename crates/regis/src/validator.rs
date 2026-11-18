//! Checking ReGIS v2 before it is sent (`docs/regis-v2.md`, "Checking
//! before sending").

use alloc::string::String;
use alloc::vec::Vec;

use crate::event::Event;
use crate::font::Font;
use crate::scene::CommittedDrawing;
use crate::surface::Screen;
use crate::terminal::Terminal;

/// What a sender feeds its ReGIS to before sending it: a receiver's own
/// code and caps, so that a commit fails here exactly when it would fail
/// at the receiver. It keeps no pixels: what the stream draws outside
/// numbered drawings is read and not drawn, since no check depends on it.
///
/// Feed it every byte that will be sent, in order. An
/// [`Event::CommitFailed`] among the events returned means the receiver
/// would refuse that commit too.
#[derive(Debug)]
pub struct Validator {
    terminal: Terminal,
    screen: Screen,
}

impl Default for Validator {
    fn default() -> Self {
        Self::new()
    }
}

impl Validator {
    /// A validator in the state of a receiver that has been sent nothing.
    #[must_use]
    pub fn new() -> Self {
        Self { terminal: Terminal::new(), screen: Screen::absent() }
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

    /// The work units the bytes fed so far have spent drawing outside
    /// numbered drawings, as [`Terminal::base_work`] counts them: 0 when
    /// they drew nothing on the base picture. The validator keeps no
    /// pixels, but what a command would visit is paid for all the same.
    /// A sender asks after feeding a whole sprite sheet, which must draw
    /// nothing itself (neoteletext-common `docs/animation.md`, ANIM-17).
    #[must_use]
    pub const fn base_work(&self) -> u64 {
        self.terminal.base_work()
    }

    /// What the stream committed for a numbered drawing, as
    /// [`Terminal::committed_drawing`] gives it.
    #[must_use]
    pub fn committed_drawing(&self, number: u16) -> Option<CommittedDrawing> {
        self.terminal.committed_drawing(number)
    }

    /// The label of a committed numbered drawing, as [`Terminal::label`]
    /// gives it (DEC-6638 item 6).
    #[must_use]
    pub fn label(&self, number: u16) -> Option<String> {
        self.terminal.label(number)
    }

    /// Every committed drawing with a label, as [`Terminal::labels`] gives
    /// them.
    #[must_use]
    pub fn labels(&self) -> Vec<(u16, String)> {
        self.terminal.labels()
    }

    /// The picture's committed title, as [`Terminal::title`] gives it
    /// (DEC-6638 item 3).
    #[must_use]
    pub fn title(&self) -> Option<String> {
        self.terminal.title()
    }

    /// The lowest-numbered top-level drawing the bytes fed so far leave
    /// able to write on a receiver's screen, as
    /// [`Terminal::visible_drawing`] gives it; `None` when there is none.
    /// A sender asks after feeding a whole sprite sheet, and refuses the
    /// sheet when a drawing is named: a sheet defines hidden masters and
    /// draws nothing itself (neoteletext-common `docs/animation.md`,
    /// ANIM-17). Only committed drawings count: operations with no commit
    /// after them have changed nothing.
    #[must_use]
    pub fn visible_drawing(&self) -> Option<u16> {
        self.terminal.visible_drawing()
    }
}
