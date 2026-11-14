//! The rewriting itself: following the position and the write controls
//! through a command string and writing each command as short as it goes.

use crate::Level;
use crate::command::{Command, Item};
use crate::point::{Position, Target};
use crate::token::Token;
use crate::write::{self, Controls, Setting};

/// A command after rewriting, or one left as it was.
enum Piece {
    Write(Vec<Setting>),
    /// Moves, and the position they start from.
    Moves {
        from: Position,
        targets: Vec<Target>,
    },
    /// Lines, and the position they start from.
    Lines {
        from: Position,
        targets: Vec<Target>,
    },
    Kept(Item),
}

/// The targets of a command whose arguments are all plain brackets.
fn targets(command: &Command) -> Option<Vec<Target>> {
    command
        .arguments
        .iter()
        .map(|token| match token {
            Token::Bracket(inside) => Target::parse(inside),
            _ => None,
        })
        .collect()
}

fn follow(from: Position, targets: &[Target]) -> Position {
    targets.iter().fold(from, |at, target| target.resolve(at))
}

#[derive(Default)]
struct Rewriter {
    pieces: Vec<Piece>,
    position: Position,
    controls: Controls,
    /// Write pieces since anything was last drawn: a setting in one of
    /// them is pointless if a later one replaces it first.
    undrawn: Vec<usize>,
}

impl Rewriter {
    fn forget_everything(&mut self) {
        self.position = Position::UNKNOWN;
        self.controls = Controls::default();
        self.undrawn.clear();
    }

    fn write(&mut self, settings: Vec<Setting>) {
        for setting in settings {
            if !self.controls.apply(&setting) {
                continue;
            }
            for index in &self.undrawn {
                if let Some(Piece::Write(earlier)) = self.pieces.get_mut(*index) {
                    earlier.retain(|old| !write::overrides(&setting, old));
                }
            }
            if let Some(Piece::Write(last)) = self.pieces.last_mut() {
                last.push(setting);
            } else {
                self.undrawn.push(self.pieces.len());
                self.pieces.push(Piece::Write(vec![setting]));
            }
        }
    }

    fn command(&mut self, command: Command) {
        match (command.letter, targets(&command)) {
            (b'W', _) => {
                if let Some(settings) = write::parse(&command.arguments) {
                    self.write(settings);
                } else {
                    self.controls = Controls::default();
                    self.undrawn.clear();
                    self.pieces.push(Piece::Kept(Item::Command(command)));
                }
            }
            (b'P', Some(targets)) if !targets.is_empty() => {
                let to = follow(self.position, &targets);
                if let Some(Piece::Moves { targets: earlier, .. }) = self.pieces.last_mut() {
                    earlier.extend(targets);
                } else {
                    self.pieces.push(Piece::Moves { from: self.position, targets });
                }
                self.position = to;
            }
            (b'V', Some(targets)) if !targets.is_empty() => {
                let to = follow(self.position, &targets);
                self.pieces.push(Piece::Lines { from: self.position, targets });
                self.position = to;
                self.undrawn.clear();
            }
            _ => {
                // Any other command may move the position; none of them
                // changes the write controls beyond its own end.
                self.position = Position::UNKNOWN;
                self.undrawn.clear();
                self.pieces.push(Piece::Kept(Item::Command(command)));
            }
        }
    }

    fn item(&mut self, item: Item) {
        match item {
            Item::Command(command) => self.command(command),
            Item::Loose(Token::Semicolon) => self.pieces.push(Piece::Kept(item)),
            Item::Loose(_) => {
                // A macrograph or a load command can hold anything.
                self.forget_everything();
                self.pieces.push(Piece::Kept(item));
            }
        }
    }
}

fn brackets(out: &mut Vec<u8>, mut at: Position, targets: &[Target]) {
    for target in targets {
        let to = target.resolve(at);
        out.push(b'[');
        out.extend_from_slice(&target.shortest(at, to));
        out.push(b']');
        at = to;
    }
}

/// Moves written as one command. Only the last matters when it names both
/// coordinates outright, and none matter if they lead nowhere.
fn moves(out: &mut Vec<u8>, from: Position, targets: &[Target], level: Level) -> bool {
    let to = follow(from, targets);
    let known = |position: Position| position.x.is_some() && position.y.is_some();
    if known(from) && known(to) {
        if from == to && level == Level::Joined {
            return false;
        }
        let whole = Target { x: crate::point::Part::Same, y: crate::point::Part::Same };
        out.push(b'P');
        out.push(b'[');
        out.extend_from_slice(&whole.shortest(from, to));
        out.push(b']');
        return true;
    }
    let absolute = |target: &Target| {
        matches!(target.x, crate::point::Part::Absolute(_)) && matches!(target.y, crate::point::Part::Absolute(_))
    };
    let start = targets.iter().rposition(absolute).unwrap_or(0);
    out.push(b'P');
    brackets(out, follow(from, targets.get(..start).unwrap_or_default()), targets.get(start..).unwrap_or_default());
    true
}

/// Rewrites a list of items and writes the result.
pub(crate) fn rewrite(items: Vec<Item>, level: Level, out: &mut Vec<u8>) {
    let mut rewriter = Rewriter::default();
    for item in items {
        rewriter.item(item);
    }
    // Lines that follow lines directly need no second command letter.
    let mut after_lines = false;
    for piece in &rewriter.pieces {
        match piece {
            Piece::Write(settings) if settings.is_empty() => {}
            Piece::Write(settings) => {
                out.extend_from_slice(b"W(");
                for setting in settings {
                    setting.write(out);
                }
                out.push(b')');
                after_lines = false;
            }
            Piece::Moves { from, targets } => {
                if moves(out, *from, targets, level) {
                    after_lines = false;
                }
            }
            Piece::Lines { from, targets } => {
                if !after_lines {
                    out.push(b'V');
                }
                brackets(out, *from, targets);
                after_lines = true;
            }
            Piece::Kept(item) => {
                item.write(out);
                after_lines = false;
            }
        }
    }
}
