//! Painting the committed scene over the base picture
//! (`docs/regis-v2.md`, "Attributes", "Rules for contents", "Commit and
//! repaint" and "Picking").
//!
//! The scene is first laid out as a list of steps, each one drawing's
//! contents replayed at one place. A picture painted before keeps its
//! list, so the next one finds what changed and repaints only the
//! rectangle that holds it: the same steps, cut to that rectangle.

use alloc::collections::BTreeMap;
use alloc::vec::Vec;

use super::picture::{Painted, Picture};
use super::stage::{Contents, Drawing, Stage};
use super::{Scene, ViewerState};
use crate::limits::{DRAWING_BYTE_COST, DRAWING_VISIT_COST, MAX_DAMAGE_STEPS, MAX_DRAWING_DEPTH, MAX_REBUILD_WORK};
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

/// One drawing's contents replayed at one place.
#[derive(Clone, Debug)]
pub(super) struct Step {
    contents: Contents,
    /// Where it lands and what it may change.
    window: Window,
    /// The pixels it can change: where its contents reach, moved to its
    /// place and cut by its clip rectangles. `None` if there are none.
    area: Option<Bounds>,
}

impl Step {
    /// Whether two steps paint the same pixels the same way: the same
    /// stored contents (not merely equal bytes), at the same place, under
    /// the same clip, for the same pick target.
    fn same(&self, other: &Self) -> bool {
        Contents::ptr_eq(&self.contents, &other.contents) && self.window == other.window
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

/// The rectangle that holds everything two lists of steps paint
/// differently. Steps the lists share at their start and at their end are
/// painted the same and in the same order relative to everything else, so
/// a pixel none of the others can change comes out as it did before.
fn changed(old: &[Step], new: &[Step]) -> Option<Bounds> {
    let same = |(old, new): (&Step, &Step)| old.same(new);
    let start = old.iter().zip(new).take_while(|pair| same(*pair)).count();
    let (old, new) = (old.get(start..).unwrap_or(&[]), new.get(start..).unwrap_or(&[]));
    let end = old.iter().rev().zip(new.iter().rev()).take_while(|pair| same(*pair)).count();
    let middle = |steps: &[Step]| {
        let kept = steps.len().saturating_sub(end);
        steps.get(..kept).unwrap_or(&[]).iter().fold(None, |bounds, step| Bounds::join(bounds, step.area))
    };
    Bounds::join(middle(old), middle(new))
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
    /// order. A picture this scene painted before is repainted only where
    /// the base, the drawings or the viewer state changed it; the result
    /// is the same, pixel for pixel and in the pick record, as painting it
    /// whole (`docs/regis-v2.md`, "Commit and repaint").
    pub(crate) fn compose(&mut self, base: &Screen, palette: &Palette, picture: &mut Picture) {
        self.player.machine.palette = palette.clone();
        let walk = Walk { stage: &self.committed, viewer: self.viewer };
        let screen = Place { x: 0, y: 0, clip: Some(Bounds::page()), pick: 0, reused: false };
        let mut layout = Layout::default();
        let mut listed = true;
        for number in self.committed.children(0) {
            listed &= walk.lay_out(&mut layout, number, screen, false, MAX_DRAWING_DEPTH);
        }
        let Layout { steps, origins, visits } = layout;
        let base = base.displayed();
        let before = picture
            .painted
            .take()
            .filter(|before| listed && before.palette == *palette && before.tables_version == self.tables_version);
        // A scene of more steps than a picture keeps is painted whole
        // each time, straight from the drawings.
        let damage = match &before {
            Some(before) => Bounds::join(difference(&before.base, base.pixels()), changed(&before.steps, &steps)),
            None => Some(Bounds::page()),
        };
        let whole = before.is_none();
        picture.begin(base, damage, whole, origins);
        let mut work = visits.saturating_mul(DRAWING_VISIT_COST);
        if !listed {
            for number in self.committed.children(0) {
                let mut painter = |step: Step| paint(&mut self.player, picture, &step, Bounds::page(), true, &mut work);
                walk.visit(&mut painter, &mut BTreeMap::new(), &mut 0, number, screen, false, MAX_DRAWING_DEPTH);
            }
        } else if let Some(damage) = damage {
            for step in &steps {
                paint(&mut self.player, picture, step, damage, whole, &mut work);
            }
        }
        picture.end(self.commits, work, damage);
        if listed {
            let mut kept = before.map(|before| before.base).unwrap_or_default();
            kept.clear();
            kept.extend_from_slice(base.pixels());
            picture.painted =
                Some(Painted { steps, base: kept, palette: palette.clone(), tables_version: self.tables_version });
        }
    }
}

/// Replays one step inside the rectangle being repainted. When the whole
/// picture is being painted every step is replayed, whatever it was
/// measured to reach, so that a whole repaint does not rest on the
/// measurement a partial one is checked against.
fn paint(player: &mut Interpreter, picture: &mut Picture, step: &Step, damage: Bounds, whole: bool, work: &mut u64) {
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
        // checked what the whole costs, wherever it lies.
        Window { left: cut.left, top: cut.top, right: cut.right, bottom: cut.bottom, whole_cost: false, ..step.window }
    };
    let allowed = MAX_REBUILD_WORK.saturating_sub(*work);
    let spent = play(player, picture.screen_mut(), &step.contents, window, allowed).unwrap_or(allowed);
    *work = work.saturating_add(spent);
}

/// What is being painted: the drawings, and what the viewer's pointer is
/// doing to them.
#[derive(Clone, Copy)]
struct Walk<'a> {
    stage: &'a Stage,
    viewer: ViewerState,
}

