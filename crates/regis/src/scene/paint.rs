//! Painting the committed scene over the base picture
//! (`docs/regis-v2.md`, "Attributes", "Rules for contents", "Commit and
//! repaint" and "Picking").
//!
//! The scene is first laid out as a list of steps, each one drawing's
//! contents replayed at one place. A picture painted before keeps its
//! list, so the next one finds what changed and repaints only the
//! rectangles that hold it, each apart from the others: the same steps,
//! cut to each rectangle.

use alloc::collections::BTreeMap;
use alloc::vec::Vec;

use super::animation::{AnimationState, ForcedLook, painted_after};
use super::picture::{Painted, Picture};
use super::stage::{Contents, Drawing, Stage};
use super::{Scene, ViewerState};
use crate::limits::{
    DAMAGE_LOOKAHEAD_STEPS, DRAWING_BYTE_COST, DRAWING_VISIT_COST, MAX_DAMAGE_RECTANGLES, MAX_DAMAGE_STEPS,
    MAX_DRAWING_DEPTH, MAX_REBUILD_WORK,
};
use crate::palette::Palette;
use crate::surface::{Bounds, Screen, Window};
use crate::terminal::Interpreter;
use crate::work::Work;

/// Where a drawing is being painted.
#[derive(Clone, Copy)]
struct Place {
    /// The screen position of its origin: the positions up its parent
    /// chain added together. Sixteen levels of 16-bit positions fit `i32`.
    x: i32,
    y: i32,
    /// The pixels it may show in: the screen cut by every clip rectangle
    /// up the chain; `None` when they leave nothing.
    clip: Option<Bounds>,
    /// The drawing picking reports for the pixels it writes, 0 for none.
    pick: u16,
    /// It is being painted as part of a source: reuse gives appearance,
    /// not identity, so no drawing below is itself picked.
    reused: bool,
}

impl Place {
    /// The whole screen: where the top-level drawings are.
    fn screen() -> Self {
        Self { x: 0, y: 0, clip: Some(Bounds::page()), pick: 0, reused: false }
    }
}

/// One drawing's contents replayed at one place.
#[derive(Clone, Debug)]
pub(super) struct Step {
    contents: Contents,
    /// Where it lands and what it may change.
    window: Window,
    /// The pixels it can change: where its contents reach, moved to its
    /// place and cut by its clip rectangles. `None` if there are none.
    area: Option<Bounds>,
    /// The work its contents cost beside their bytes, as measured at the
    /// commit: what a paint rate pays for (DEC-6382).
    cost: u64,
    /// `Some` while the drawing is being painted gradually and this step
    /// is not yet whole: the work units of it painted so far.
    part: Option<u64>,
}

impl Step {
    /// Whether two steps paint the same pixels the same way: the same
    /// stored contents (not merely equal bytes), at the same place, under
    /// the same clip, for the same pick target, as far through.
    fn same(&self, other: &Self) -> bool {
        Contents::ptr_eq(&self.contents, &other.contents) && self.window == other.window && self.part == other.part
    }
}

/// The scene laid out for painting.
#[derive(Default)]
struct Layout {
    steps: Vec<Step>,
    /// The screen position of each detectable drawing painted.
    origins: BTreeMap<u16, (i32, i32)>,
    /// The drawings visited: each is paid for.
    visits: u64,
}

/// Where a walk of the scene hands what it finds.
struct Sink<'a> {
    step: &'a mut dyn FnMut(Step),
    origins: &'a mut BTreeMap<u16, (i32, i32)>,
    visits: &'a mut u64,
}

/// Replays contents under a window. `None` if the work allowed ran out.
fn play(player: &mut Interpreter, screen: &mut Screen, contents: &[u8], window: Window, allowed: u64) -> Option<u64> {
    reset(player);
    player.machine.work = Work::with(allowed);
    if let Some(canvas) = screen.page_mut(0) {
        canvas.set_window(window);
    }
    let mut complete = true;
    for byte in contents {
        // Each byte is paid for, as a byte out of a macrograph is.
        if !player.machine.work.spend(DRAWING_BYTE_COST) {
            complete = false;
            break;
        }
        player.command_byte(screen, *byte);
    }
    player.finish(screen);
    complete &= !player.machine.work.take_exhausted();
    // What a drawing asks of the caller (a pause, a reply) is not acted on.
    drop(player.machine.take_events());
    complete.then(|| allowed.saturating_sub(player.machine.work.left()))
}

