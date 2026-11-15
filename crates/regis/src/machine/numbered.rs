//! The drawing command, `D`: the one command ReGIS v2 adds
//! (`docs/regis-v2.md`, "The D command" and "Grammar").
//!
//! The interpreter only reads the operations here and queues them; the
//! scene (`crate::scene`) carries them out.

use super::Machine;
use super::frames::Context;
use crate::lexer::Token;
use crate::limits::MAX_DRAWING_NUMBER;
use crate::number::Number;

/// A clip rectangle, relative to its drawing's origin.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Clip {
    pub(crate) x: i32,
    pub(crate) y: i32,
    pub(crate) width: i32,
    pub(crate) height: i32,
}

/// One attribute of a numbered drawing, as `D(S n(...))` sets it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Attribute {
    X(i32),
    Y(i32),
    Depth(i32),
    Visible(bool),
    Parent(u16),
    Source(u16),
    /// The drawing painted in its place while it is hovered, 0 for none.
    Hover(u16),
    /// The drawing painted in its place while it is pressed, 0 for none.
    Pressed(u16),
    Detectable(bool),
    Clip(Option<Clip>),
}

/// One `D` operation, in the order read.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Operation {
    /// `D(B n)`: the contents of drawing `n` follow; `None` when the
    /// operation was malformed, and the contents are then thrown away.
    Begin(Option<u16>),
    /// `D(E)` with no definition open.
    End,
    /// `D(S n ...)`: drawing `n` exists from now on.
    Create(u16),
    Set(u16, Attribute),
    /// `D(K n)`
    Delete(u16),
    /// `D(Z)`
    DeleteAll,
    /// `D(C)`
    Commit,
    /// An operation that could not be read; the drawing concerned, or 0.
    Malformed(u16),
}

/// The option letter kept once `S` has its drawing number, so that only
/// then may its attributes open. Letters arrive in upper case.
const SET_NAMED: u8 = b's';

/// The operation that always stands alone in its `D` command, once read.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
enum Alone {
    #[default]
    None,
    /// `B`, its number still to come.
    Begin,
    /// `B n`
    BeginNumbered(u16),
    /// `E`
    End,
}

/// What has been read of the `D` command in progress.
#[derive(Clone, Copy, Debug, Default)]
pub(crate) struct Numbered {
    /// Its parenthesis is open.
    open: bool,
    /// Operations begun in it.
    count: u32,
    alone: Alone,
    /// Something in it was malformed.
    bad: bool,
    /// A malformed operation was read: everything up to the next
    /// semicolon is dropped.
    skip: bool,
    /// The drawing `S` names.
    target: u16,
    /// The clip rectangle being read: x, y, width, height.
    clip: [i32; 4],
}

/// A drawing number: 1 to 65,535.
fn drawing_number(number: Number) -> Option<u16> {
    u16::try_from(number.wide()).ok().filter(|value| (1..=MAX_DRAWING_NUMBER).contains(value))
}

/// A drawing number, or 0 for none.
fn drawing_reference(number: Number) -> Option<u16> {
    u16::try_from(number.wide()).ok()
}

/// 0 or 1.
fn flag(number: Number) -> Option<bool> {
    match number.wide() {
        0 => Some(false),
        1 => Some(true),
        _ => None,
    }
}

impl Machine {
    /// `D(`
    pub(super) fn open_drawing_command(&mut self) {
        self.numbered = Numbered { open: true, ..Numbered::default() };
    }

    /// Whether a `D` command is being read: its bytes count against the
    /// staged operations cap.
    pub(crate) fn in_drawing_command(&self) -> bool {
        self.syntax.command == Some(super::frames::Command::Drawing)
    }

    /// A `D` operation could not be read. The batch will fail at its
    /// commit, and reading starts again at the next semicolon; but `D(B`
    /// is read to its end, so that the contents after it are taken and
    /// thrown away, never drawn on the base.
    fn malformed_drawing(&mut self) {
        let first = !core::mem::replace(&mut self.numbered.bad, true);
        if !matches!(self.numbered.alone, Alone::Begin | Alone::BeginNumbered(_)) {
            if first {
                self.operations.push(Operation::Malformed(self.numbered.target));
            }
            self.numbered.skip = true;
        }
    }

    /// A semicolon: a `D` command still open is malformed.
    pub(super) fn abandon_drawing_command(&mut self) {
        if self.numbered.open && !self.numbered.bad {
            self.operations.push(Operation::Malformed(self.numbered.target));
        }
        self.numbered = Numbered::default();
    }

    /// Whether tokens are being dropped up to the next semicolon.
    pub(super) const fn skipping_to_resync(&self) -> bool {
        self.numbered.skip
    }

    /// A token directly inside `D(...)`.
    pub(super) fn drawing_option(&mut self, token: &Token) {
        match *token {
            Token::Letter(letter) => self.drawing_letter(letter),
            Token::Number(number) => self.drawing_operand(number),
            Token::Comma => self.syntax.set_option_letter(0),
            Token::Open if self.syntax.option_letter() == SET_NAMED => {
                self.syntax.set_option_letter(0);
                self.syntax.open(Context::DrawingSet);
            }
            Token::Open => {
                self.syntax.open(Context::Skip);
                self.malformed_drawing();
            }
            _ => self.malformed_drawing(),
        }
    }

