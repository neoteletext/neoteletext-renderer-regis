//! The animation state: what a receiver sets on numbered drawings to
//! move, show, hide, reorder and change them with no byte of the stream
//! arriving (neoteletext-common `docs/animation.md`, ANIM-6 and ANIM-9),
//! and the paint rate, which is played on the receiver's ticks (DEC-6382;
//! ANIM-15).

use alloc::collections::BTreeMap;
use alloc::vec::Vec;

use super::Scene;
use super::paint::Walk;
use super::stage::{Contents, Stage};
use crate::limits::{
    LAYERS, MAX_ANIMATED_DRAWINGS, MAX_DRAWING_DEPTH, MAX_OBJECT_FRAMES, PAINT_RATE_UNIT, TICKS_PER_SECOND,
};
use crate::machine::Attribute;
use crate::surface::Bounds;

/// The look a receiver forces on a drawing whatever the viewer's pointer
/// is doing (ANIM-6, "forced look").
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ForcedLook {
    /// None: the pointer decides the look.
    #[default]
    None,
    /// The hover look, the drawing its attribute `H` names.
    Hover,
    /// The pressed look, the drawing its attribute `A` names.
    Pressed,
    /// The alternative look, the drawing its attribute `N` names.
    Alternative,
}

impl ForcedLook {
    /// The look an operation's byte names: 0 none, 1 hover, 2 pressed, 3
    /// alternative (ANIM-6). `None` for any other value.
    #[must_use]
    pub const fn from_code(code: u8) -> Option<Self> {
        Some(match code {
            0 => Self::None,
            1 => Self::Hover,
            2 => Self::Pressed,
            3 => Self::Alternative,
            _ => return None,
        })
    }

    /// The byte that names this look: 0 to 3.
    #[must_use]
    pub const fn code(self) -> u8 {
        match self {
            Self::None => 0,
            Self::Hover => 1,
            Self::Pressed => 2,
            Self::Alternative => 3,
        }
    }
}

/// The animation state of one numbered drawing (ANIM-6, ANIM-9). Each
/// part that is set is used in place of the attribute the stream
/// committed; the picture is then, pixel for pixel, that of a scene
/// committed with those values. It is the receiver's alone: no byte of the
/// stream can read or set it.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct AnimationState {
    /// Shown or hidden, in place of the attribute `V`; `None` for the
    /// drawing's own.
    pub shown: Option<bool>,
    /// The position, x then y, in pixels relative to the drawing's parent,
    /// in place of the attributes `X` and `Y`; `None` for the drawing's
    /// own.
    pub position: Option<(i16, i16)>,
    /// The layer, 0 to 3, in place of the attribute `G`; `None` for the
    /// drawing's own. A value above 3 is taken as 3. It takes effect on a
    /// top-level drawing.
    pub layer: Option<u8>,
    /// The order within the layer, or within the parent, in place of the
    /// depth attribute `L`; `None` for the drawing's own.
    pub order: Option<i8>,
    /// The frame in force, 0 to 15: which entry of the drawing's frame
    /// list `F(...)` is painted in its place. An entry beyond the list is
    /// its last; a drawing with no frame list ignores it.
    pub frame: u8,
    /// The forced look.
    pub look: ForcedLook,
}

impl AnimationState {
    /// The state with its values brought inside their ranges.
    fn bounded(mut self) -> Self {
        let top = LAYERS.saturating_sub(1);
        self.layer = self.layer.map(|layer| layer.min(top));
        let last = u8::try_from(MAX_OBJECT_FRAMES.saturating_sub(1)).unwrap_or(u8::MAX);
        self.frame = self.frame.min(last);
        self
    }
}

/// A rectangle of a picture's pixels: what [`Terminal::bounds`] gives
/// (ANIM-13). It lies inside the picture of 800 by 480.
///
/// [`Terminal::bounds`]: crate::Terminal::bounds
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Rectangle {
    /// The leftmost column.
    pub x: u32,
    /// The top row.
    pub y: u32,
    /// The columns, at least 1.
    pub width: u32,
    /// The rows, at least 1.
    pub height: u32,
}

impl Rectangle {
    fn of(bounds: Bounds) -> Option<Self> {
        let bounds = bounds.within(Bounds::page())?;
        let side = |low: i32, high: i32| high.saturating_sub(low).saturating_add(1).unsigned_abs();
        Some(Self {
            x: bounds.left.unsigned_abs(),
            y: bounds.top.unsigned_abs(),
            width: side(bounds.left, bounds.right),
            height: side(bounds.top, bounds.bottom),
        })
    }
}

