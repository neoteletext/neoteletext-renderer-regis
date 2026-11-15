//! The terminal: the public face of the renderer.

mod source;
mod stream;

use alloc::vec::Vec;

use crate::event::Event;
use crate::font::Font;
use crate::palette::Palette;
use crate::scene::{Picture, Scene};
use crate::surface::Screen;
pub(crate) use source::Interpreter;
use stream::{Step, Stream};

/// A ReGIS terminal's state: everything but the pixels.
///
/// Feed it bytes in pieces of any size; it draws on the [`Screen`] lent to
/// each call and returns what the caller must act on.
///
/// The numbered drawings of ReGIS v2 (`docs/regis-v2.md`) are kept here
/// too. They are not drawn on the [`Screen`], which holds only what the
/// stream draws outside them; [`Terminal::compose`] paints them over a
/// copy of it, when the caller wants a picture to show.
#[derive(Debug)]
pub struct Terminal {
    stream: Stream,
    base: Interpreter,
    scene: Scene,
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
        Self { stream: Stream::default(), base: Interpreter::new(), scene: Scene::new() }
    }

    /// Supplies the shapes of the built-in characters.
    pub fn set_font(&mut self, font: Font) {
        self.scene.set_font(font.clone());
        self.base.machine.set_font(font);
    }

    /// Allows `R(M(letter))` to report a macrograph's contents. It is off
    /// at first: the manual warns the report "may be a security risk", and
    /// the VT340 lets its user disable it.
    pub fn allow_macrograph_reports(&mut self, allowed: bool) {
        self.base.machine.set_macrograph_reports(allowed);
    }

    /// Has `feed` obey the 8-bit controls 0x90 (device control string) and
    /// 0x9C (string terminator) as well as `ESC P` and `ESC \`. It is off
    /// at first, because both bytes occur inside UTF-8 text, where obeying
    /// them would let plain text start or end ReGIS.
    pub fn recognise_eight_bit_controls(&mut self, recognise: bool) {
        self.stream.recognise_eight_bit_controls(recognise);
    }

    /// The output map: the colour of each pixel value. While ReGIS v2
    /// operations wait for their commit, a change to the map waits with
    /// them, so that new drawings and their colours appear together.
    #[must_use]
    pub fn palette(&self) -> &Palette {
        self.scene.held_palette().unwrap_or_else(|| self.base.machine.palette())
    }

    /// The active position, in the stream's own coordinates.
    #[must_use]
    pub const fn position(&self) -> (i32, i32) {
        self.base.machine.position()
    }

    /// Whether the stream has the graphics cursor switched on (`S(C1)`).
    /// The library does not draw the cursor.
    #[must_use]
    pub const fn cursor_visible(&self) -> bool {
        self.base.machine.cursor_visible()
    }

    /// Whether the stream has ever committed ReGIS v2 operations. Until it
    /// has, the [`Screen`] is the whole picture, as in ReGIS v1; after, the
    /// picture is what [`Terminal::compose`] makes.
    #[must_use]
    pub fn has_drawings(&self) -> bool {
        self.scene.commits() != 0 || self.scene.any_committed()
    }

    /// The count of ReGIS v2 commits applied; it wraps to 0.
    #[must_use]
    pub const fn commits(&self) -> u32 {
        self.scene.commits()
    }

    /// Makes the picture to show: the displayed page of `base`, the screen
    /// this terminal has been drawing on, with every visible numbered
    /// drawing painted over it in drawing order (`docs/regis-v2.md`,
    /// "Commit and repaint"). The whole picture is rebuilt at each call,
    /// so the caller calls it when it wants a frame: after an
    /// [`Event::Committed`], and when [`Screen::take_changed`] says the
    /// base changed.
    pub fn compose(&mut self, base: &Screen, picture: &mut Picture) {
        let palette = self.palette().clone();
        self.scene.compose(base, &palette, picture);
    }

    /// Feeds bytes of a terminal data stream. ReGIS is taken from device
    /// control strings (`ESC P p` … `ESC \`); everything else is ignored.
    #[must_use = "the events hold replies and limit notices the caller must act on"]
    pub fn feed(&mut self, screen: &mut Screen, bytes: &[u8]) -> Vec<Event> {
        for &byte in bytes {
            self.base.machine.earn();
            match self.stream.step(byte) {
                Step::Outside => {}
                Step::Enter { restart } => {
                    if restart {
                        self.base.restart(screen);
                    }
                }
                Step::Regis => self.regis_byte(screen, byte),
            }
            if !self.stream.in_regis() {
                // A definition left open when ReGIS mode ends is dropped,
                // and its batch fails.
                self.scene.abandon_definition();
            }
        }
        self.base.machine.take_events()
    }

    /// Feeds ReGIS commands directly, with no device control string.
    #[must_use = "the events hold replies and limit notices the caller must act on"]
    pub fn feed_regis(&mut self, screen: &mut Screen, bytes: &[u8]) -> Vec<Event> {
        for &byte in bytes {
            self.base.machine.earn();
            self.regis_byte(screen, byte);
        }
        self.base.machine.take_events()
    }

    /// Ends the input: acts on a number or string that was only waiting
    /// for the byte after it. A ReGIS v2 definition still open is dropped.
    #[must_use = "the events hold replies and limit notices the caller must act on"]
    pub fn finish(&mut self, screen: &mut Screen) -> Vec<Event> {
        self.base.finish(screen);
        self.scene.abandon_definition();
        self.base.machine.take_events()
    }

    /// One byte of ReGIS: the contents of a numbered drawing being
    /// defined, or a command.
    fn regis_byte(&mut self, screen: &mut Screen, byte: u8) {
        if self.scene.defining() {
            self.scene.definition_byte(byte);
            return;
        }
        let was_drawing = self.base.machine.in_drawing_command();
        self.base.command_byte(screen, byte);
        if was_drawing || self.base.machine.in_drawing_command() {
            self.scene.count_staged(1);
        }
        if !self.base.machine.operations.is_empty() {
            let operations = core::mem::take(&mut self.base.machine.operations);
            for operation in operations {
                self.scene.apply(operation, &mut self.base.machine);
            }
            if self.scene.defining() {
                // The definition's bytes are stored, not read: nothing
                // half-read may be left for the first of them to finish.
                self.base.forget_input();
            }
        }
    }
}