/// A fresh drawing context: everything as at power-up but the shared
/// tables, the font and the output map (`docs/regis-v2.md`, "Rules for
/// contents", rule 1).
fn reset(player: &mut Interpreter) {
    player.forget_input();
    let mut fresh = crate::machine::Machine::new();
    fresh.role = crate::machine::Role::Drawing;
    core::mem::swap(&mut fresh.macrographs, &mut player.machine.macrographs);
    core::mem::swap(&mut fresh.palette, &mut player.machine.palette);
    core::mem::swap(&mut fresh.characters.font, &mut player.machine.characters.font);
    core::mem::swap(&mut fresh.characters.sets, &mut player.machine.characters.sets);
    player.machine = fresh;
}

/// The rectangle that holds every pixel in which two pages differ.
fn difference(old: &[u8], new: &[u8]) -> Option<Bounds> {
    let width = usize::try_from(Bounds::page().right).unwrap_or(0).saturating_add(1);
    let mut bounds = None;
    for (row, (old, new)) in old.chunks(width).zip(new.chunks(width)).enumerate() {
        if old == new {
            continue;
        }
        let differs = |(old, new): (&u8, &u8)| old != new;
        let first = old.iter().zip(new).position(differs).unwrap_or(0);
        let last = old.iter().zip(new).rposition(differs).unwrap_or(0);
        let side = |value: usize| i32::try_from(value).unwrap_or(i32::MAX);
        let run = Bounds { left: side(first), top: side(row), right: side(last), bottom: side(row) };
        bounds = Bounds::join(bounds, Some(run));
    }
    bounds
}

/// The rectangles that hold everything two lists of steps paint
/// differently. The steps of the two lists are matched in order, each
/// with one that is the same (`Step::same`): those matched paint the same
/// and in the same order relative to one another in both, so a pixel no
/// unmatched step can change comes out as it did before. Each unmatched
/// step's area is damage; steps far apart give rectangles far apart, so
/// that a key pressed at one side of a picture and a meter lit at the
/// other repaint those two places and not every drawing between them.
fn changed(old: &[Step], new: &[Step]) -> Damage {
    let mut damage = Damage::default();
    let (mut i, mut j) = (0, 0);
    while i < old.len() || j < new.len() {
        match (old.get(i), new.get(j)) {
            (Some(was), Some(now)) if was.same(now) => {
                i = i.saturating_add(1);
                j = j.saturating_add(1);
            }
            (Some(was), Some(now)) => {
                // A step added or taken away: the nearer of the two
                // matches within the look-ahead, else both changed.
                let ahead = |steps: &[Step], from: usize, step: &Step| {
                    steps.iter().skip(from).take(DAMAGE_LOOKAHEAD_STEPS).position(|other| other.same(step))
                };
                match (ahead(new, j, was), ahead(old, i, now)) {
                    (Some(added), removed) if removed.is_none_or(|removed| added <= removed) => {
                        for step in new.iter().skip(j).take(added) {
                            damage.add(step.area);
                        }
                        j = j.saturating_add(added);
                    }
                    (_, Some(removed)) => {
                        for step in old.iter().skip(i).take(removed) {
                            damage.add(step.area);
                        }
                        i = i.saturating_add(removed);
                    }
                    _ => {
                        damage.add(was.area);
                        damage.add(now.area);
                        i = i.saturating_add(1);
                        j = j.saturating_add(1);
                    }
                }
            }
            (Some(was), None) => {
                damage.add(was.area);
                i = i.saturating_add(1);
            }
            (None, Some(now)) => {
                damage.add(now.area);
                j = j.saturating_add(1);
            }
            (None, None) => break,
        }
    }
    damage
}

/// The rectangles of a picture to repaint: at most
/// `MAX_DAMAGE_RECTANGLES`, none meeting another; past that the two whose
/// joining adds least are joined.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(super) struct Damage {
    rectangles: Vec<Bounds>,
}

impl Damage {
    /// The whole picture.
    fn page() -> Self {
        Self { rectangles: alloc::vec![Bounds::page()] }
    }

