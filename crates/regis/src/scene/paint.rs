//! Painting the committed scene over the base picture
//! (`docs/regis-v2.md`, "Attributes", "Rules for contents" and "Picking").

use super::picture::Picture;
use super::stage::Stage;
use super::{Scene, ViewerState};
use crate::limits::{DRAWING_BYTE_COST, DRAWING_VISIT_COST, MAX_DRAWING_DEPTH, MAX_REBUILD_WORK};
use crate::palette::Palette;
use crate::surface::{Canvas, Screen, Window};
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
    /// up the chain; left, top, right, bottom, both ends included.
    clip: (i32, i32, i32, i32),
    /// The drawing picking reports for the pixels it writes, 0 for none.
    pick: u16,
    /// It is being painted as part of a source: reuse gives appearance,
    /// not identity, so no drawing below is itself picked.
    reused: bool,
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

impl Scene {
    /// The work a drawing's contents cost to paint, wherever it is placed:
    /// they are replayed with nowhere to draw. `None` if that is more than
    /// a whole repaint may cost.
    pub(super) fn measure(&mut self, contents: &[u8]) -> Option<u64> {
        let mut nowhere = Screen::absent();
        let window = Window { left: 0, right: -1, whole_cost: true, ..Window::page() };
        play(&mut self.player, &mut nowhere, contents, window, MAX_REBUILD_WORK)
    }

    /// Rebuilds the picture: the base, then every visible drawing in
    /// drawing order.
    pub(crate) fn compose(&mut self, base: &Screen, palette: &Palette, picture: &mut Picture) {
        picture.begin(base.displayed());
        self.player.machine.palette = palette.clone();
        let screen = (0, 0, Canvas::width_i32().saturating_sub(1), Canvas::height_i32().saturating_sub(1));
        let place = Place { x: 0, y: 0, clip: screen, pick: 0, reused: false };
        let mut work = 0_u64;
        for number in self.committed.children(0) {
            let walk = Walk { stage: &self.committed, viewer: self.viewer };
            paint(walk, &mut self.player, picture, number, place, false, MAX_DRAWING_DEPTH, &mut work);
        }
        picture.end(self.commits, work);
    }
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
    fn shown(self, number: u16, drawing: &super::stage::Drawing, as_source: bool) -> u16 {
        let look =
            |named: u16, look: u16| (!as_source && named == number && self.stage.get(look).is_some()).then_some(look);
        look(self.viewer.pressed, drawing.pressed)
            .or_else(|| look(self.viewer.hovered, drawing.hover))
            .unwrap_or(drawing.source)
    }
}

/// Paints one drawing and what nests in it. `as_source` when it is painted
/// in place of another's contents: its own position, visibility and
/// detectable flag are then not used.
#[allow(clippy::too_many_arguments, reason = "the walk's whole state; a struct would only rename the arguments")]
fn paint(
    walk: Walk<'_>,
    player: &mut Interpreter,
    picture: &mut Picture,
    number: u16,
    mut place: Place,
    as_source: bool,
    depth: usize,
    work: &mut u64,
) {
    let stage = walk.stage;
    let Some(drawing) = stage.get(number) else { return };
    // The commit checked depth and work; these stop a fault there from
    // becoming unbounded painting.
    let Some(depth) = depth.checked_sub(1) else { return };
    if *work > MAX_REBUILD_WORK {
        return;
    }
    if !as_source {
        if !drawing.visible || number == walk.viewer.hidden {
            return;
        }
        place.x = place.x.saturating_add(drawing.x);
        place.y = place.y.saturating_add(drawing.y);
        if drawing.detectable && !place.reused {
            place.pick = number;
            picture.note_origin(number, place.x, place.y);
        }
    }
    if let Some(clip) = drawing.clip {
        let left = place.x.saturating_add(clip.x);
        let top = place.y.saturating_add(clip.y);
        place.clip = (
            place.clip.0.max(left),
            place.clip.1.max(top),
            place.clip.2.min(left.saturating_add(clip.width).saturating_sub(1)),
            place.clip.3.min(top.saturating_add(clip.height).saturating_sub(1)),
        );
    }
    *work = work.saturating_add(DRAWING_VISIT_COST);
    let shown = walk.shown(number, drawing, as_source);
    if shown == 0 {
        let window = Window {
            dx: place.x,
            dy: place.y,
            left: place.clip.0,
            top: place.clip.1,
            right: place.clip.2,
            bottom: place.clip.3,
            pick: place.pick,
            whole_cost: true,
        };
        let allowed = MAX_REBUILD_WORK.saturating_sub(*work);
        let spent = play(player, picture.screen_mut(), &drawing.contents, window, allowed).unwrap_or(allowed);
        *work = work.saturating_add(spent);
    } else {
        let reused = Place { reused: true, ..place };
        paint(walk, player, picture, shown, reused, true, depth, work);
    }
    for child in stage.children(number) {
        paint(walk, player, picture, child, place, false, depth, work);
    }
}