    fn drawing_letter(&mut self, letter: u8) {
        let numbered = &mut self.numbered;
        // `B` and `E` always stand alone.
        let alone = numbered.count == 0;
        let after_alone = numbered.alone != Alone::None;
        numbered.count = numbered.count.saturating_add(1);
        self.syntax.set_option_letter(letter);
        match letter {
            b'B' => {
                self.numbered.alone = Alone::Begin;
                if !alone {
                    self.malformed_drawing();
                }
            }
            b'E' if alone => self.numbered.alone = Alone::End,
            b'S' | b'K' if !after_alone => {}
            b'Z' if !after_alone => self.operations.push(Operation::DeleteAll),
            b'C' if !after_alone => self.operations.push(Operation::Commit),
            _ => self.malformed_drawing(),
        }
    }

    fn drawing_operand(&mut self, number: Number) {
        let letter = self.syntax.option_letter();
        self.syntax.set_option_letter(0);
        match (letter, drawing_number(number)) {
            (b'B', Some(drawing)) if self.numbered.alone == Alone::Begin => {
                self.numbered.alone = Alone::BeginNumbered(drawing);
            }
            (b'S', Some(drawing)) => {
                self.numbered.target = drawing;
                self.operations.push(Operation::Create(drawing));
                self.syntax.set_option_letter(SET_NAMED);
            }
            (b'K', Some(drawing)) => self.operations.push(Operation::Delete(drawing)),
            _ => self.malformed_drawing(),
        }
    }

    /// `)` of `D(...)`.
    pub(super) fn close_drawing_command(&mut self) {
        let numbered = core::mem::take(&mut self.numbered);
        match numbered.alone {
            Alone::BeginNumbered(drawing) if !numbered.bad => self.operations.push(Operation::Begin(Some(drawing))),
            Alone::Begin | Alone::BeginNumbered(_) => self.operations.push(Operation::Begin(None)),
            Alone::End if !numbered.bad => self.operations.push(Operation::End),
            Alone::End | Alone::None => {}
        }
    }

    /// A token inside `D(S n(...))`.
    pub(super) fn drawing_attribute(&mut self, token: &Token) {
        match *token {
            Token::Letter(letter @ (b'X' | b'Y' | b'L' | b'V' | b'P' | b'R' | b'H' | b'A' | b'I' | b'C')) => {
                self.syntax.set_option_letter(letter);
            }
            Token::Number(number) => {
                let letter = self.syntax.option_letter();
                self.syntax.set_option_letter(0);
                // Positions and depths are 16-bit signed, as coordinates
                // are; a sign is part of the value.
                let attribute = match letter {
                    b'X' => Some(Attribute::X(number.value())),
                    b'Y' => Some(Attribute::Y(number.value())),
                    b'L' => Some(Attribute::Depth(number.value())),
                    b'V' => flag(number).map(Attribute::Visible),
                    b'I' => flag(number).map(Attribute::Detectable),
                    b'P' => drawing_reference(number).map(Attribute::Parent),
                    b'R' => drawing_reference(number).map(Attribute::Source),
                    b'H' => drawing_reference(number).map(Attribute::Hover),
                    // `A` for active: `P` is the parent.
                    b'A' => drawing_reference(number).map(Attribute::Pressed),
                    b'C' if number.wide() == 0 => Some(Attribute::Clip(None)),
                    _ => None,
                };
                match attribute {
                    Some(attribute) => self.operations.push(Operation::Set(self.numbered.target, attribute)),
                    None => self.malformed_drawing(),
                }
            }
            Token::Comma => self.syntax.set_option_letter(0),
            Token::Open if self.syntax.option_letter() == b'C' => {
                self.numbered.clip = [0; 4];
                self.syntax.open(Context::DrawingClip);
            }
            Token::Open => {
                self.syntax.open(Context::Skip);
                self.malformed_drawing();
            }
            _ => self.malformed_drawing(),
        }
    }

    /// A token inside `C(...)`.
    pub(super) fn drawing_clip(&mut self, token: &Token) {
        match *token {
            Token::Letter(letter @ (b'X' | b'Y' | b'W' | b'H')) => self.syntax.set_option_letter(letter),
            Token::Number(number) => {
                let index = match self.syntax.option_letter() {
                    b'X' => 0,
                    b'Y' => 1,
                    b'W' => 2,
                    b'H' => 3,
                    _ => usize::MAX,
                };
                self.syntax.set_option_letter(0);
                match self.numbered.clip.get_mut(index) {
                    Some(part) => *part = number.value(),
                    None => self.malformed_drawing(),
                }
            }
            Token::Comma => self.syntax.set_option_letter(0),
            Token::Open => {
                self.syntax.open(Context::Skip);
                self.malformed_drawing();
            }
            _ => self.malformed_drawing(),
        }
    }

    /// `)` of `C(...)`.
    pub(super) fn close_drawing_clip(&mut self) {
        self.syntax.set_option_letter(0);
        let [x, y, width, height] = self.numbered.clip;
        if width < 0 || height < 0 {
            self.malformed_drawing();
        } else {
            let clip = Clip { x, y, width, height };
            self.operations.push(Operation::Set(self.numbered.target, Attribute::Clip(Some(clip))));
        }
    }
}
