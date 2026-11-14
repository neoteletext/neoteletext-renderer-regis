//! Colour specifiers: `(R)`, `(H120L50S100)`, `(L33)` (manual chapter 2,
//! "Output Mapping Control").

use super::Machine;
use super::frames::{ColourTarget, Context};
use crate::lexer::Token;
use crate::palette::{hls_to_rgb, named};

/// A colour being read between parentheses.
#[derive(Clone, Copy, Debug, Default)]
pub(crate) struct ColourSpec {
    hue: Option<i32>,
    lightness: Option<i32>,
    saturation: Option<i32>,
    named: Option<[u8; 3]>,
}

impl ColourSpec {
    /// The colour read, if any part of one was.
    fn colour(&self) -> Option<[u8; 3]> {
        if self.named.is_some() {
            return self.named;
        }
        if self.hue.is_none() && self.lightness.is_none() && self.saturation.is_none() {
            return None;
        }
        // A lightness alone is a shade of grey.
        Some(hls_to_rgb(self.hue.unwrap_or(0), self.lightness.unwrap_or(0), self.saturation.unwrap_or(0)))
    }
}

impl Machine {
    pub(super) fn open_colour(&mut self, target: ColourTarget) {
        self.colour = ColourSpec::default();
        self.syntax.open(Context::Colour(target));
    }

    pub(super) fn colour_token(&mut self, token: &Token) {
        match *token {
            // H, L and S take a number; any other letter may name a colour.
            // `A` only marks the colour (not monochrome) value and changes
            // nothing on a VT340.
            Token::Letter(letter @ (b'H' | b'L' | b'S')) => self.syntax.set_option_letter(letter),
            Token::Letter(b'A') => {}
            Token::Letter(letter) => {
                self.syntax.set_option_letter(0);
                if let Some(colour) = named(letter) {
                    self.colour.named = Some(colour);
                }
            }
            Token::Number(number) => {
                let value = Some(number.value());
                match self.syntax.option_letter() {
                    b'H' => self.colour.hue = value,
                    b'L' => self.colour.lightness = value,
                    b'S' => self.colour.saturation = value,
                    _ => {}
                }
            }
            Token::Open => self.syntax.open(Context::Skip),
            _ => {}
        }
    }

    /// `)` after a colour: use it for what it was written for.
    pub(super) fn finish_colour(&mut self, target: ColourTarget) {
        let Some(colour) = self.colour.colour() else { return };
        match target {
            ColourTarget::Map => {
                if let Some(location) = self.map_location {
                    self.palette.set(location, colour);
                }
            }
            ColourTarget::Background => self.pen.background = self.palette.nearest(colour),
            ColourTarget::Foreground { temporary } => {
                let nearest = self.palette.nearest(colour);
                self.controls_mut(temporary).foreground = nearest;
            }
        }
    }
}
