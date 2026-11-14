//! The write control command, `W` (manual chapter 3).

use super::Machine;
use super::frames::{ColourTarget, Context};
use super::screen::colour_location;
use crate::convert::to_u32;
use crate::font::CELL_WIDTH;
use crate::lexer::{Extent, Token};
use crate::limits::{MAX_COORDINATE, MAX_PATTERN_MULTIPLIER};
use crate::number::Number;
use crate::write::{Shade, Style, Tile};

/// What a shading option has said so far: `W(S'X'(X)[400])` arrives in
/// pieces.
#[derive(Clone, Copy, Debug, Default)]
pub(crate) struct ShadeOption {
    vertical: bool,
    character: Option<u8>,
}

/// The planes a VT340 has: a mask of four bits (manual chapter 3, table 3-3).
const ALL_PLANES: i32 = 15;
/// The rows of a stored cell a shading character shows of its ten.
const SHADING_ROWS: i32 = 8;
const CELL_ROWS: i32 = 10;

impl Machine {
    /// An option of `W`, for the lasting or the temporary controls.
    pub(super) fn write_option(&mut self, token: &Token, temporary: bool) {
        match *token {
            Token::Letter(letter) => self.write_letter(letter, temporary),
            Token::Number(number) => self.write_number(number, temporary),
            Token::Extent(extent) if self.syntax.option_letter() == b'S' => self.shade_to(extent, temporary),
            Token::Text(ref text) if self.syntax.option_letter() == b'S' => {
                self.shade_option.character = text.first().copied();
                self.shade_to(Extent::default(), temporary);
            }
            Token::Open => match self.syntax.option_letter() {
                b'P' => self.syntax.open(Context::PatternMultiplier { temporary }),
                b'I' => self.open_colour(ColourTarget::Foreground { temporary }),
                b'S' => self.syntax.open(Context::ShadeAxis),
                _ => self.syntax.open(Context::Skip),
            },
            _ => {}
        }
    }

    fn write_letter(&mut self, letter: u8, temporary: bool) {
        let style = match letter {
            b'V' => Some(Style::Overlay),
            b'R' => Some(Style::Replace),
            b'C' => Some(Style::Complement),
            b'E' => Some(Style::Erase),
            _ => None,
        };
        if let Some(style) = style {
            self.controls_mut(temporary).style = style;
        }
        if letter == b'S' {
            self.shade_option = ShadeOption::default();
        }
        self.syntax.set_option_letter(letter);
    }

    fn write_number(&mut self, number: Number, temporary: bool) {
        let value = number.value();
        match self.syntax.option_letter() {
            b'M' => self.controls_mut(temporary).pv_multiplier = value.clamp(0, MAX_COORDINATE),
            b'N' => self.controls_mut(temporary).negative = value != 0,
            b'I' => {
                if let Some(location) = colour_location(value) {
                    self.controls_mut(temporary).foreground = location;
                }
            }
            b'F' => {
                if let Some(planes) = u8::try_from(value).ok().filter(|_| value <= ALL_PLANES) {
                    self.controls_mut(temporary).planes = planes;
                }
            }
            b'P' => self.controls_mut(temporary).set_pattern(number.digits, number.bits, value),
            b'S' if value == 0 => self.controls_mut(temporary).shade = None,
            b'S' => self.shade_to(Extent::default(), temporary),
            _ => {}
        }
    }

    /// `W(P n(M n))`: how many pixels each pattern bit covers.
    pub(super) fn pattern_multiplier(&mut self, token: &Token, temporary: bool) {
        match *token {
            Token::Letter(letter) => self.syntax.set_option_letter(letter),
            Token::Number(number) if self.syntax.option_letter() == b'M' => {
                self.controls_mut(temporary).pattern_multiplier =
                    to_u32(number.value()).clamp(1, MAX_PATTERN_MULTIPLIER);
            }
            Token::Open => self.syntax.open(Context::Skip),
            _ => {}
        }
    }

    /// `W(S(X))`: the reference line is vertical.
    pub(super) fn shade_axis(&mut self, token: &Token) {
        match *token {
            Token::Letter(b'X') => self.shade_option.vertical = true,
            Token::Open => self.syntax.open(Context::Skip),
            _ => {}
        }
    }

    /// Turns shading on with its reference line through the place `extent`
    /// names; parts left out are the active position's.
    fn shade_to(&mut self, extent: Extent, temporary: bool) {
        let reference = self.addressing.to_pixel(self.resolve(extent));
        let option = self.shade_option;
        let tile = option.character.map(|code| self.shading_tile(code));
        let at = if option.vertical { reference.x } else { reference.y };
        self.controls_mut(temporary).shade = Some(Shade { vertical: option.vertical, at, tile });
    }

    /// The shading character's shape at the text size now in force; only
    /// "the top 8 × 8 matrix of the 8 × 10 character cell" shows.
    fn shading_tile(&self, code: u8) -> Tile {
        let glyph = self.glyph(code, CELL_ROWS);
        let (width, height) = self.text.controls.unit;
        let tile_height = height.saturating_mul(SHADING_ROWS).checked_div(CELL_ROWS).unwrap_or(SHADING_ROWS);
        Tile { rows: glyph.top_rows(), width: width.max(CELL_WIDTH), height: tile_height.max(1) }
    }
}
