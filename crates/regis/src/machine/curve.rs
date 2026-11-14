//! The curve command, `C`: circles, arcs and interpolated curves (manual
//! chapter 6).

use alloc::vec::Vec;

use super::Machine;
use super::frames::Context;
use crate::geometry::Point;
use crate::lexer::Token;
use crate::limits::{ARC_STEP_COST, CURVE_WINDOW};
use crate::raster::{arc_points, span_points};
use crate::surface::Screen;

/// A whole circle, the arc drawn when no angle is given.
const CIRCLE: i32 = 360;
/// How many of a closed curve's first points are needed again to close it.
const CLOSING_POINTS: usize = 3;

/// The options of a circle or arc; they last "until you enter a new command".
#[derive(Clone, Copy, Debug, Default)]
pub(crate) struct ArcOptions {
    /// `(C)`: the argument is the centre and the active position is on the
    /// circumference, instead of the other way about.
    centre_given: bool,
    /// `(A n)`: the arc's angle; positive is counterclockwise.
    degrees: Option<i32>,
}

/// An interpolated curve in progress, in pixels.
#[derive(Clone, Debug)]
pub(crate) struct Curve {
    closed: bool,
    /// The first points, kept to close the curve.
    first: Vec<Point>,
    /// The last four points: the curve is drawn between the middle two.
    recent: Vec<Point>,
}

impl Machine {
    pub(super) fn curve_option(&mut self, screen: &mut Screen, token: &Token) {
        match *token {
            Token::Letter(b'C') => self.arc.centre_given = true,
            Token::Letter(b'B') => self.begin_curve(true),
            Token::Letter(b'S') => self.begin_curve(false),
            Token::Letter(b'E') => self.end_curve(screen),
            Token::Letter(letter) => self.syntax.set_option_letter(letter),
            Token::Number(number) if self.syntax.option_letter() == b'A' => {
                self.arc.degrees = Some(number.rounded().clamp(-CIRCLE, CIRCLE));
            }
            Token::Open if self.syntax.option_letter() == b'W' => self.open_temporary_write(),
            Token::Open => self.syntax.open(Context::Skip),
            _ => {}
        }
    }

    /// A position given to the curve command.
    pub(super) fn curve_argument(&mut self, screen: &mut Screen, target: Point) {
        if self.curve.is_some() {
            self.position = target;
            self.curve_point(screen, self.addressing.to_pixel(target));
        } else {
            self.arc(screen, target);
        }
    }

    /// A circle or arc. With the centre at the active position the cursor
    /// does not move; with the centre given, an arc leaves the cursor at
    /// its end so that arcs can be linked.
    fn arc(&mut self, screen: &mut Screen, target: Point) {
        let options = self.arc;
        let degrees = options.degrees.unwrap_or(CIRCLE);
        let (here, there) = (self.pixel(), self.addressing.to_pixel(target));
        let (centre, start) = if options.centre_given { (there, here) } else { (here, there) };
        // Each degree is a rotation whether or not it draws a pixel, so the
        // steps are paid for before they are taken (SOLVEDSECURITYISSUES 007).
        if !self.work.spend(ARC_STEP_COST.saturating_mul(u64::from(degrees.unsigned_abs()))) {
            return;
        }

        let mut previous: Option<Point> = None;
        let mut pieces = 0u32;
        arc_points(centre, start, degrees, |point| {
            match previous {
                Some(from) => {
                    self.arc_piece(screen, from, point, pieces == 0 && degrees.abs() == CIRCLE);
                    pieces = pieces.saturating_add(1);
                }
                None => {
                    if let Some(fill) = &mut self.fill {
                        fill.vertex(point);
                    }
                }
            }
            previous = Some(point);
        });
        if pieces == 0 && !self.in_fill() {
            // A radius of nothing: a single pixel.
            self.paint(screen, |painter| painter.point(start));
        }
        if options.centre_given && degrees.abs() != CIRCLE {
            if let Some(end) = previous {
                self.position = self.addressing.to_user(end);
            }
        } else {
            self.pen.chain = None;
        }
    }

    /// One chord of an arc. A whole circle ends on the pixel it began on,
    /// so its first chord leaves that pixel to the last.
    fn arc_piece(&mut self, screen: &mut Screen, from: Point, to: Point, first_of_circle: bool) {
        if first_of_circle {
            self.pen.chain = Some(from);
        }
        self.segment(screen, from, to);
    }

    fn begin_curve(&mut self, closed: bool) {
        let start = self.pixel();
        self.curve = Some(Curve { closed, first: Vec::from([start]), recent: Vec::from([start]) });
    }

    /// Adds a point to the curve and draws the span that it completes:
    /// "ReGIS uses four specified positions at a time".
    fn curve_point(&mut self, screen: &mut Screen, point: Point) {
        let Some(curve) = &mut self.curve else { return };
        if curve.first.len() < CLOSING_POINTS {
            curve.first.push(point);
        }
        if curve.recent.len() == CURVE_WINDOW {
            curve.recent.remove(0);
        }
        curve.recent.push(point);
        let Ok(window) = <[Point; CURVE_WINDOW]>::try_from(curve.recent.as_slice()) else { return };

        if let Some(fill) = &mut self.fill {
            fill.vertex(window[1]);
        }
        let mut from = window[1];
        span_points(window, |to| {
            self.segment(screen, from, to);
            from = to;
        });
    }

    /// `(E)`: an open curve simply stops; a closed one is joined back to
    /// its start, where the cursor returns.
    fn end_curve(&mut self, screen: &mut Screen) {
        let Some(curve) = self.curve.clone() else { return };
        if curve.closed {
            for point in &curve.first {
                self.curve_point(screen, *point);
            }
            if let Some(start) = curve.first.first() {
                self.position = self.addressing.to_user(*start);
            }
        }
        self.curve = None;
        self.pen.chain = None;
    }
}
