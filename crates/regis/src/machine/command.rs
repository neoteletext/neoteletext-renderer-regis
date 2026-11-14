//! Dispatch: which command or option each token belongs to.

use super::Machine;
use super::curve::ArcOptions;
use super::frames::{Command, Context};
use super::state::Fault;
use super::write::ShadeOption;
use crate::lexer::{DigitMode, Token};
use crate::surface::Screen;

/// Error codes of the error report (manual chapter 10, table 10-1).
pub(super) mod errors {
    pub(crate) const IGNORED_CHARACTER: u8 = 1;
    pub(crate) const EXTRA_COORDINATES: u8 = 3;
    pub(crate) const ALPHABET_RANGE: u8 = 4;
    pub(crate) const STACK_OVERFLOW: u8 = 7;
    pub(crate) const STACK_UNDERFLOW: u8 = 8;
    pub(crate) const TEXT_SIZE: u8 = 9;
}

impl Machine {
    /// What a digit means where the interpreter now stands.
    pub(crate) fn digit_mode(&self) -> DigitMode {
        if !self.syntax.at_command_level() {
            DigitMode::Number
        } else if self.syntax.command == Some(Command::Load) && self.characters.cell.is_some() {
            DigitMode::Hex
        } else {
            DigitMode::Bare
        }
    }

    pub(super) const fn in_fill(&self) -> bool {
        self.fill.is_some()
    }

    /// Interprets one token.
    pub(crate) fn token(&mut self, screen: &mut Screen, token: Token) {
        match token {
            Token::Resync => self.resynchronise(screen),
            Token::Open if self.syntax.skipping() => self.syntax.open(Context::Skip),
            Token::Close if self.syntax.skipping() => {
                self.syntax.close();
            }
            _ if self.syntax.skipping() => {}
            token if self.syntax.at_command_level() => self.argument(screen, token),
            Token::Close => self.close(),
            token => self.option(screen, &token),
        }
    }

    /// `;`: closes whatever was open and waits for a key letter; also
    /// clears the error report (manual chapter 1, "Resynchronization").
    pub(crate) fn resynchronise(&mut self, screen: &mut Screen) {
        self.finish_cell();
        self.syntax.reset();
        if self.in_fill() {
            self.end_fill(screen);
        }
        self.end_command();
        // Options that arrive in pieces do not survive a resynchronisation.
        self.text.tilted = false;
        self.corner = None;
        self.map_location = None;
        self.shade_option = ShadeOption::default();
        self.fault = Fault::default();
    }

    /// Ends the effects that last only for one command.
    fn end_command(&mut self) {
        self.pen.begin_command();
        self.end_figure();
    }

    /// Ends a curve in progress and the circle options, which last "until
    /// you enter a new command" (manual chapter 6).
    fn end_figure(&mut self) {
        self.curve = None;
        self.arc = ArcOptions::default();
    }

    fn begin_command(&mut self, command: Command) {
        self.finish_cell();
        if self.in_fill() {
            // Inside a fill only the path matters; write controls set there
            // last to its end (manual chapter 11, "Temporary Write Controls").
            self.end_figure();
        } else {
            self.end_command();
        }
        self.syntax.command = Some(command);
        if command == Command::Text {
            self.text.line_start = self.position;
        }
    }

    /// A token at command level.
    fn argument(&mut self, screen: &mut Screen, token: Token) {
        match (token, self.syntax.command) {
            (Token::Letter(letter), _) => self.key_letter(letter),
            (Token::Open, command) => self.open_command(command),
            (Token::Close, _) if self.in_fill() => {
                self.syntax.close();
                self.end_fill(screen);
                self.syntax.command = Some(Command::Fill);
            }
            (Token::Extent(extent), Some(command)) => {
                if extent.extra {
                    self.note_fault(errors::EXTRA_COORDINATES, 0);
                }
                self.extent_argument(screen, command, extent);
            }
            (Token::Digit(digit), Some(command)) => self.digit_argument(screen, command, digit),
            (Token::Text(text), Some(Command::Text)) => self.draw_string(screen, &text),
            (Token::Text(text), Some(Command::Load)) => self.begin_cell(&text),
            (Token::Hex(value), Some(Command::Load)) => self.cell_digit(value),
            (Token::Comma, Some(Command::Load)) => self.cell_separator(),
            (Token::Symbol(byte), _) => self.note_fault(errors::IGNORED_CHARACTER, byte),
            (Token::Close, _) => self.note_fault(errors::IGNORED_CHARACTER, b')'),
            _ => {}
        }
    }

    fn key_letter(&mut self, letter: u8) {
        match Command::of(letter) {
            Some(command) if !self.in_fill() || command.allowed_in_fill() => self.begin_command(command),
            _ => self.note_fault(errors::IGNORED_CHARACTER, letter),
        }
    }

    /// `(` at command level: the command's options, or a fill's body.
    fn open_command(&mut self, command: Option<Command>) {
        let context = match command {
            Some(Command::Fill) if !self.in_fill() => {
                self.begin_fill();
                self.syntax.command = None;
                Context::Fill
            }
            Some(Command::Write) if self.in_fill() => {
                // "Only the last W option in a polygon fill command affects
                // the graphic image" (manual chapter 11).
                self.pen.temporary = None;
                Context::TemporaryWrite
            }
            Some(command) => Context::Options(command),
            None => Context::Skip,
        };
        self.syntax.open(context);
    }

    /// `)` inside options.
    fn close(&mut self) {
        let Some(frame) = self.syntax.close() else { return };
        match frame.context {
            Context::Colour(target) => self.finish_colour(target),
            Context::Options(Command::Report) => self.finish_report(frame.option),
            Context::Options(Command::Text) => self.text.tilted = false,
            _ => {}
        }
    }

    /// A token inside options.
    fn option(&mut self, screen: &mut Screen, token: &Token) {
        let Some(context) = self.syntax.context() else { return };
        match context {
            Context::Options(Command::Screen) => self.screen_option(screen, token),
            Context::Options(Command::Write) => self.write_option(token, false),
            Context::TemporaryWrite => self.write_option(token, true),
            Context::Options(Command::Position) => self.position_option(screen, token, false),
            Context::Options(Command::Vector) => self.position_option(screen, token, true),
            Context::Options(Command::Curve) => self.curve_option(screen, token),
            Context::Options(Command::Text) => self.text_option(token),
            Context::Options(Command::Load) => self.load_option(token),
            Context::Options(Command::Report) => self.report_option(token),
            Context::Colour(_) => self.colour_token(token),
            Context::PatternMultiplier { temporary } => self.pattern_multiplier(token, temporary),
            Context::ShadeAxis => self.shade_axis(token),
            Context::ReportMacrograph => self.report_macrograph(token),
            Context::ReportPosition => self.report_position(token),
            Context::Designators | Context::Skip | Context::Options(Command::Fill) | Context::Fill => {
                if *token == Token::Open {
                    self.syntax.open(Context::Skip);
                }
            }
        }
    }

    /// Begins write controls that last only for this command.
    pub(super) fn open_temporary_write(&mut self) {
        if !self.in_fill() {
            // "Another temporary write control option" ends the one before.
            self.pen.temporary = None;
        }
        self.syntax.open(Context::TemporaryWrite);
    }
}
