//! The text command, `T` (manual chapter 7).

use super::Machine;
use super::command::errors;
use super::frames::Context;
use super::lettering::LOADABLE_SETS;
use crate::font::Glyph;
use crate::geometry::Point;
use crate::lexer::Token;
use crate::limits::MAX_TEXT_STACK;
use crate::number::Number;
use crate::surface::Screen;
use crate::text::draw_character;

const BACKSPACE: u8 = 0x08;
const TAB: u8 = 0x09;
const LINE_FEED: u8 = 0x0A;
const CARRIAGE_RETURN: u8 = 0x0D;

impl Machine {
    pub(super) fn text_option(&mut self, token: &Token) {
        match *token {
            Token::Letter(b'B') => {
                if self.text.saved.len() < MAX_TEXT_STACK {
                    self.text.saved.push(self.text.controls);
                }
            }
            Token::Letter(b'E') => {
                if let Some(saved) = self.text.saved.pop() {
                    self.text.controls = saved;
                }
            }
            Token::Letter(letter) => self.syntax.set_option_letter(letter),
            Token::Number(number) => self.text_number(number),
            Token::Extent(extent) => {
                let (width, height) = (extent.x.map(|axis| axis.value), extent.y.map(|axis| axis.value));
                match self.syntax.option_letter() {
                    b'S' => self.text.controls.set_display(width, height),
                    b'U' => self.text.controls.set_unit(width, height),
                    b'M' => self.text.controls.set_size_multiplier(width, height),
                    _ => {}
                }
            }
            Token::Open if self.syntax.option_letter() == b'W' => self.open_temporary_write(),
            Token::Open if self.syntax.option_letter() == b'A' => self.syntax.open(Context::Designators),
            Token::Open => self.syntax.open(Context::Skip),
            _ => {}
        }
    }

    fn text_number(&mut self, number: Number) {
        let value = number.value();
        match self.syntax.option_letter() {
            b'A' => {
                if let Ok(alphabet @ 0..=LOADABLE_SETS) = usize::try_from(value) {
                    self.text.controls.alphabet = alphabet;
                }
            }
            b'S' => {
                if !self.text.controls.set_standard(value) {
                    self.note_fault(errors::TEXT_SIZE, 0);
                }
            }
            b'H' => self.text.controls.set_height_multiplier(value),
            b'D' => {
                self.text.controls.set_tilt(number.rounded(), self.text.tilted);
                self.text.tilted = true;
            }
            b'I' => self.text.controls.set_italic(value),
            _ => {}
        }
    }

    /// A pixel-vector digit in a text command: half a cell's move, for
    /// subscripts, superscripts and overstrikes.
    pub(super) fn text_half_cell(&mut self, digit: u8) {
        let offset = self.text.controls.half_cell(i32::from(digit));
        self.shift(offset);
    }

    /// Moves the active position by an offset in pixels.
    fn shift(&mut self, (dx, dy): (i32, i32)) {
        let at = self.pixel();
        let moved = Point::new(at.x.saturating_add(dx), at.y.saturating_add(dy));
        self.position = self.addressing.to_user(moved);
    }

    /// The shape of a character in the set now selected.
    pub(super) fn glyph(&self, code: u8, unit_height: i32) -> Glyph {
        self.characters.glyph(self.text.controls.alphabet, code, unit_height)
    }

    /// Draws a quoted string from the active position.
    pub(super) fn draw_string(&mut self, screen: &mut Screen, text: &[u8]) {
        for &code in text {
            let (dx, dy) = self.text.controls.advance();
            match code {
                CARRIAGE_RETURN => self.position = self.text.line_start,
                LINE_FEED => {
                    let down = self.text.controls.line_feed();
                    let start = self.addressing.to_pixel(self.text.line_start);
                    let moved = Point::new(start.x.saturating_add(down.0), start.y.saturating_add(down.1));
                    self.text.line_start = self.addressing.to_user(moved);
                    self.shift(down);
                }
                BACKSPACE => self.shift((dx.saturating_neg(), dy.saturating_neg())),
                TAB => self.shift((dx, dy)),
                _ => {
                    let glyph = self.glyph(code, self.text.controls.unit.1);
                    let (origin, controls) = (self.pixel(), self.text.controls);
                    self.paint(screen, |painter| draw_character(painter, &glyph, origin, &controls));
                    self.shift((dx, dy));
                }
            }
        }
        self.pen.chain = None;
    }
}
