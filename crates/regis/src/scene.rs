//! ReGIS v2: numbered drawings, kept here and painted over the base
//! picture (`docs/regis-v2.md`).
//!
//! `D` operations change a staged copy of the scene; a commit checks the
//! staged copy against every cap and makes it the committed scene, or
//! fails and throws it away. The committed scene is painted when the
//! caller asks for a picture.

mod capture;
mod commit;
mod paint;
mod picture;
mod stage;

use alloc::vec::Vec;

use capture::{Capture, Captured};
pub use picture::{Pick, Picture};
use stage::Stage;

use crate::event::{CommitError, Event};
use crate::font::{Font, Loadable};
use crate::limits::{MAX_DRAWING_CONTENT_BYTES, MAX_STAGED_BYTES};
use crate::machine::{LOADABLE_SETS, Machine, Operation, Role};
use crate::macrograph::Macrographs;
use crate::palette::Palette;
use crate::terminal::Interpreter;

/// The tables numbered drawings share with the base: the macrographs and
/// the loadable character sets, as they stood at the last commit
/// (`docs/regis-v2.md`, "Rules for contents", rule 4).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
struct Tables {
    macrographs: Macrographs,
    sets: [Loadable; LOADABLE_SETS],
}

impl Tables {
    fn of(machine: &Machine) -> Self {
        Self { macrographs: machine.macrographs.clone(), sets: machine.characters.sets.clone() }
    }
}

/// What the viewer's pointer is doing to the picture, as the program that
/// shows it has found from its own pick record. It changes how numbered
/// drawings are painted and is the caller's alone: no byte of the stream
/// can read or set it (`docs/regis-v2.md`, "Viewer state").
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ViewerState {
    /// The drawing the pointer is over, 0 for none: it is painted with its
    /// hover look, if it has one.
    pub hovered: u16,
    /// The drawing a button or a contact is pressed on, 0 for none: it is
    /// painted with its pressed look, if it has one, which wins over the
    /// hover look.
    pub pressed: u16,
    /// A drawing not painted, with its children, 0 for none: how a
    /// receiver blinks a cursor the stream drew.
    pub hidden: u16,
}

/// The operations waiting for a commit.
#[derive(Debug)]
struct Batch {
    /// The first thing wrong with it: it fails at its commit.
    failure: Option<(CommitError, u16)>,
    /// The output map as the batch found it: shown until the commit, and
    /// put back if the commit fails.
    palette: Palette,
}

/// Every numbered drawing, and what is being done to them.
#[derive(Debug)]
pub(crate) struct Scene {
    /// What is on show.
    committed: Stage,
    /// What the operations since the last commit have made of it.
    staged: Stage,
    batch: Option<Batch>,
    /// Bytes of `D` commands and definitions read since the last commit.
    staged_bytes: usize,
    /// The definition being received.
    capture: Option<Capture>,
    commits: u32,
    /// The committed copy of the shared tables.
    tables: Tables,
    /// Replays drawings' contents; it holds the committed tables.
    player: Interpreter,
    viewer: ViewerState,
}

impl Scene {
    pub(crate) fn new() -> Self {
        let mut player = Interpreter::new();
        player.machine.role = Role::Drawing;
        Self {
            committed: Stage::default(),
            staged: Stage::default(),
            batch: None,
            staged_bytes: 0,
            capture: None,
            commits: 0,
            tables: Tables::default(),
            player,
            viewer: ViewerState::default(),
        }
    }

    /// Sets the viewer state. False when the picture certainly stays as it
    /// is: no drawing named, before or after, is painted differently for
    /// it.
    pub(crate) fn set_viewer_state(&mut self, state: ViewerState) -> bool {
        let old = core::mem::replace(&mut self.viewer, state);
        let exists = |number: u16| number != 0 && self.committed.get(number).is_some();
        let look = |number: u16, pick: fn(&stage::Drawing) -> u16| {
            self.committed.get(number).is_some_and(|drawing| exists(pick(drawing)))
        };
        let hover = |number: u16| look(number, |drawing| drawing.hover);
        let pressed = |number: u16| look(number, |drawing| drawing.pressed);
        let hovered = old.hovered != state.hovered && (hover(old.hovered) || hover(state.hovered));
        // Releasing a drawing with no pressed look uncovers its hover look.
        let either = |number: u16| pressed(number) || hover(number);
        let pressing = old.pressed != state.pressed && (either(old.pressed) || either(state.pressed));
        let hiding = old.hidden != state.hidden && (exists(old.hidden) || exists(state.hidden));
        hovered || pressing || hiding
    }