    /// Adds an area, joining it with every rectangle it meets.
    fn add(&mut self, area: Option<Bounds>) {
        let Some(mut joined) = area.and_then(|area| area.within(Bounds::page())) else { return };
        // A joined rectangle can meet others it did not before.
        while let Some(index) = self.rectangles.iter().position(|held| held.within(joined).is_some()) {
            joined = joined.with(self.rectangles.swap_remove(index));
        }
        if self.rectangles.len() < MAX_DAMAGE_RECTANGLES {
            self.rectangles.push(joined);
            return;
        }
        let growth = |held: &Bounds| area_of(held.with(joined)).saturating_sub(area_of(*held));
        let Some(index) =
            self.rectangles.iter().enumerate().min_by_key(|(_, held)| growth(held)).map(|(index, _)| index)
        else {
            return;
        };
        let merged = self.rectangles.swap_remove(index).with(joined);
        self.add(Some(merged));
    }

    /// Adds every rectangle of another.
    fn join(&mut self, other: &Self) {
        for &area in &other.rectangles {
            self.add(Some(area));
        }
    }

    /// Joins every rectangles one step meets, until no step meets two:
    /// each step is then replayed for one rectangle at most, so that a
    /// repaint costs no more than painting the picture whole, whose every
    /// step is replayed once.
    fn once_per_step(&mut self, steps: &[Step]) {
        loop {
            let shared = steps.iter().find_map(|step| {
                let area = step.area?;
                let mut met = self.rectangles.iter().filter(|held| held.within(area).is_some());
                let first = *met.next()?;
                met.next().map(|second| first.with(*second))
            });
            let Some(joined) = shared else { return };
            self.add(Some(joined));
        }
    }

    /// The rectangle that holds them all, if any.
    fn bounds(&self) -> Option<Bounds> {
        self.rectangles.iter().fold(None, |bounds, area| Bounds::join(bounds, Some(*area)))
    }

    /// The rectangles.
    pub(super) fn rectangles(&self) -> &[Bounds] {
        &self.rectangles
    }
}

/// The pixels of a rectangle.
fn area_of(area: Bounds) -> u64 {
    let side = |low: i32, high: i32| u64::from(high.saturating_sub(low).saturating_add(1).unsigned_abs());
    side(area.left, area.right).saturating_mul(side(area.top, area.bottom))
}

/// Replays the first part of a drawing's contents: everything before the
/// first thing that `allowance` work units, not counting the bytes read,
/// cannot pay for (DEC-6382 items 1 and 5). The window's `whole_cost`
/// makes that the same part wherever the drawing lies and however much of
/// it is being repainted. The work it cost, bytes included.
fn play_part(player: &mut Interpreter, screen: &mut Screen, contents: &[u8], window: Window, allowance: u64) -> u64 {
    reset(player);
    player.machine.work = Work::strict(allowance);
    if let Some(canvas) = screen.page_mut(0) {
        canvas.set_window(window);
    }
    let mut read = 0_u64;
    for byte in contents {
        if player.machine.work.stopped() {
            break;
        }
        read = read.saturating_add(1);
        player.command_byte(screen, *byte);
    }
    if player.machine.work.stopped() {
        // Whatever was half read belongs to the part not yet painted.
        player.forget_input();
    } else {
        player.finish(screen);
    }
    let spent = allowance.saturating_sub(player.machine.work.left());
    let _ = player.machine.work.take_exhausted();
    drop(player.machine.take_events());
    read.saturating_mul(DRAWING_BYTE_COST).saturating_add(spent)
}

impl Scene {
    /// What a drawing's contents take to paint, wherever it is placed:
    /// they are replayed with nowhere to draw. `None` if that is more
    /// work than a whole repaint may cost.
    pub(super) fn measure(&mut self, contents: &[u8]) -> Option<super::stage::Measured> {
        let mut nowhere = Screen::absent();
        if let Some(canvas) = nowhere.page_mut(0) {
            canvas.measure_reach();
        }
        let window = Window { left: 0, right: -1, whole_cost: true, ..Window::page() };
        let work = play(&mut self.player, &mut nowhere, contents, window, MAX_REBUILD_WORK);
        let reach = nowhere.page_mut(0).and_then(crate::surface::Canvas::take_reach);
        work.map(|work| super::stage::Measured { work, reach })
    }

