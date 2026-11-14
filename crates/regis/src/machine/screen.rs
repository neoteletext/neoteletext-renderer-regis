//! The screen control command, `S` (manual chapter 2).

use super::Machine;
use super::frames::{ColourTarget, Context};
use crate::addressing::Addressing;
use crate::convert::{to_u8, to_u32};
use crate::geometry::Point;
use crate::lexer::{Extent, Token};
use crate::limits::MAX_WAIT_TICKS;
use crate::palette::COLOURS;
use crate::surface::Screen;

/// The pixels of a page: the work an erase or a scroll costs.
const PAGE_AREA: u64 = crate::SCREEN_WIDTH as u64 * crate::SCREEN_HEIGHT as u64;

impl Machine {
    pub(super) fn screen_option(&mut self, screen: &mut Screen, token: &Token) {
        match *token {
            Token::Letter(b'E') => self.erase(screen),
            Token::Letter(letter) => {
                self.syntax.set_option_letter(letter);
                match letter {
                    b'A' => self.corner = None,
                    // A location is named afresh by each M option.
                    b'M' => self.map_location = None,
                    _ => {}
                }
            }
            Token::Number(number) => self.screen_number(screen, number.value()),
            Token::Extent(extent) if self.syntax.option_letter() == b'A' => self.addressing_corner(extent),
            Token::Open => self.screen_open(),
            _ => {}
        }
    }

    fn screen_number(&mut self, screen: &mut Screen, value: i32) {
        match self.syntax.option_letter() {
            b'M' => self.map_location = usize::try_from(value).ok().filter(|location| *location < COLOURS),
            b'I' => {
                if let Some(location) = colour_location(value) {
                    self.pen.background = location;
                }
            }
            b'T' => self.outbox.wait(to_u32(value.clamp(0, MAX_WAIT_TICKS))),
            b'C' => self.cursor_visible = value != 0,
            b'P' => {
                if let Ok(page) = usize::try_from(value) {
                    screen.display(page);
                }
            }
            _ => {}
        }
    }

    fn screen_open(&mut self) {
        match self.syntax.option_letter() {
            b'M' => self.open_colour(ColourTarget::Map),
            b'I' => self.open_colour(ColourTarget::Background),
            b'W' => self.open_temporary_write(),
            // Hard copy (`H`) and cursor styles (`C`) draw nothing here.
            _ => self.syntax.open(Context::Skip),
        }
    }

    /// One corner of `S(A[x1,y1][x2,y2])`. Both corners must be given in
    /// full "or ReGIS ignores the command".
    fn addressing_corner(&mut self, extent: Extent) {
        let (Some(x), Some(y)) = (extent.x, extent.y) else {
            self.corner = None;
            return;
        };
        let point = Point::new(x.value, y.value);
        match self.corner.take() {
            None => self.corner = Some(point),
            Some(first) => {
                if let Some(addressing) = Addressing::new(first, point) {
                    self.addressing = addressing;
                }
            }
        }
    }

    /// `S(E)`: the whole page to the background colour. Also turns shading
    /// off, stops any curve and empties the position stack (manual chapter
    /// 2, "Screen Erase").
    fn erase(&mut self, screen: &mut Screen) {
        // Clearing the page is paid for like any drawing; an erase that
        // cannot be afforded is not done.
        if self.work.spend(PAGE_AREA)
            && let Some(canvas) = screen.page_mut(self.page)
        {
            canvas.clear(self.pen.background);
        }
        self.pen.write.shade = None;
        if let Some(temporary) = &mut self.pen.temporary {
            temporary.shade = None;
        }
        self.curve = None;
        self.stack.clear();
        self.syntax.set_option_letter(b'E');
    }

    /// `S[x,y]`: "the position you specify becomes the new upper-left
    /// corner of the screen" (manual chapter 2, "Scroll Argument"). A
    /// signed part is measured from the present corner; a part left out
    /// keeps it.
    pub(super) fn scroll_to(&mut self, screen: &mut Screen, extent: Extent) {
        let corner = self.addressing.origin();
        let target = Point::new(
            extent.x.map_or(corner.x, |axis| axis.resolve(corner.x)),
            extent.y.map_or(corner.y, |axis| axis.resolve(corner.y)),
        );
        let pixel = self.addressing.to_pixel(target);
        self.scroll(screen, pixel.x, pixel.y);
    }

    /// Moves the picture so that the pixel (`dx`, `dy`) lands on the
    /// top-left corner.
    pub(super) fn scroll(&mut self, screen: &mut Screen, dx: i32, dy: i32) {
        // Moving every pixel of the page is paid for like any drawing.
        if (dx, dy) != (0, 0)
            && self.work.spend(PAGE_AREA)
            && let Some(canvas) = screen.page_mut(self.page)
        {
            canvas.scroll(dx, dy, self.pen.background);
        }
    }
}

/// An output map location from a number, if it is one.
pub(super) fn colour_location(value: i32) -> Option<u8> {
    usize::try_from(value).ok().filter(|location| *location < COLOURS).map(|_| to_u8(value))
}
