//! The terminal: the public face of the renderer.

mod source;
mod stream;

use alloc::vec::Vec;

use crate::event::Event;
use crate::font::Font;
use crate::lexer::Lexer;
use crate::machine::Machine;
use crate::palette::Palette;
use crate::surface::Screen;
use source::Source;
use stream::{Step, Stream};

/// A ReGIS terminal's state: everything but the pixels.
///
/// Feed it bytes in pieces of any size; it draws on the [`Screen`] lent to
/// each call and returns what the caller must act on.
#[derive(Debug)]
pub struct Terminal {
    stream: Stream,
    source: Source,
    lexer: Lexer,
    machine: Machine,
}

impl Default for Terminal {
    fn default() -> Self {
        Self::new()
    }
}

impl Terminal {
    /// A terminal as at power-up (manual chapter 1, table 1-2), with no font.
    #[must_use]
    pub fn new() -> Self {
        Self { stream: Stream::default(), source: Source::default(), lexer: Lexer::default(), machine: Machine::new() }
    }

    /// Supplies the shapes of the built-in characters.
    pub fn set_font(&mut self, font: Font) {
        self.machine.set_font(font);
    }

    /// Allows `R(M(letter))` to report a macrograph's contents. It is off
    /// at first: the manual warns the report "may be a security risk", and
    /// the VT340 lets its user disable it.
    pub fn allow_macrograph_reports(&mut self, allowed: bool) {
        self.machine.set_macrograph_reports(allowed);
    }

    /// Has `feed` obey the 8-bit controls 0x90 (device control string) and
    /// 0x9C (string terminator) as well as `ESC P` and `ESC \`. It is off
    /// at first, because both bytes occur inside UTF-8 text, where obeying
    /// them would let plain text start or end ReGIS.
    pub fn recognise_eight_bit_controls(&mut self, recognise: bool) {
        self.stream.recognise_eight_bit_controls(recognise);
    }

    /// The output map: the colour of each pixel value.
    #[must_use]
    pub const fn palette(&self) -> &Palette {
        self.machine.palette()
    }

    /// The active position, in the stream's own coordinates.
    #[must_use]
    pub const fn position(&self) -> (i32, i32) {
        self.machine.position()
    }

    /// Whether the stream has the graphics cursor switched on (`S(C1)`).
    /// The library does not draw the cursor.
    #[must_use]
    pub const fn cursor_visible(&self) -> bool {
        self.machine.cursor_visible()
    }

    /// Feeds bytes of a terminal data stream. ReGIS is taken from device
    /// control strings (`ESC P p` … `ESC \`); everything else is ignored.
    #[must_use = "the events hold replies and limit notices the caller must act on"]
    pub fn feed(&mut self, screen: &mut Screen, bytes: &[u8]) -> Vec<Event> {
        for &byte in bytes {
            self.machine.earn();
            match self.stream.step(byte) {
                Step::Outside => {}
                Step::Enter { restart } => {
                    if restart {
                        self.restart(screen);
                    }
                }
                Step::Regis => self.command_byte(screen, byte),
            }
        }
        self.machine.take_events()
    }

    /// Feeds ReGIS commands directly, with no device control string.
    #[must_use = "the events hold replies and limit notices the caller must act on"]
    pub fn feed_regis(&mut self, screen: &mut Screen, bytes: &[u8]) -> Vec<Event> {
        for &byte in bytes {
            self.machine.earn();
            self.command_byte(screen, byte);
        }
        self.machine.take_events()
    }

    /// Ends the input: acts on a number or string that was only waiting
    /// for the byte after it.
    #[must_use = "the events hold replies and limit notices the caller must act on"]
    pub fn finish(&mut self, screen: &mut Screen) -> Vec<Event> {
        if let Some(token) = self.lexer.finish() {
            self.machine.token(screen, token);
        }
        self.machine.take_events()
    }

    /// `ESC P 1 p`: "ReGIS begins at a new command".
    pub(super) fn restart(&mut self, screen: &mut Screen) {
        self.lexer.reset();
        self.source.reset();
        self.machine.resynchronise(screen);
    }
}