    /// Makes the picture: the base, then every visible drawing in drawing
    /// order, layer by layer. A picture this scene painted before is
    /// repainted only where the base, the drawings, the viewer state, the
    /// animation state or the progress of gradual painting changed it;
    /// the result is the same, pixel for pixel and in the pick record, as
    /// painting it whole (`docs/regis-v2.md`, "Commit and repaint").
    ///
    /// The drawings of layer 0 are painted over the base into a background
    /// raster the picture keeps, and the layers above over a copy of that,
    /// so a change above layer 0 replays nothing of layer 0 (animation.md,
    /// ANIM-12).
    pub(crate) fn compose(&mut self, base: &Screen, palette: &Palette, picture: &mut Picture) {
        self.player.machine.palette = palette.clone();
        self.settle_starting();
        let walk = Walk {
            stage: &self.committed,
            viewer: self.viewer,
            animation: &self.animation,
            painting: Some(&self.painting),
        };
        let top = walk.ordered(0);
        let mut layout = Layout::default();
        let mut listed = true;
        // The top-level drawings come layer by layer, so the background's
        // steps are the first of the list.
        let mut background = 0;
        for (layer, _, number) in &top {
            listed &= walk.lay_out(&mut layout, *number, MAX_DRAWING_DEPTH);
            if *layer == 0 {
                background = layout.steps.len();
            }
        }
        let Layout { steps, origins, visits } = layout;
        let base = base.displayed();
        let before = picture
            .painted
            .take()
            .filter(|before| listed && before.palette == *palette && before.tables_version == self.tables_version);
        let whole = before.is_none();
        let mut work = visits.saturating_mul(DRAWING_VISIT_COST);
        if !listed {
            // A scene of more steps than a picture keeps is painted whole
            // each time, straight from the drawings, in the same order.
            picture.background = None;
            picture.begin(base, Some(Bounds::page()), &[], true, origins);
            for (_, _, number) in &top {
                let mut painter =
                    |step: Step| paint(&mut self.player, picture.screen_mut(), &step, Bounds::page(), true, &mut work);
                let mut sink = Sink { step: &mut painter, origins: &mut BTreeMap::new(), visits: &mut 0 };
                walk.visit(&mut sink, *number, Place::screen(), false, MAX_DRAWING_DEPTH);
            }
            picture.end(self.commits, work);
            return;
        }
        let (behind, over) = steps.split_at_checked(background).unwrap_or((&steps, &[]));
        // Where the base differs from the one the picture was painted over;
        // the whole page when there was none.
        let mut base_changed = Some(Bounds::page());
        let (mut behind_damage, mut damage) = match &before {
            Some(before) => {
                let (old_behind, old_over) =
                    before.steps.split_at_checked(before.background).unwrap_or((&before.steps, &[]));
                let mut behind_damage = changed(old_behind, behind);
                base_changed = difference(&before.base, base.pixels());
                behind_damage.add(base_changed);
                behind_damage.once_per_step(behind);
                let mut damage = changed(old_over, over);
                damage.join(&behind_damage);
                damage.once_per_step(over);
                (behind_damage, damage)
            }
            None => (Damage::page(), Damage::page()),
        };
        let mut raster = picture.background.take();
        if behind.is_empty() {
            // Nothing is in layer 0: the base is the background.
            raster = None;
        } else {
            let fresh = raster.is_none();
            if fresh {
                (behind_damage, damage) = (Damage::page(), Damage::page());
            }
            let raster = raster.get_or_insert_with(Screen::single);
            repaint_background(&mut self.player, raster, base, behind, &behind_damage, whole || fresh, &mut work);
        }
        let beneath = raster.as_ref().map_or(base, Screen::displayed);
        let bounds = damage.bounds();
        picture.begin(beneath, bounds, damage.rectangles(), whole, origins);
        for &area in damage.rectangles() {
            for step in over {
                paint(&mut self.player, picture.screen_mut(), step, area, whole, &mut work);
            }
            if whole {
                break;
            }
        }
        picture.end(self.commits, work);
        picture.background = raster;
        let mut kept = before.map(|before| before.base).unwrap_or_default();
        // A base that did not change is kept as it is, not copied again.
        if base_changed.is_some() {
            kept.clear();
            kept.extend_from_slice(base.pixels());
        }
        picture.painted = Some(Painted {
            steps,
            background,
            base: kept,
            palette: palette.clone(),
            tables_version: self.tables_version,
        });
    }
}