/// The parts of a drawing's animation state that a batch's own operations
/// wrote over: the last writer wins (ANIM-6, "One state, last writer
/// wins").
#[derive(Clone, Copy, Debug)]
pub(super) enum Written {
    /// Some of its parts: the position, the shown part, and the layer and
    /// order.
    Parts { position: bool, shown: bool, order: bool },
    /// The drawing was deleted: its whole state goes.
    All,
}

impl Default for Written {
    fn default() -> Self {
        Self::Parts { position: false, shown: false, order: false }
    }
}

/// What painting gradually depends on, for one drawing with a paint rate,
/// before a change: compared after it to find the drawings whose painting
/// starts (DEC-6382 item 2).
pub(super) struct Rated {
    number: u16,
    contents: Contents,
    rate: u16,
    /// Shown, with every drawing up its parent chain.
    visible: bool,
}

/// The ticks after which `cost` work units have been painted at `rate`.
fn ticks_to_paint(cost: u64, rate: u16) -> u64 {
    let a_second = u64::from(rate).saturating_mul(PAINT_RATE_UNIT);
    cost.saturating_mul(TICKS_PER_SECOND).div_ceil(a_second.max(1))
}

/// The work units painted after `ticks` at `rate`: `ticks × rate × 100 ÷
/// 60`, rounded down, from the total of ticks so that nothing is lost to
/// rounding (ANIM-1, ANIM-15).
pub(super) fn painted_after(ticks: u64, rate: u16) -> u64 {
    let units = ticks.saturating_mul(u64::from(rate)).saturating_mul(PAINT_RATE_UNIT);
    units.checked_div(TICKS_PER_SECOND).unwrap_or(0)
}

/// Whether a drawing is shown with every drawing up its parent chain,
/// under the animation state.
fn visible_up_the_chain(stage: &Stage, animation: &BTreeMap<u16, AnimationState>, number: u16) -> bool {
    let mut at = number;
    // The commit bounded the chain; a longer one is a fault there.
    for _ in 0..=MAX_DRAWING_DEPTH {
        let Some(drawing) = stage.get(at) else { return false };
        let shown = animation.get(&at).and_then(|state| state.shown).unwrap_or(drawing.visible);
        if !shown {
            return false;
        }
        if drawing.parent == 0 {
            return true;
        }
        at = drawing.parent;
    }
    false
}

