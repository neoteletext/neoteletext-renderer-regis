//! The position (`P`) and vector (`V`) commands (manual chapters 4 and 5),
//! and the routing of arguments to every command.

use super::Machine;
use super::command::errors;
use super::frames::{Command, Context};
use crate::lexer::{Extent, Token};
use crate::limits::MAX_POSITION_STACK;
use crate::surface::Screen;

/// The highest pixel-vector digit (manual chapter 1, figure 1-2).
const MAX_PIXEL_VECTOR: u8 = 7;

impl Machine {
    /// A bracketed extent given to a command.
    pub(super) fn extent_argument(&mut self, screen: &mut Screen, command: Command, extent: Extent) {
        match command {
            Command::Position => self.move_to(self.resolve(extent)),
            Command::Vector if extent.is_null() => self.dot(screen),
            Command::Vector => self.line_to(screen, self.resolve(extent)),
            Command::Curve => self.curve_argument(screen, self.resolve(extent)),
            // Spacing is a displacement in the stream's coordinates.
            Command::Text => self.text.controls.spacing = Some(self.addressing.offset(axis(extent.x), axis(extent.y))),
            Command::Screen => self.scroll_to(screen, extent),
            Command::Write | Command::Load | Command::Report | Command::Fill => {}
        }
    }

    /// A pixel-vector digit given to a command.
    pub(super) fn digit_argument(&mut self, screen: &mut Screen, command: Command, digit: u8) {
        if digit > MAX_PIXEL_VECTOR {
            self.note_fault(errors::IGNORED_CHARACTER, digit.saturating_add(b'0'));
            return;
        }
        match command {
            Command::Position => self.move_to(self.pixel_vector(digit)),
            Command::Vector => self.line_to(screen, self.pixel_vector(digit)),
            Command::Curve => self.curve_argument(screen, self.pixel_vector(digit)),
            Command::Text => self.text_half_cell(digit),
            Command::Screen => {
                // A pixel vector scrolls by the multiplier in its screen direction.
                let (x, y) = crate::geometry::direction(i32::from(digit));
                let steps = self.addressing.length(self.controls().pv_multiplier);
                self.scroll(screen, x.saturating_mul(steps), y.saturating_mul(steps));
            }
            Command::Write | Command::Load | Command::Report | Command::Fill => {}
        }
    }

    pub(super) fn move_to(&mut self, target: crate::geometry::Point) {
        self.position = target;
        self.pen.chain = None;
    }

    /// An option of `P` or, with `draws`, of `V`.
    pub(super) fn position_option(&mut self, screen: &mut Screen, token: &Token, draws: bool) {
        match *token {
            Token::Letter(b'B') => self.save_position(true),
            Token::Letter(b'S') => self.save_position(false),
            Token::Letter(b'E') => self.restore_position(screen, draws),
            Token::Letter(letter) => self.syntax.set_option_letter(letter),
            Token::Number(number) if self.syntax.option_letter() == b'P' && !draws => {
                // `P(P n)`: the page drawing goes to.
                if let Ok(page @ 0..=1) = usize::try_from(number.value()) {
                    self.page = page;
                }
            }
            Token::Open if self.syntax.option_letter() == b'W' => self.open_temporary_write(),
            Token::Open => self.syntax.open(Context::Skip),
            _ => {}
        }
    }

    /// `(B)` saves the position; `(S)` saves a placeholder.
    fn save_position(&mut self, bounded: bool) {
        if self.stack.len() >= MAX_POSITION_STACK {
            self.note_fault(errors::STACK_OVERFLOW, if bounded { b'B' } else { b'S' });
            return;
        }
        self.stack.push(bounded.then_some(self.position));
    }

    /// `(E)`: back to the position saved by `(B)`, drawing there for a
    /// vector command; after `(S)` nothing moves.
    fn restore_position(&mut self, screen: &mut Screen, draws: bool) {
        match self.stack.pop() {
            None => self.note_fault(errors::STACK_UNDERFLOW, b'E'),
            Some(None) => {}
            Some(Some(saved)) if draws => self.line_to(screen, saved),
            Some(Some(saved)) => self.move_to(saved),
        }
    }
}

/// An extent part as a plain offset; a part left out is zero.
fn axis(part: Option<crate::lexer::Axis>) -> i32 {
    part.map_or(0, |axis| axis.value)
}