/// Repaints layer 0 over the base into the background raster: whole, or
/// each rectangle of `damage` put back from the base and repainted on its
/// own (animation.md ANIM-12).
fn repaint_background(
    player: &mut Interpreter,
    raster: &mut Screen,
    base: &crate::surface::Canvas,
    behind: &[Step],
    damage: &Damage,
    whole: bool,
    work: &mut u64,
) {
    if whole {
        if let Some(canvas) = raster.page_mut(0) {
            canvas.set_window(Window::page());
            canvas.copy_from(base);
        }
        for step in behind {
            paint(player, raster, step, Bounds::page(), true, work);
        }
    } else {
        for &area in damage.rectangles() {
            if let Some(canvas) = raster.page_mut(0) {
                canvas.set_window(Window::page());
                canvas.restore(base, area);
            }
            for step in behind {
                paint(player, raster, step, area, false, work);
            }
        }
    }
    if let Some(canvas) = raster.page_mut(0) {
        canvas.set_window(Window::page());
        canvas.tidy_pick();
    }
}

/// Replays one step inside the rectangle being repainted. When the whole
/// picture is being painted every step is replayed, whatever it was
/// measured to reach, so that a whole repaint does not rest on the
/// measurement a partial one is checked against.
fn paint(player: &mut Interpreter, screen: &mut Screen, step: &Step, damage: Bounds, whole: bool, work: &mut u64) {
    // The commit checked the work; this stops a fault there from
    // becoming unbounded painting.
    if *work > MAX_REBUILD_WORK {
        return;
    }
    let clip =
        Bounds { left: step.window.left, top: step.window.top, right: step.window.right, bottom: step.window.bottom };
    let window = if whole {
        step.window
    } else {
        let Some(cut) = step.area.and_then(|area| area.within(damage)).and_then(|area| area.within(clip)) else {
            return;
        };
        // Only the part repainted is paid for: the commit has already
        // checked what the whole costs, wherever it lies. A step painted
        // in part is metered whole, so that the part is the same part.
        let whole_cost = step.part.is_some();
        Window { left: cut.left, top: cut.top, right: cut.right, bottom: cut.bottom, whole_cost, ..step.window }
    };
    let allowed = MAX_REBUILD_WORK.saturating_sub(*work);
    let spent = match step.part {
        Some(part) => play_part(player, screen, &step.contents, window, part.min(allowed)),
        None => play(player, screen, &step.contents, window, allowed).unwrap_or(allowed),
    };
    *work = work.saturating_add(spent);
}

/// What is being painted: the drawings, what the viewer's pointer is
/// doing to them, and what the receiver's animation state makes of them.
#[derive(Clone, Copy)]
pub(super) struct Walk<'a> {
    pub(super) stage: &'a Stage,
    pub(super) viewer: ViewerState,
    pub(super) animation: &'a BTreeMap<u16, AnimationState>,
    /// The ticks each drawing being painted gradually has been painting;
    /// `None` to take every drawing as finished.
    pub(super) painting: Option<&'a BTreeMap<u16, u64>>,
}

