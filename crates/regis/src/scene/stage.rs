//! One state of the scene: the numbered drawings and their attributes.

use alloc::boxed::Box;
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
use crate::event::CommitError;
use crate::limits::{DEFAULT_LAYER, DRAWING_EXTRAS_BYTES, DRAWING_RECORD_BYTES, MAX_DRAWING_RECORD_BYTES};
use crate::machine::{Attribute, Clip, Frames, Line};
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
pub(crate) struct Drawing {
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
    /// The paint rate `T` the stream gave it, 0 for at once (DEC-6382;
    /// ANIM-15); `None` when it gave none, and the drawing then takes the
    /// receiver's default (DEC-7031 item 2, REQ-7025). A drawing deleted
    /// and made again has none.
    pub(super) rate: Option<u16>,
    /// Its frame list and label, kept apart and only once one is given:
    /// together they are larger than the rest of the record, and most
    /// drawings have neither (SOLVEDSECURITYISSUES 015).
    extras: Option<Box<Extras>>,
}

/// The attributes of a drawing too large to keep in every record.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct Extras {
    /// The drawings of which the one the frame in force names is painted
    /// in its place (ANIM-10).
    frames: Frames,
    /// The label a screen reader speaks for it, `U`; never drawn
    /// (DEC-6638 item 6).
    label: Line,
}

impl Drawing {
    /// Its paint rate in force: its own `T`, else the receiver's default
    /// (DEC-7031 item 2). 0 paints it at once.
    pub(super) fn rate_in_force(&self, default_rate: u16) -> u16 {
        self.rate.unwrap_or(default_rate)
    }

    /// Its frame list; empty when it has none.
    pub(super) fn frames(&self) -> Frames {
        self.extras.as_ref().map(|extras| extras.frames).unwrap_or_default()
    }

    /// Its label; `None` when it has none.
    pub(super) fn label(&self) -> Option<&Line> {
        self.extras.as_ref().map(|extras| &extras.label)
    }
}

impl Drawing {
    /// A drawing with default attributes and the contents given, which are
    /// a stage's one empty contents: an empty drawing allocates nothing
    /// for them.
    fn new(contents: Contents) -> Self {
        Self {
            contents,
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
            rate: None,
            extras: None,
        }
    }
}

/// The numbered drawings at one moment. Copying it copies no contents.
#[derive(Clone, Debug)]
pub(super) struct Stage {
    /// Each record is boxed, so that a node of the map holds numbers and
    /// pointers and not eleven whole records, most of a node empty.
    drawings: BTreeMap<u16, Box<Drawing>>,
    /// Every (parent, child) pair, top-level drawings under parent 0, so a
    /// drawing's children are found without looking at every drawing.
    children: BTreeSet<(u16, u16)>,
    /// Bytes of contents held.
    bytes: usize,
    /// The memory the records hold, as [`DRAWING_RECORD_BYTES`] and
    /// [`DRAWING_EXTRAS_BYTES`] charge it; held to
    /// [`MAX_DRAWING_RECORD_BYTES`] (SOLVEDSECURITYISSUES 015).
    records: usize,
    /// The contents of every drawing that has none: one allocation shared.
    empty: Contents,
    /// The picture's title, `D(T'...')`; never drawn (DEC-6638 item 3).
    pub(super) title: Line,
}

impl Default for Stage {
    fn default() -> Self {
        Self {
            drawings: BTreeMap::new(),
            children: BTreeSet::new(),
            bytes: 0,
            records: 0,
            empty: Shared::from([]),
            title: Line::default(),
        }
    }
}

impl Stage {
    pub(super) fn is_empty(&self) -> bool {
        self.drawings.is_empty()
    }

    pub(super) fn get(&self, drawing: u16) -> Option<&Drawing> {
        self.drawings.get(&drawing).map(|drawing| &**drawing)
    }

    pub(super) fn get_mut(&mut self, drawing: u16) -> Option<&mut Drawing> {
        self.drawings.get_mut(&drawing).map(|drawing| &mut **drawing)
    }