impl Scene {
    fn walk(&self) -> Walk<'_> {
        Walk { stage: &self.committed, viewer: self.viewer, animation: &self.animation, painting: None }
    }

    /// Sets a drawing's animation state. False when it was not kept as
    /// given: the drawing does not exist, 64 drawings already hold state
    /// (and then nothing is kept), or showing a drawing the stream hid
    /// would take a repaint over the cap on one repaint (and then the
    /// state is kept without its shown part).
    pub(crate) fn set_animation(&mut self, number: u16, state: AnimationState) -> bool {
        let state = state.bounded();
        if state == AnimationState::default() {
            let existed = self.committed.get(number).is_some();
            self.clear_animation(number);
            return existed;
        }
        let full = self.animation.len() >= MAX_ANIMATED_DRAWINGS && !self.animation.contains_key(&number);
        if self.committed.get(number).is_none() || full {
            return false;
        }
        let was_visible = self.shown_up_the_chain(number);
        self.animation.insert(number, state);
        let mut kept = true;
        if state.shown == Some(true) && !self.animation_work_fits() {
            // The commit's check of work did not count this drawing shown
            // (SOLVEDSECURITYISSUES 014).
            if let Some(state) = self.animation.get_mut(&number) {
                state.shown = None;
            }
            self.forget_empty_states();
            kept = false;
        }
        self.note_shown(number, was_visible);
        kept
    }

    /// Forgets a drawing's animation state: it is painted as committed.
    pub(crate) fn clear_animation(&mut self, number: u16) {
        if !self.animation.contains_key(&number) {
            return;
        }
        let was_visible = self.shown_up_the_chain(number);
        self.animation.remove(&number);
        self.note_shown(number, was_visible);
    }

    /// Whether a drawing is shown with every drawing up its parent chain.
    fn shown_up_the_chain(&self, number: u16) -> bool {
        visible_up_the_chain(&self.committed, &self.animation, number)
    }

    /// A showing of a drawing by the animation state starts its painting,
    /// and that of the drawings below it, as a commit that made it
    /// visible does (ANIM-15). It is only noted here, and the drawings
    /// are found once for all that were noted, when time next passes or
    /// a picture is made: a receiver may show many drawings in one tick,
    /// and the walk is of the whole scene.
    fn note_shown(&mut self, number: u16, was_visible: bool) {
        if self.rated && !was_visible && self.shown_up_the_chain(number) {
            self.starting.insert(number);
        }
    }

    /// The animation state a drawing holds.
    pub(crate) fn animation(&self, number: u16) -> Option<AnimationState> {
        self.animation.get(&number).copied()
    }

    /// The position in force of a drawing, relative to its parent: its
    /// animation state's, else its attributes `X` and `Y`.
    pub(crate) fn drawing_position(&self, number: u16) -> Option<(i16, i16)> {
        let drawing = self.committed.get(number)?;
        let narrow = |value: i32| i16::try_from(value.clamp(i32::from(i16::MIN), i32::from(i16::MAX))).unwrap_or(0);
        let own = (narrow(drawing.x), narrow(drawing.y));
        Some(self.animation.get(&number).and_then(|state| state.position).unwrap_or(own))
    }

    /// A drawing's bounding rectangle on the screen (ANIM-13).
    pub(crate) fn bounds(&self, number: u16) -> Option<Rectangle> {
        self.walk().bounds(number).and_then(Rectangle::of)
    }

    /// Notes what a `D` operation of the open batch writes over in the
    /// animation state; it is forgotten if the batch's commit succeeds.
    pub(super) fn note_written(&mut self, drawing: u16, attribute: Attribute) {
        if !self.animation.contains_key(&drawing) {
            return;
        }
        let Some(batch) = &mut self.batch else { return };
        let Written::Parts { position, shown, order } = batch.written.entry(drawing).or_default() else { return };
        match attribute {
            Attribute::X(_) | Attribute::Y(_) => *position = true,
            Attribute::Visible(_) => *shown = true,
            Attribute::Depth(_) | Attribute::Layer(_) => *order = true,
            _ => {}
        }
    }

    /// Notes the animated drawings a deletion of the open batch removed:
    /// their state is forgotten at the commit even if the batch makes a
    /// drawing of the same number again.
    pub(super) fn note_deleted(&mut self) {
        let Some(batch) = &mut self.batch else { return };
        for number in self.animation.keys() {
            if self.staged.get(*number).is_none() {
                batch.written.insert(*number, Written::All);
            }
        }
    }

    /// After a commit: the last writer wins. The parts of the animation
    /// state the batch wrote over are forgotten, with the state of every
    /// drawing that no longer exists; a state that would now show more
    /// than the cap on one repaint allows loses its shown part; and the
    /// drawings whose painting starts are found.
    pub(super) fn committed_over_animation(&mut self, written: &BTreeMap<u16, Written>, before: &[Rated]) {
        for (number, written) in written {
            let Some(state) = self.animation.get_mut(number) else { continue };
            let Written::Parts { position, shown, order } = *written else {
                *state = AnimationState::default();
                continue;
            };
            if position {
                state.position = None;
            }
            if shown {
                state.shown = None;
            }
            if order {
                state.layer = None;
                state.order = None;
            }
        }
        let committed = &self.committed;
        self.animation.retain(|number, _| committed.get(*number).is_some());
        self.forget_empty_states();
        while !self.animation_work_fits() {
            let Some(state) = self.animation.values_mut().rev().find(|state| state.shown == Some(true)) else { break };
            state.shown = None;
        }
        self.forget_empty_states();
        self.rated = self.committed.drawings().any(|(_, drawing)| drawing.rate != 0);
        let committed = &self.committed;
        self.painting.retain(|number, _| committed.get(*number).is_some_and(|drawing| drawing.rate != 0));
        self.starting.retain(|number| committed.get(*number).is_some());
        self.start_painting(before);
    }

    /// A state that sets nothing is no state: its place is freed.
    fn forget_empty_states(&mut self) {
        self.animation.retain(|_, state| *state != AnimationState::default());
    }

    /// Whether painting the scene with the drawings the animation state
    /// shows costs no more than one repaint may.
    fn animation_work_fits(&mut self) -> bool {
        if !self.animation.values().any(|state| state.shown == Some(true)) {
            return true;
        }
        // The check reads the staged scene and keeps its measurements
        // there, so the committed scene stands in for it meanwhile.
        core::mem::swap(&mut self.staged, &mut self.committed);
        let animation = core::mem::take(&mut self.animation);
        let fits = self.check_work(&animation).is_ok();
        self.animation = animation;
        core::mem::swap(&mut self.staged, &mut self.committed);
        fits
    }

    /// The drawings with a paint rate, as they stand.
    pub(super) fn rated(&self) -> Vec<Rated> {
        if !self.rated {
            return Vec::new();
        }
        let rated = self.committed.drawings().filter(|(_, drawing)| drawing.rate != 0);
        rated
            .map(|(number, drawing)| Rated {
                number,
                contents: drawing.contents.clone(),
                rate: drawing.rate,
                visible: visible_up_the_chain(&self.committed, &self.animation, number),
            })
            .collect()
    }

    /// Notes the drawings with a paint rate whose painting starts at a
    /// commit: each that is new, whose contents or rate changed, or that
    /// became visible (DEC-6382 item 2). A drawing that only moved,
    /// changed parent, clip or depth goes on as it was.
    fn start_painting(&mut self, before: &[Rated]) {
        if !self.rated {
            self.starting.clear();
            return;
        }
        for (number, drawing) in self.committed.drawings().filter(|(_, drawing)| drawing.rate != 0) {
            let was = before.binary_search_by_key(&number, |rated| rated.number).ok().and_then(|at| before.get(at));
            let starts = was.is_none_or(|was| {
                !Contents::ptr_eq(&was.contents, &drawing.contents)
                    || was.rate != drawing.rate
                    || (!was.visible && visible_up_the_chain(&self.committed, &self.animation, number))
            });
            if starts {
                self.starting.insert(number);
            }
        }
        self.settle_starting();
    }

    /// Starts the painting of every drawing noted, and of the drawings
    /// with a paint rate below each: they wait for it, then start in
    /// their turn (DEC-6382 item 3). One walk of the scene, however many
    /// were noted.
    pub(super) fn settle_starting(&mut self) {
        if self.starting.is_empty() {
            return;
        }
        let starting = core::mem::take(&mut self.starting);
        // Each entry: a drawing, and whether a drawing above it starts.
        let mut walk: Vec<(u16, bool)> = self.committed.child_numbers(0).map(|number| (number, false)).collect();
        while let Some((number, under)) = walk.pop() {
            let Some(drawing) = self.committed.get(number) else { continue };
            let starts = under || starting.contains(&number);
            if starts && drawing.rate != 0 {
                self.painting.insert(number, 0);
            }
            walk.extend(self.committed.child_numbers(number).map(|child| (child, starts)));
        }
        // A drawing that costs nothing to paint is finished at once.
        let _ = self.advance_ticks(0);
    }

    /// Tells the scene that ticks of the presentation clock have passed:
    /// every drawing being painted gradually earns `ticks × rate × 100 ÷
    /// 60` work units more, and a drawing that finishes lets its children
    /// start with the ticks left over. True while a drawing that is shown
    /// is still being painted.
    pub(crate) fn advance_ticks(&mut self, ticks: u32) -> bool {
        self.settle_starting();
        if self.painting.is_empty() {
            return false;
        }
        let mut painting = core::mem::take(&mut self.painting);
        let walk = self.walk();
        let mut still = false;
        for number in self.committed.child_numbers(0) {
            still |= walk.advance(&mut painting, number, u64::from(ticks), MAX_DRAWING_DEPTH);
        }
        self.painting = painting;
        still
    }
}

impl Walk<'_> {
    /// Advances the painting of one drawing and of what is below it.
    /// True while something shown is still being painted.
    fn advance(self, painting: &mut BTreeMap<u16, u64>, number: u16, ticks: u64, depth: usize) -> bool {
        let Some(drawing) = self.stage.get(number) else { return false };
        let Some(depth) = depth.checked_sub(1) else { return false };
        // A hidden drawing does not paint: it starts when it is shown.
        if !self.visible(number, drawing) {
            return false;
        }
        let mut left = ticks;
        if let Some(painted) = painting.get_mut(&number) {
            let needed = ticks_to_paint(self.own_cost(number, drawing), drawing.rate);
            let total = painted.saturating_add(ticks);
            if drawing.rate != 0 && total < needed {
                *painted = total;
                return true;
            }
            left = total.saturating_sub(needed).min(ticks);
            painting.remove(&number);
        }
        let mut still = false;
        for child in self.stage.child_numbers(number) {
            still |= self.advance(painting, child, left, depth);
        }
        still
    }
}