impl Walk<'_> {
    /// The drawing painted in place of a drawing's contents, 0 for its
    /// own: its pressed look while it is pressed, its hover look while it
    /// is hovered, and its source otherwise (`docs/regis-v2.md`, "Viewer
    /// state"). A look that does not exist leaves the ordinary look. A
    /// drawing painted as another's source or look is not itself the
    /// drawing under the pointer, so its own looks are not used.
    fn shown(self, number: u16, drawing: &Drawing, as_source: bool) -> u16 {
        let look =
            |named: u16, look: u16| (!as_source && named == number && self.stage.get(look).is_some()).then_some(look);
        look(self.viewer.pressed, drawing.pressed)
            .or_else(|| look(self.viewer.hovered, drawing.hover))
            .unwrap_or(drawing.source)
    }

    /// Lists the steps of one drawing and what nests in it. False, with
    /// the list cut short, once it is longer than a picture keeps.
    fn lay_out(self, layout: &mut Layout, number: u16, place: Place, as_source: bool, depth: usize) -> bool {
        let Layout { steps, origins, visits } = layout;
        let mut room = true;
        let mut list = |step: Step| {
            if steps.len() < MAX_DAMAGE_STEPS {
                steps.push(step);
            } else {
                room = false;
            }
        };
        self.visit(&mut list, origins, visits, number, place, as_source, depth);
        room
    }

    /// Walks one drawing and what nests in it in painting order, handing
    /// over each step. `as_source` when it is painted in place of
    /// another's contents: its own position, visibility and detectable
    /// flag are then not used.
    #[allow(clippy::too_many_arguments, reason = "the walk's whole state; a struct would only rename the arguments")]
    fn visit(
        self,
        step: &mut dyn FnMut(Step),
        origins: &mut BTreeMap<u16, (i32, i32)>,
        visits: &mut u64,
        number: u16,
        mut place: Place,
        as_source: bool,
        depth: usize,
    ) {
        let Some(drawing) = self.stage.get(number) else { return };
        // The commit checked depth and work; these stop a fault there
        // from becoming an unbounded walk.
        let Some(depth) = depth.checked_sub(1) else { return };
        if visits.saturating_mul(DRAWING_VISIT_COST) > MAX_REBUILD_WORK {
            return;
        }
        if !as_source {
            if !drawing.visible || number == self.viewer.hidden {
                return;
            }
            place.x = place.x.saturating_add(drawing.x);
            place.y = place.y.saturating_add(drawing.y);
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
        *visits = visits.saturating_add(1);
        let shown = self.shown(number, drawing, as_source);
        if shown == 0 {
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
            step(Step { contents: drawing.contents.clone(), window, area });
        } else {
            let reused = Place { reused: true, ..place };
            self.visit(step, origins, visits, shown, reused, true, depth);
        }
        for child in self.stage.children(number) {
            self.visit(step, origins, visits, child, place, false, depth);
        }
    }
}