impl Walk<'_> {
    fn state(self, number: u16) -> AnimationState {
        self.animation.get(&number).copied().unwrap_or_default()
    }

    /// Whether a drawing is shown: by its animation state, else by its
    /// attribute `V` (animation.md, ANIM-9).
    pub(super) fn visible(self, number: u16, drawing: &Drawing) -> bool {
        self.state(number).shown.unwrap_or(drawing.visible)
    }

    /// A drawing's children with what orders them, in drawing order: by
    /// layer among the top-level drawings (ANIM-12), then by depth, then
    /// by number. The animation state's layer and order stand in for the
    /// attributes `G` and `L`.
    fn ordered(self, parent: u16) -> Vec<(u8, i32, u16)> {
        let key = |child: u16| {
            let state = self.state(child);
            let (layer, depth) = self.stage.get(child).map_or((0, 0), |drawing| (drawing.layer, drawing.depth));
            let layer = if parent == 0 { state.layer.unwrap_or(layer) } else { 0 };
            (layer, state.order.map_or(depth, i32::from), child)
        };
        let mut children: Vec<(u8, i32, u16)> = self.stage.child_numbers(parent).map(key).collect();
        children.sort_unstable();
        children
    }

    /// The drawing painted in place of a drawing's contents, 0 for its
    /// own. In order: a forced look; its pressed look while it is pressed;
    /// its hover look while it is hovered; the entry of its frame list
    /// the frame in force names; its source (`docs/regis-v2.md`, "Viewer
    /// state"; animation.md, ANIM-10). A look that does not exist leaves
    /// the next in that order. A drawing painted as another's source or
    /// look is not itself the drawing under the pointer, so its own looks
    /// are not used; its frame list is, being part of how it looks.
    fn shown(self, number: u16, drawing: &Drawing, as_source: bool) -> u16 {
        let state = self.state(number);
        let existing = |look: u16| (!as_source && self.stage.get(look).is_some()).then_some(look);
        let forced = match state.look {
            ForcedLook::None => 0,
            ForcedLook::Hover => drawing.hover,
            ForcedLook::Pressed => drawing.pressed,
            ForcedLook::Alternative => drawing.alternative,
        };
        let pointed = |named: u16, look: u16| if named == number { existing(look) } else { None };
        existing(forced)
            .or_else(|| pointed(self.viewer.pressed, drawing.pressed))
            .or_else(|| pointed(self.viewer.hovered, drawing.hover))
            .or_else(|| drawing.frames.entry(state.frame))
            .unwrap_or(drawing.source)
    }

    /// Lists the steps of one top-level drawing and what nests in it.
    /// False, with the list cut short, once it is longer than a picture
    /// keeps.
    fn lay_out(self, layout: &mut Layout, number: u16, depth: usize) -> bool {
        let Layout { steps, origins, visits } = layout;
        let mut room = true;
        let mut list = |step: Step| {
            if steps.len() < MAX_DAMAGE_STEPS {
                steps.push(step);
            } else {
                room = false;
            }
        };
        self.visit(&mut Sink { step: &mut list, origins, visits }, number, Place::screen(), false, depth);
        room
    }

    /// Where a drawing is painted, given where its parent is: moved by
    /// its position, cut by its clip rectangle, picked as itself if it is
    /// detectable. `None` when it is not shown. `as_source` when it is
    /// painted in place of another's contents: its own position,
    /// visibility and detectable flag are then not used.
    fn enter(
        self,
        origins: &mut BTreeMap<u16, (i32, i32)>,
        number: u16,
        drawing: &Drawing,
        mut place: Place,
        as_source: bool,
    ) -> Option<Place> {
        if !as_source {
            if !self.visible(number, drawing) || number == self.viewer.hidden {
                return None;
            }
            let (x, y) =
                self.state(number).position.map_or((drawing.x, drawing.y), |(x, y)| (i32::from(x), i32::from(y)));
            place.x = place.x.saturating_add(x);
            place.y = place.y.saturating_add(y);
            if drawing.detectable && !place.reused {
                place.pick = number;
                origins.insert(number, (place.x, place.y));
            }
        }
        if let Some(clip) = drawing.clip {
            let left = place.x.saturating_add(clip.x);
            let top = place.y.saturating_add(clip.y);
            let cut = Bounds {
                left,
                top,
                right: left.saturating_add(clip.width).saturating_sub(1),
                bottom: top.saturating_add(clip.height).saturating_sub(1),
            };
            place.clip = place.clip.and_then(|clip| clip.within(cut));
        }
        Some(place)
    }

    /// Walks one drawing and what nests in it in painting order, handing
    /// over each step.
    fn visit(self, sink: &mut Sink<'_>, number: u16, place: Place, as_source: bool, depth: usize) {
        let Some(drawing) = self.stage.get(number) else { return };
        // The commit checked depth and work; these stop a fault there
        // from becoming an unbounded walk.
        let Some(depth) = depth.checked_sub(1) else { return };
        if sink.visits.saturating_mul(DRAWING_VISIT_COST) > MAX_REBUILD_WORK {
            return;
        }
        let Some(place) = self.enter(sink.origins, number, drawing, place, as_source) else { return };
        *sink.visits = sink.visits.saturating_add(1);
        // A drawing painted as part of a source is paid for by the
        // drawing that placed it: its own paint rate is not used.
        let painting = if place.reused { None } else { self.painting.and_then(|painting| painting.get(&number)) };
        if let Some(ticks) = painting.filter(|_| drawing.rate != 0) {
            // What is painted in the drawing's place appears in order as
            // its work is earned, and its children wait for it to finish
            // (DEC-6382 items 1 and 3).
            let mut left = painted_after(*ticks, drawing.rate);
            let mut stopped = false;
            let step = &mut *sink.step;
            let mut gradual = |step_whole: Step| {
                if stopped {
                    return;
                }
                if step_whole.cost <= left {
                    left = left.saturating_sub(step_whole.cost);
                    step(step_whole);
                } else {
                    stopped = true;
                    if left != 0 {
                        step(Step { part: Some(left), ..step_whole });
                    }
                }
            };
            let mut inner = Sink { step: &mut gradual, origins: &mut *sink.origins, visits: &mut *sink.visits };
            self.own(&mut inner, number, drawing, place, as_source, depth);
            return;
        }
        self.own(sink, number, drawing, place, as_source, depth);
        for (_, _, child) in self.ordered(number) {
            self.visit(sink, child, place, false, depth);
        }
    }

    /// What is painted for a drawing itself, its children apart: its own
    /// contents, or the drawing painted in their place with that
    /// drawing's children.
    fn own(self, sink: &mut Sink<'_>, number: u16, drawing: &Drawing, place: Place, as_source: bool, depth: usize) {
        let shown = self.shown(number, drawing, as_source);
        if shown != 0 {
            self.visit(sink, shown, Place { reused: true, ..place }, true, depth);
            return;
        }
        // An empty clip is a window that holds no pixel.
        let clip = place.clip.unwrap_or(Bounds { left: 0, top: 0, right: -1, bottom: -1 });
        let window = Window {
            dx: place.x,
            dy: place.y,
            left: clip.left,
            top: clip.top,
            right: clip.right,
            bottom: clip.bottom,
            pick: place.pick,
            whole_cost: true,
        };
        // Contents the commit did not measure may reach anywhere.
        let reach = drawing
            .measured
            .map_or(Some(Bounds::page()), |measured| measured.reach.map(|reach| reach.moved(place.x, place.y)));
        let area = place.clip.and_then(|clip| reach.and_then(|reach| reach.within(clip)));
        let bytes = u64::try_from(drawing.contents.len()).unwrap_or(u64::MAX).saturating_mul(DRAWING_BYTE_COST);
        let cost = drawing.measured.map_or(0, |measured| measured.work.saturating_sub(bytes));
        (sink.step)(Step { contents: drawing.contents.clone(), window, area, cost, part: None });
    }

    /// The work units a paint rate pays for a drawing: everything painted
    /// in its place, its children apart, under the state in force.
    pub(super) fn own_cost(self, number: u16, drawing: &Drawing) -> u64 {
        let mut cost = 0_u64;
        let mut add = |step: Step| cost = cost.saturating_add(step.cost);
        let mut sink = Sink { step: &mut add, origins: &mut BTreeMap::new(), visits: &mut 0 };
        self.own(&mut sink, number, drawing, Place { reused: true, ..Place::screen() }, false, MAX_DRAWING_DEPTH);
        cost
    }

    /// The rectangle of the screen a drawing can write in as its state
    /// stands, with its shown children and the look or frame in force
    /// (animation.md, ANIM-13). `None` while it does not exist, is not
    /// shown, or reaches no pixel of the screen.
    pub(super) fn bounds(self, number: u16) -> Option<Bounds> {
        let mut chain = Vec::new();
        let mut at = number;
        loop {
            let drawing = self.stage.get(at)?;
            if drawing.parent == 0 {
                break;
            }
            // The commit bounded the chain; a longer one is a fault there.
            if chain.len() >= MAX_DRAWING_DEPTH {
                return None;
            }
            chain.push(drawing.parent);
            at = drawing.parent;
        }
        let mut place = Place::screen();
        let mut origins = BTreeMap::new();
        for ancestor in chain.iter().rev() {
            place = self.enter(&mut origins, *ancestor, self.stage.get(*ancestor)?, place, false)?;
        }
        let mut bounds = None;
        let mut join = |step: Step| bounds = Bounds::join(bounds, step.area);
        let whole = Walk { painting: None, ..self };
        whole.visit(
            &mut Sink { step: &mut join, origins: &mut origins, visits: &mut 0 },
            number,
            place,
            false,
            MAX_DRAWING_DEPTH,
        );
        bounds
    }
}
