//! The terminal: the public face of the renderer.

mod source;
mod stream;

use alloc::string::String;
use alloc::vec::Vec;

use crate::event::Event;
use crate::font::Font;
use crate::palette::Palette;
use crate::scene::{AnimationState, CommittedDrawing, Picture, Rectangle, Scene, ViewerState};
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

    /// The work units drawing outside numbered drawings has cost since the
    /// terminal began: the pixels visited, as the allowance counts them.
    /// What painting the numbered drawings cost is each picture's
    /// ([`Picture::work`]).
    #[must_use]
    pub const fn base_work(&self) -> u64 {
        self.base.machine.work.spent()
    }

    /// The count of ReGIS v2 commits applied; it wraps to 0.
    #[must_use]
    pub const fn commits(&self) -> u32 {
        self.scene.commits()
    }

    /// Says what the viewer's pointer is doing to the picture: which
    /// drawing it is over, which is pressed, and which the caller has
    /// hidden (`docs/regis-v2.md`, "Viewer state"). The next
    /// [`Terminal::compose`] paints accordingly. It returns false when the
    /// picture certainly stays as it is, so that a caller composes only
    /// when there may be something new to show.
    pub fn set_viewer_state(&mut self, state: ViewerState) -> bool {
        self.scene.set_viewer_state(state)
    }

    /// Sets the animation state of a numbered drawing: the receiver's own
    /// position, shown or hidden, layer, order, frame and forced look for
    /// it, each used in place of what the stream committed (neoteletext-
    /// common `docs/animation.md`, ANIM-6 and ANIM-9). The next
    /// [`Terminal::compose`] paints accordingly, repainting only what the
    /// change damaged; the picture is that of a scene committed with those
    /// values. A state that sets nothing clears the drawing's state.
    ///
    /// At most [`MAX_ANIMATED_DRAWINGS`](crate::limits::MAX_ANIMATED_DRAWINGS)
    /// drawings hold state at once. It returns false when the state was
    /// not kept as given: the drawing does not exist or the table is full
    /// (nothing is kept), or showing a drawing the stream hid would take
    /// one repaint over [`MAX_REBUILD_WORK`](crate::limits::MAX_REBUILD_WORK)
    /// (the state is kept without its shown part).
    ///
    /// The last writer wins: a commit that sets a drawing's `X` or `Y`
    /// forgets the state's position, `V` its shown part, `L` or `G` its
    /// layer and order, and a deletion the whole state.
    /// [`Terminal::animation`] says what is held.
    pub fn set_animation(&mut self, number: u16, state: AnimationState) -> bool {
        self.scene.set_animation(number, state)
    }

    /// Forgets the animation state of a numbered drawing: it is painted
    /// as the stream committed it.
    pub fn clear_animation(&mut self, number: u16) {
        self.scene.clear_animation(number);
    }

    /// The animation state a numbered drawing holds; `None` when it holds
    /// none. A receiver reads it after a commit to learn what the stream
    /// wrote over.
    #[must_use]
    pub fn animation(&self, number: u16) -> Option<AnimationState> {
        self.scene.animation(number)
    }

    /// What the stream committed for a numbered drawing: its attributes
    /// `V` (shown), `G` (layer) and `L` (depth order), whatever animation
    /// state it holds. `None` when the drawing does not exist, so that it
    /// also answers whether one does. With [`Terminal::animation`] it
    /// gives a rule table a target's shown part, layer and order
    /// (neoteletext-common `docs/page-rules.md`, RULE-26): each is the
    /// animation state's where that sets it, else the one here. Operations
    /// still waiting for their commit are not counted.
    #[must_use]
    pub fn committed_drawing(&self, number: u16) -> Option<CommittedDrawing> {
        self.scene.committed_drawing(number)
    }

    /// The label a screen reader speaks for numbered drawing `number`, as
    /// the last successful commit left it: the attribute `U` (DEC-6638
    /// item 6, `docs/regis-v2.md`, "Attributes"). `None` when the drawing
    /// does not exist or has no label. A label is never drawn; the caller
    /// gives it to the screen-reader tree with the drawing's rectangle
    /// ([`Terminal::bounds`]). Its bytes, which a v1 quoted string carries,
    /// are read as ISO/IEC 8859-1.
    #[must_use]
    pub fn label(&self, number: u16) -> Option<String> {
        self.scene.label(number)
    }

    /// Every committed numbered drawing that has a label, in order of
    /// number, with its label.
    #[must_use]
    pub fn labels(&self) -> Vec<(u16, String)> {
        self.scene.labels()
    }

    /// The picture's title, as the last successful commit left it:
    /// `D(T'...')`, the name a page list shows for the picture when the
    /// program's index gives none (DEC-6638 item 3, `docs/regis-v2.md`,
    /// "Title"). `None` when it has none. A title is never drawn.
    #[must_use]
    pub fn title(&self) -> Option<String> {
        self.scene.title()
    }

    /// The lowest-numbered top-level drawing that can write on the
    /// screen as the state stands: it is shown, and its contents, or
    /// those of a shown drawing below it or of the look or frame in
    /// force, were measured at their commit to reach a pixel of the
    /// screen inside every clip rectangle ([`Terminal::bounds`] is not
    /// `None`). `None` when the numbered drawings leave the picture as
    /// the base alone made it. A sprite sheet must leave none: it
    /// defines hidden masters and draws nothing itself (neoteletext-common
    /// `docs/animation.md`, ANIM-17).
    #[must_use]
    pub fn visible_drawing(&self) -> Option<u16> {
        self.scene.visible_drawing()
    }

    /// The position in force of a numbered drawing, x then y, in pixels
    /// relative to its parent: its animation state's, else its attributes
    /// `X` and `Y`. `None` when the drawing does not exist. It is what a
    /// relative move adds to (ANIM-6).
    #[must_use]
    pub fn drawing_position(&self, number: u16) -> Option<(i16, i16)> {
        self.scene.drawing_position(number)
    }

    /// The bounding rectangle of a numbered drawing in the picture's
    /// pixels, as its state stands: where its contents were measured to
    /// reach at its commit, with its shown children and the look or frame
    /// in force, moved to its screen position and cut by its clip
    /// rectangles and the screen (ANIM-13). `None` while the drawing does
    /// not exist, is not shown, or reaches no pixel of the screen. Every
    /// pixel the drawing writes lies inside it. A drawing still being
    /// painted gradually has the rectangle it will have when finished.
    #[must_use]
    pub fn bounds(&self, number: u16) -> Option<Rectangle> {
        self.scene.bounds(number)
    }

    /// Says that `ticks` of the presentation clock, 60 to the second,
    /// have passed (ANIM-1, ANIM-15). Each drawing being painted
    /// gradually, by its paint rate `T`, has then earned `ticks × rate ×
    /// 100 ÷ 60` work units since its painting started, counted from the
    /// total of its ticks; the next [`Terminal::compose`] paints what
    /// that buys. It returns true while a shown drawing is still being
    /// painted, so that a caller knows to call again; `advance_ticks(0)`
    /// only asks. A caller that wants every drawing whole at once passes
    /// `u32::MAX`.
    pub fn advance_ticks(&mut self, ticks: u32) -> bool {
        self.scene.advance_ticks(ticks)
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
