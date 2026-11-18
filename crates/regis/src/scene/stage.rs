//! One state of the scene: the numbered drawings and their attributes.

use alloc::collections::btree_map::Entry;
use alloc::collections::{BTreeMap, BTreeSet};
// Contents are shared between the committed and the staged scene. Where
// the target has atomics the sharing is thread-safe, so that a terminal
// or a validator can be handed to another thread, as a server does; a
// target without them has no threads to hand it to.
#[cfg(not(target_has_atomic = "ptr"))]
use alloc::rc::Rc as Shared;
#[cfg(target_has_atomic = "ptr")]
use alloc::sync::Arc as Shared;
use alloc::vec::Vec;

use super::Scene;
use crate::limits::DEFAULT_LAYER;
use crate::machine::{Attribute, Clip, Frames};
use crate::surface::Bounds;

/// A drawing's contents, shared between every state that holds them.
pub(super) type Contents = Shared<[u8]>;

/// What painting a drawing's contents was measured to take, under the
/// committed tables.
#[derive(Clone, Copy, Debug)]
pub(super) struct Measured {
    /// The work it costs, wherever the drawing is placed.
    pub(super) work: u64,
    /// The pixels it can write, relative to the drawing's origin and
    /// before any clipping; `None` if it writes none.
    pub(super) reach: Option<Bounds>,
}

/// A numbered drawing (`docs/regis-v2.md`, "Attributes").
#[derive(Clone, Debug)]
pub(super) struct Drawing {
    /// The ReGIS commands it holds, as received.
    pub(super) contents: Contents,
    /// What its contents take to paint, once measured.
    pub(super) measured: Option<Measured>,
    pub(super) x: i32,
    pub(super) y: i32,
    pub(super) depth: i32,
    pub(super) visible: bool,
    /// The drawing it belongs to, 0 for none.
    pub(super) parent: u16,
    /// The drawing painted in place of its contents, 0 for none.
    pub(super) source: u16,
    /// The drawing painted in its place while the viewer's pointer is
    /// over it, 0 for none.
    pub(super) hover: u16,
    /// The drawing painted in its place while it is pressed, 0 for none.
    pub(super) pressed: u16,
    pub(super) detectable: bool,
    pub(super) clip: Option<Clip>,
    /// The layer it is painted in when it is a top-level drawing, 0 to 3
    /// (animation.md, ANIM-12).
    pub(super) layer: u8,
    /// The drawing painted in its place while the alternative look is
    /// forced, 0 for none (ANIM-10).
    pub(super) alternative: u16,
    /// The drawings of which the one the frame in force names is painted
    /// in its place (ANIM-10).
    pub(super) frames: Frames,
    /// The paint rate, 0 for at once (DEC-6382; ANIM-15).
    pub(super) rate: u16,
}

impl Default for Drawing {
    fn default() -> Self {
        Self {
            contents: Shared::from([]),
            measured: None,
            x: 0,
            y: 0,
            depth: 0,
            visible: true,
            parent: 0,
            source: 0,
            hover: 0,
            pressed: 0,
            detectable: false,
            clip: None,
            layer: DEFAULT_LAYER,
            alternative: 0,
            frames: Frames::default(),
            rate: 0,
        }
    }
}

/// The numbered drawings at one moment. Copying it copies no contents.
#[derive(Clone, Debug, Default)]
pub(super) struct Stage {
    drawings: BTreeMap<u16, Drawing>,
    /// Every (parent, child) pair, top-level drawings under parent 0, so a
    /// drawing's children are found without looking at every drawing.
    children: BTreeSet<(u16, u16)>,
    /// Bytes of contents held.
    bytes: usize,
}

impl Stage {
    pub(super) fn is_empty(&self) -> bool {
        self.drawings.is_empty()
    }

    pub(super) fn get(&self, drawing: u16) -> Option<&Drawing> {
        self.drawings.get(&drawing)
    }

    pub(super) fn get_mut(&mut self, drawing: u16) -> Option<&mut Drawing> {
        self.drawings.get_mut(&drawing)
    }

