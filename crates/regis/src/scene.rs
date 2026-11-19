//! ReGIS v2: numbered drawings, kept here and painted over the base
//! picture (`docs/regis-v2.md`).
//!
//! `D` operations change a staged copy of the scene; a commit checks the
//! staged copy against every cap and makes it the committed scene, or
//! fails and throws it away. The committed scene is painted when the
//! caller asks for a picture.

mod animation;
mod capture;
mod commit;
mod paint;
mod picture;
mod stage;

use alloc::collections::{BTreeMap, BTreeSet};
use alloc::vec::Vec;

use animation::Written;
pub use animation::{AnimationState, CommittedDrawing, ForcedLook, Rectangle};
use capture::{Capture, Captured};
pub use picture::{Pick, Picture};
use stage::Stage;
pub(crate) use stage::{Drawing, Extras};

use crate::event::{CommitError, Event};
use crate::font::{Font, Loadable};
use crate::limits::{
    MAX_ANIMATED_DRAWINGS, MAX_DRAWING_CONTENT_BYTES, MAX_STAGED_BYTES, MOST_DRAWINGS_HELD, held_allocation,
    ordered_map_entry_bytes, ordered_map_root_bytes,
};
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

/// The memory one drawing's entry holds in the receiver's record of the
/// drawings being painted gradually and of those whose painting is to
/// start.
const PAINTING_ENTRY_BYTES: usize = ordered_map_entry_bytes(size_of::<u16>(), size_of::<u64>())
    .saturating_add(ordered_map_entry_bytes(size_of::<u16>(), 0));
/// The memory one animated drawing's entry holds in the animation state
/// and in an open batch's record of what it wrote over.
const ANIMATED_ENTRY_BYTES: usize = ordered_map_entry_bytes(size_of::<u16>(), size_of::<AnimationState>())
    .saturating_add(ordered_map_entry_bytes(size_of::<u16>(), size_of::<Written>()));

/// The most the receiver's per-drawing painting state can hold: every
/// drawing a scene can hold being painted, and every animated drawing.
pub(crate) const fn most_painting_state_bytes() -> usize {
    MOST_DRAWINGS_HELD
        .saturating_mul(PAINTING_ENTRY_BYTES)
        .saturating_add(MAX_ANIMATED_DRAWINGS.saturating_mul(ANIMATED_ENTRY_BYTES))
}

/// The roots of the scene's maps, which [`ordered_map_entry_bytes`] does
/// not count: two for each scene, four for the painting and animation
/// state, two for a commit's checks, and the empty contents each scene
/// shares, each at most the root of the largest map.
pub(crate) const MAP_ROOTS_BYTES: usize = 12 * ordered_map_root_bytes(size_of::<u16>(), size_of::<AnimationState>())
    + 2 * held_allocation(2 * size_of::<usize>());