    pub(super) fn numbers(&self) -> impl Iterator<Item = u16> + '_ {
        self.drawings.keys().copied()
    }

    pub(super) fn drawings(&self) -> impl Iterator<Item = (u16, &Drawing)> + '_ {
        self.drawings.iter().map(|(number, drawing)| (*number, &**drawing))
    }

    /// The memory it holds: its records as charged and its contents.
    pub(super) const fn held_bytes(&self) -> usize {
        self.records.saturating_add(self.bytes)
    }

    /// The memory it holds that `other` does not share: its records, and
    /// the contents of each drawing that are not the very contents `other`
    /// holds for the same number (a commit copies the staged scene without
    /// copying contents).
    pub(super) fn held_apart_from(&self, other: &Self) -> usize {
        let mut theirs = other.drawings.iter().peekable();
        let mut unshared = 0_usize;
        for (number, drawing) in &self.drawings {
            while theirs.next_if(|(their_number, _)| *their_number < number).is_some() {}
            let shared = theirs.peek().is_some_and(|(their_number, theirs)| {
                *their_number == number && Contents::ptr_eq(&theirs.contents, &drawing.contents)
            });
            if !shared {
                unshared = unshared.saturating_add(drawing.contents.len());
            }
        }
        self.records.saturating_add(unshared)
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

    /// Charges `bytes` more of records, if the cap allows.
    fn charge(&mut self, bytes: usize) -> Result<(), CommitError> {
        let records = self.records.saturating_add(bytes);
        if records > MAX_DRAWING_RECORD_BYTES {
            return Err(CommitError::Drawings);
        }
        self.records = records;
        Ok(())
    }

    /// `B` or `S` on an unused number makes the drawing, with default
    /// attributes. Refused, and nothing made, when its record would take
    /// the records past [`MAX_DRAWING_RECORD_BYTES`].
    pub(super) fn create(&mut self, number: u16) -> Result<(), CommitError> {
        if self.drawings.contains_key(&number) {
            return Ok(());
        }
        self.charge(DRAWING_RECORD_BYTES)?;
        self.drawings.insert(number, Box::new(Drawing::new(self.empty.clone())));
        self.children.insert((0, number));
        Ok(())
    }

    /// Sets an attribute, making the drawing first if need be. Refused,
    /// with nothing changed, when the records would pass their cap.
    pub(super) fn set(&mut self, number: u16, attribute: Attribute) -> Result<(), CommitError> {
        self.create(number)?;
        let had_extras = self.get(number).is_some_and(|drawing| drawing.extras.is_some());
        let wants_extras = matches!(attribute, Attribute::Frames(_) | Attribute::Label(_));
        if wants_extras && !had_extras {
            self.charge(DRAWING_EXTRAS_BYTES)?;
        }
        let Some(drawing) = self.drawings.get_mut(&number) else { return Ok(()) };
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
            Attribute::Frames(frames) => drawing.extras.get_or_insert_default().frames = frames,
            Attribute::Rate(rate) => drawing.rate = Some(rate),
            Attribute::Label(label) => drawing.extras.get_or_insert_default().label = label,
            Attribute::Parent(parent) => {
                let old = core::mem::replace(&mut drawing.parent, parent);
                self.children.remove(&(old, number));
                self.children.insert((parent, number));
            }
        }
        // A frame list and a label both removed give the memory back.
        if let Some(drawing) = self.drawings.get_mut(&number)
            && drawing.extras.as_deref().is_some_and(|extras| *extras == Extras::default())
        {
            drawing.extras = None;
            self.records = self.records.saturating_sub(DRAWING_EXTRAS_BYTES);
        }
        Ok(())
    }

    /// Replaces a drawing's contents, keeping its attributes and children.
    /// Refused, and nothing stored, if the contents of all drawings would
    /// then pass their cap ([`CommitError::Contents`]) or the drawing's
    /// record the records' ([`CommitError::Drawings`]).
    pub(super) fn define(&mut self, number: u16, contents: Vec<u8>) -> Result<(), CommitError> {
        self.create(number)?;
        let drawing = self.drawings.get_mut(&number).ok_or(CommitError::Contents)?;
        let bytes = self.bytes.saturating_sub(drawing.contents.len()).saturating_add(contents.len());
        if bytes > Scene::contents_cap() {
            return Err(CommitError::Contents);
        }
        self.bytes = bytes;
        drawing.contents = if contents.is_empty() { self.empty.clone() } else { Shared::from(contents) };
        drawing.measured = None;
        Ok(())
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
            let extras = if drawing.extras.is_some() { DRAWING_EXTRAS_BYTES } else { 0 };
            self.records = self.records.saturating_sub(DRAWING_RECORD_BYTES.saturating_add(extras));
            self.children.remove(&(drawing.parent, next));
            doomed.extend(self.children.range((next, u16::MIN)..=(next, u16::MAX)).map(|(_, child)| *child));
        }
    }
}