    pub(super) fn numbers(&self) -> impl Iterator<Item = u16> + '_ {
        self.drawings.keys().copied()
    }

    pub(super) fn drawings(&self) -> impl Iterator<Item = (u16, &Drawing)> + '_ {
        self.drawings.iter().map(|(number, drawing)| (*number, drawing))
    }

    /// Forgets every measurement: the shared tables or the font changed.
    pub(super) fn forget_measurements(&mut self) {
        for drawing in self.drawings.values_mut() {
            drawing.measured = None;
        }
    }

    /// A drawing's children (all of them, shown or not), by number; the
    /// top-level drawings are the children of 0.
    pub(super) fn child_numbers(&self, parent: u16) -> impl Iterator<Item = u16> + '_ {
        self.children.range((parent, u16::MIN)..=(parent, u16::MAX)).map(|(_, child)| *child)
    }

    /// A drawing's children (all of them, shown or not), in drawing order:
    /// by depth, and by number among equal depths.
    pub(super) fn children(&self, parent: u16) -> Vec<u16> {
        let mut children: Vec<(i32, u16)> = self
            .children
            .range((parent, u16::MIN)..=(parent, u16::MAX))
            .map(|(_, child)| (self.get(*child).map_or(0, |drawing| drawing.depth), *child))
            .collect();
        children.sort_unstable();
        children.into_iter().map(|(_, child)| child).collect()
    }

    /// `B` or `S` on an unused number makes the drawing, with default
    /// attributes.
    pub(super) fn create(&mut self, number: u16) {
        if let Entry::Vacant(vacant) = self.drawings.entry(number) {
            vacant.insert(Drawing::default());
            self.children.insert((0, number));
        }
    }

    pub(super) fn set(&mut self, number: u16, attribute: Attribute) {
        self.create(number);
        let Some(drawing) = self.drawings.get_mut(&number) else { return };
        match attribute {
            Attribute::X(x) => drawing.x = x,
            Attribute::Y(y) => drawing.y = y,
            Attribute::Depth(depth) => drawing.depth = depth,
            Attribute::Visible(visible) => drawing.visible = visible,
            Attribute::Source(source) => drawing.source = source,
            Attribute::Hover(hover) => drawing.hover = hover,
            Attribute::Pressed(pressed) => drawing.pressed = pressed,
            Attribute::Detectable(detectable) => drawing.detectable = detectable,
            Attribute::Clip(clip) => drawing.clip = clip,
            Attribute::Layer(layer) => drawing.layer = layer,
            Attribute::Alternative(alternative) => drawing.alternative = alternative,
            Attribute::Frames(frames) => drawing.frames = frames,
            Attribute::Rate(rate) => drawing.rate = rate,
            Attribute::Parent(parent) => {
                let old = core::mem::replace(&mut drawing.parent, parent);
                self.children.remove(&(old, number));
                self.children.insert((parent, number));
            }
        }
    }

    /// Replaces a drawing's contents, keeping its attributes and children.
    /// `None`, and nothing stored, if the contents of all drawings would
    /// then pass the cap.
    pub(super) fn define(&mut self, number: u16, contents: Vec<u8>) -> Option<()> {
        self.create(number);
        let drawing = self.drawings.get_mut(&number)?;
        let bytes = self.bytes.saturating_sub(drawing.contents.len()).saturating_add(contents.len());
        if bytes > Scene::contents_cap() {
            return None;
        }
        self.bytes = bytes;
        drawing.contents = Shared::from(contents);
        drawing.measured = None;
        Some(())
    }

    /// Removes a drawing and its children, and theirs. A number that does
    /// not exist does nothing.
    pub(super) fn delete(&mut self, number: u16) {
        let mut doomed = Vec::from([number]);
        while let Some(next) = doomed.pop() {
            // A drawing already gone is not followed again, so parents
            // that name each other in a ring end the walk.
            let Some(drawing) = self.drawings.remove(&next) else { continue };
            self.bytes = self.bytes.saturating_sub(drawing.contents.len());
            self.children.remove(&(drawing.parent, next));
            doomed.extend(self.children.range((next, u16::MIN)..=(next, u16::MAX)).map(|(_, child)| *child));
        }
    }
}
