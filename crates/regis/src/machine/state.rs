//! The interpreter's state.

use alloc::vec::Vec;

use super::colour::ColourSpec;
use super::curve::{ArcOptions, Curve};
use super::fill::Fill;
use super::lettering::{Characters, Text};
use super::numbered::{Numbered, Operation};
use super::outbox::Outbox;
use super::pen::Pen;
use super::syntax::Syntax;
use super::write::ShadeOption;
use crate::addressing::Addressing;
use crate::event::Event;
use crate::font::Font;
use crate::geometry::Point;
use crate::macrograph::Macrographs;
use crate::palette::Palette;
use crate::work::Work;
use crate::write::WriteControls;

/// What the `R(E)` report answers with: an error code and the character
/// at fault (manual chapter 10, table 10-1).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct Fault {
    pub(crate) code: u8,
    pub(crate) character: u8,
}

/// Whose commands a machine carries out.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Role {
    /// The stream's, on the base picture.
    Base,
    /// A numbered drawing's contents: `S`, `L`, `R` and `D` have no effect
    /// there (`docs/regis-v2.md`, "Rules for contents").
    Drawing,
}

/// Where the byte being read came from; `D` is honoured only in the
/// stream as received.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Reading {
    Stream,
    Macrograph,
}

/// Everything ReGIS remembers between bytes.
#[derive(Debug)]
pub(crate) struct Machine {
    /// The active position, in user coordinates.
    pub(super) position: Point,
    pub(super) addressing: Addressing,
    /// The page drawing goes to.
    pub(super) page: usize,
    /// Positions saved by `(B)`; `None` is the placeholder `(S)` saves.
    pub(super) stack: Vec<Option<Point>>,
    pub(super) pen: Pen,
    pub(crate) palette: Palette,
    pub(super) text: Text,
    pub(crate) characters: Characters,

    // Figures in progress.
    pub(super) arc: ArcOptions,
    pub(super) curve: Option<Curve>,
    pub(super) fill: Option<Fill>,

    // Options that arrive in pieces.
    pub(super) syntax: Syntax,
    /// The first corner of a display addressing option, awaiting the second.
    pub(super) corner: Option<Point>,
    pub(super) colour: ColourSpec,
    pub(super) map_location: Option<usize>,
    pub(super) shade_option: ShadeOption,

    pub(super) cursor_visible: bool,
    pub(super) fault: Fault,
    pub(super) macrograph_reports: bool,
    pub(crate) macrographs: Macrographs,
    pub(crate) outbox: Outbox,
    pub(crate) work: Work,

    // ReGIS v2.
    pub(crate) role: Role,
    pub(crate) reading: Reading,
    pub(super) numbered: Numbered,
    /// `D` operations read and not yet collected by the terminal.
    pub(crate) operations: Vec<Operation>,
}

impl Machine {
    /// The memory its working state holds beyond its structure, the font
    /// and provided sets aside.
    pub(crate) fn heap_bytes(&self) -> usize {
        use crate::limits::vector_bytes;
        [
            vector_bytes(&self.stack),
            vector_bytes(&self.text.saved),
            self.characters.cell.as_ref().map_or(0, super::load::LoadingCell::heap_bytes),
            self.curve.as_ref().map_or(0, Curve::heap_bytes),
            self.fill.as_ref().map_or(0, Fill::heap_bytes),
            self.syntax.heap_bytes(),
            self.macrographs.heap_bytes(),
            self.outbox.heap_bytes(),
            vector_bytes(&self.operations),
            self.characters.sets.iter().map(crate::font::Loadable::heap_bytes).fold(0, usize::saturating_add),
        ]
        .into_iter()
        .fold(0, usize::saturating_add)
    }

    /// The state at power-up (manual chapter 1, table 1-2).
    pub(crate) fn new() -> Self {
        Self {
            position: Point::default(),
            addressing: Addressing::default(),
            page: 0,
            stack: Vec::new(),
            pen: Pen::default(),
            palette: Palette::default(),
            text: Text::default(),
            characters: Characters::default(),
            arc: ArcOptions::default(),
            curve: None,
            fill: None,
            syntax: Syntax::default(),
            corner: None,
            colour: ColourSpec::default(),
            map_location: None,
            shade_option: ShadeOption::default(),
            cursor_visible: true,
            fault: Fault::default(),
            macrograph_reports: false,
            macrographs: Macrographs::default(),
            outbox: Outbox::default(),
            work: Work::new(),
            role: Role::Base,
            reading: Reading::Stream,
            numbered: Numbered::default(),
            operations: Vec::new(),
        }
    }

    /// Whether the commands are a numbered drawing's contents.
    pub(crate) fn inside_drawing(&self) -> bool {
        self.role == Role::Drawing
    }

    pub(crate) fn set_font(&mut self, font: Font) {
        self.characters.font = font;
    }

    pub(crate) fn set_provided_set(&mut self, number: usize, set: crate::provided::ProvidedSet) {
        self.characters.set_provided(number, set);
    }

    pub(crate) fn set_macrograph_reports(&mut self, allowed: bool) {
        self.macrograph_reports = allowed;
    }

    pub(crate) const fn palette(&self) -> &Palette {
        &self.palette
    }

    pub(crate) const fn position(&self) -> (i32, i32) {
        (self.position.x, self.position.y)
    }

    pub(crate) const fn cursor_visible(&self) -> bool {
        self.cursor_visible
    }

    /// Credits every budget for one byte of new input. Crediting byte by
    /// byte makes the result independent of how the input is divided
    /// between calls (SOLVEDSECURITYISSUES 008).
    pub(crate) fn earn(&mut self) {
        self.work.earn(1);
        self.outbox.earn();
    }

    /// Hands the queued events to the caller, adding a limit notice if any
    /// work was refused.
    pub(crate) fn take_events(&mut self) -> Vec<Event> {
        if self.work.take_exhausted() {
            self.outbox.limit_reached();
        }
        self.outbox.take()
    }

    /// Records a character that was ignored (error 1, manual table 10-1).
    pub(crate) fn note_ignored(&mut self, character: u8) {
        self.note_fault(super::command::errors::IGNORED_CHARACTER, character);
    }

    /// Records a parser error for the error report.
    pub(super) fn note_fault(&mut self, code: u8, character: u8) {
        self.fault = Fault { code, character };
    }

    /// The write controls now in force.
    pub(super) fn controls(&self) -> WriteControls {
        self.pen.controls()
    }

    /// The write controls an option changes.
    pub(super) fn controls_mut(&mut self, temporary: bool) -> &mut WriteControls {
        self.pen.controls_mut(temporary)
    }
}