/// The operations waiting for a commit.
#[derive(Debug)]
struct Batch {
    /// The first thing wrong with it: it fails at its commit.
    failure: Option<(CommitError, u16)>,
    /// The output map as the batch found it: shown until the commit, and
    /// put back if the commit fails.
    palette: Palette,
    /// What its operations wrote over in the animation state.
    written: BTreeMap<u16, Written>,
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
    /// Counts the changes to what every drawing's pixels depend on beside
    /// its own contents: the committed tables and the font. A picture
    /// painted under another count is repainted whole.
    tables_version: u64,
    /// Replays drawings' contents; it holds the committed tables.
    player: Interpreter,
    viewer: ViewerState,
    /// The receiver's animation state, by drawing: at most
    /// [`MAX_ANIMATED_DRAWINGS`](crate::limits::MAX_ANIMATED_DRAWINGS).
    animation: BTreeMap<u16, AnimationState>,
    /// The ticks each drawing being painted gradually has been painting
    /// (DEC-6382). A drawing not here is painted whole.
    painting: BTreeMap<u16, u64>,
    /// The drawings whose painting is to start, with the drawings below
    /// them, when time next passes or a picture is made.
    starting: BTreeSet<u16>,
    /// Whether any committed drawing has a paint rate.
    rated: bool,
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
            tables_version: 0,
            player,
            viewer: ViewerState::default(),
            animation: BTreeMap::new(),
            painting: BTreeMap::new(),
            starting: BTreeSet::new(),
            rated: false,
        }
    }

    /// Sets the viewer state. False when the picture certainly stays as it
    /// is: no drawing named, before or after, is painted differently for
    /// it.
    pub(crate) fn set_viewer_state(&mut self, state: ViewerState) -> bool {
        let old = core::mem::replace(&mut self.viewer, state);
        let exists = |number: u16| number != 0 && self.committed.get(number).is_some();
        let look = |number: u16, pick: fn(&Drawing) -> u16| {
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
        // Where text reaches depends on the shapes of its characters.
        self.committed.forget_measurements();
        self.staged.forget_measurements();
        self.tables_version = self.tables_version.wrapping_add(1);
    }

    pub(crate) fn set_provided_set(&mut self, number: usize, set: crate::provided::ProvidedSet) {
        self.player.machine.set_provided_set(number, set);
        // As a font does: the shapes decide where text reaches.
        self.committed.forget_measurements();
        self.staged.forget_measurements();
        self.tables_version = self.tables_version.wrapping_add(1);
    }

    /// The memory the scene holds for what the stream defined, at most:
    /// the committed scene, what the staged scene does not share with it,
    /// a definition being received, the receiver's painting and animation
    /// state, the replaying interpreter's working state with the font and
    /// provided sets it was given, and the committed copy of the
    /// macrographs (SOLVEDSECURITYISSUES 015). Its structure and the maps'
    /// roots are the caller's to count.
    pub(crate) fn held_bytes(&self) -> usize {
        let painting = self.painting.len().saturating_add(self.starting.len()).saturating_mul(PAINTING_ENTRY_BYTES);
        let written = self.batch.as_ref().map_or(0, |batch| batch.written.len());
        let animated = self.animation.len().saturating_add(written).saturating_mul(ANIMATED_ENTRY_BYTES);
        let capture = self.capture.as_ref().map_or(0, |capture| held_allocation(capture.held_bytes()));
        self.committed
            .held_bytes()
            .saturating_add(self.staged.held_apart_from(&self.committed))
            .saturating_add(capture)
            .saturating_add(painting)
            .saturating_add(animated)
            .saturating_add(self.player.machine.characters.given_heap_bytes())
            .saturating_add(self.player.heap_bytes())
            .saturating_add(self.tables.macrographs.heap_bytes())
            .saturating_add(self.tables.sets.iter().map(Loadable::heap_bytes).fold(0, usize::saturating_add))
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
        self.batch.get_or_insert_with(|| Batch {
            failure: None,
            palette: base.palette().clone(),
            written: BTreeMap::new(),
        })
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
            Operation::Create(drawing) => {
                if let Err(error) = self.staged.create(drawing) {
                    self.fail(base, error, drawing);
                }
            }
            Operation::Set(drawing, attribute) => match self.staged.set(drawing, attribute) {
                Ok(()) => self.note_written(drawing, attribute),
                Err(error) => self.fail(base, error, drawing),
            },
            Operation::Delete(drawing) => {
                self.staged.delete(drawing);
                self.note_deleted();
            }
            Operation::DeleteAll => {
                // `D(Z)` deletes drawings; the title is the picture's, not a
                // drawing's, and stays.
                let title = self.staged.title;
                self.staged = Stage::default();
                self.staged.title = title;
                self.note_deleted();
            }
            Operation::Title(title) => self.staged.title = title,
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
        let stored = contents.map_or(Err(CommitError::Contents), |contents| self.staged.define(target, contents));
        if let Err(error) = stored {
            batch.failure.get_or_insert((error, target));
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