    pub(crate) fn set_font(&mut self, font: Font) {
        self.player.machine.set_font(font);
    }

    pub(crate) const fn commits(&self) -> u32 {
        self.commits
    }

    pub(crate) fn any_committed(&self) -> bool {
        !self.committed.is_empty()
    }

    /// The output map to show while a batch is open.
    pub(crate) fn held_palette(&self) -> Option<&Palette> {
        self.batch.as_ref().map(|batch| &batch.palette)
    }

    /// Whether the bytes arriving are a drawing's contents.
    pub(crate) const fn defining(&self) -> bool {
        self.capture.is_some()
    }

    /// Counts bytes of `D` commands against the staged operations cap.
    pub(crate) const fn count_staged(&mut self, bytes: usize) {
        self.staged_bytes = self.staged_bytes.saturating_add(bytes);
    }

    /// The batch the operations now belong to, opened by the first of them
    /// after a commit.
    fn batch(&mut self, base: &Machine) -> &mut Batch {
        self.batch.get_or_insert_with(|| Batch { failure: None, palette: base.palette().clone() })
    }

    /// Notes the first thing wrong with the batch.
    fn fail(&mut self, base: &Machine, error: CommitError, drawing: u16) {
        let batch = self.batch(base);
        batch.failure.get_or_insert((error, drawing));
    }

    /// Carries out one `D` operation on the staged scene.
    pub(crate) fn apply(&mut self, operation: Operation, base: &mut Machine) {
        let _ = self.batch(base);
        match operation {
            Operation::Begin(target) => {
                if target.is_none() {
                    self.fail(base, CommitError::Malformed, 0);
                }
                self.capture = Some(Capture::new(target));
            }
            // `D(E)` with no definition open.
            Operation::End => self.fail(base, CommitError::Malformed, 0),
            Operation::Create(drawing) => self.staged.create(drawing),
            Operation::Set(drawing, attribute) => self.staged.set(drawing, attribute),
            Operation::Delete(drawing) => self.staged.delete(drawing),
            Operation::DeleteAll => self.staged = Stage::default(),
            Operation::Malformed(drawing) => self.fail(base, CommitError::Malformed, drawing),
            Operation::Commit => {
                let event = match self.commit(base) {
                    Ok(()) => Event::Committed(self.commits),
                    Err((error, drawing)) => Event::CommitFailed { error, drawing },
                };
                base.outbox.push(event);
            }
        }
    }

    /// One byte of the contents of the drawing being defined.
    pub(crate) fn definition_byte(&mut self, byte: u8) {
        self.count_staged(1);
        let Some(capture) = &mut self.capture else { return };
        let Some(captured) = capture.push(byte) else { return };
        self.capture = None;
        let Captured { target, contents, nested } = captured;
        let Some(batch) = &mut self.batch else { return };
        if nested {
            // Another `D` operation before `D(E)`: definitions do not nest.
            batch.failure.get_or_insert((CommitError::Malformed, target.unwrap_or(0)));
        }
        let Some(target) = target else { return };
        let stored = contents.and_then(|contents| self.staged.define(target, contents));
        if stored.is_none() {
            batch.failure.get_or_insert((CommitError::Contents, target));
        }
    }

    /// Drops a definition left open when ReGIS mode ended; its batch fails.
    pub(crate) fn abandon_definition(&mut self) {
        if let Some(capture) = self.capture.take()
            && let Some(batch) = &mut self.batch
        {
            batch.failure.get_or_insert((CommitError::Malformed, capture.target().unwrap_or(0)));
        }
    }

    /// The bytes all drawings' contents may hold: enforced as they arrive.
    const fn contents_cap() -> usize {
        MAX_DRAWING_CONTENT_BYTES
    }

    /// The bytes of operations one commit may hold.
    const fn staged_cap() -> usize {
        MAX_STAGED_BYTES
    }
}

/// The drawings a list names, for errors and tests.
#[allow(dead_code, reason = "used by the tests of this module's parts")]
fn numbers(stage: &Stage) -> Vec<u16> {
    stage.numbers().collect()
}
