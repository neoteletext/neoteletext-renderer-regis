//! Playing a picture's animation into a sequence of images
//! (neoteletext-common `docs/animation.md`, ANIM-18): the timed
//! operations of a script, which are ANIM-6's, and the frame cycles of
//! ANIM-14, which the renderer leaves to the program that has the clock.

use std::collections::BTreeMap;

use regis::limits::MAX_CYCLING_OBJECTS;
use regis::{AnimationState, ForcedLook, Terminal};

/// The most ticks one run plays (ANIM-18).
pub(crate) const MAX_TICKS: u32 = 36_000;
/// The longest period of a cycle, in ticks (ANIM-6).
const MAX_PERIOD: u32 = 3600;
/// The highest frame number (ANIM-6).
const LAST_FRAME: u8 = 15;
/// The highest layer (ANIM-6).
const TOP_LAYER: u8 = 3;
/// The highest forced look: the alternative look (ANIM-6).
const LAST_LOOK: u8 = 3;

/// One operation on a numbered drawing's animation state (ANIM-6).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Operation {
    Move {
        x: i16,
        y: i16,
        relative: bool,
    },
    Show,
    Hide,
    Look(ForcedLook),
    /// The layer, or `None` to keep it; the order.
    Order(Option<u8>, i8),
    Frame(u8),
    Cycle {
        first: u8,
        last: u8,
        period: u32,
        once: bool,
    },
    StopCycle,
    Reset,
}

/// An operation, the drawing it acts on and the tick it is due.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Timed {
    pub(crate) tick: u32,
    pub(crate) drawing: u16,
    pub(crate) operation: Operation,
}

/// A word of a script line as a number within a range.
fn number<T: TryFrom<i64>>(word: Option<&str>, what: &str, low: i64, high: i64) -> Result<T, String> {
    let word = word.ok_or_else(|| format!("{what} is missing"))?;
    let value: i64 = word.parse().map_err(|_| format!("{what}: {word} is not a number"))?;
    if !(low..=high).contains(&value) {
        return Err(format!("{what}: {value} is not between {low} and {high}"));
    }
    T::try_from(value).map_err(|_| format!("{what}: {value} is out of range"))
}

/// A word that may follow an operation's numbers, or nothing.
fn flag<'a>(words: &mut impl Iterator<Item = &'a str>, name: &str) -> Result<bool, String> {
    match words.next() {
        None => Ok(false),
        Some(word) if word == name => Ok(true),
        Some(word) => Err(format!("{word} where {name} or the end of the line was expected")),
    }
}

/// One line: `<tick> <operation> <drawing> ...`.
fn line(text: &str) -> Result<Timed, String> {
    let mut words = text.split_ascii_whitespace();
    let tick = number(words.next(), "the tick", 0, i64::from(MAX_TICKS))?;
    let name = words.next().ok_or("the operation is missing")?;
    if name == "record" {
        return Err("record is for teletext pages, which neottx plays; regis-render takes operations only".into());
    }
    let drawing = number(words.next(), "the drawing number", 1, i64::from(u16::MAX))?;
    let coordinate = |word| number(word, "a coordinate", i64::from(i16::MIN), i64::from(i16::MAX));
    let frame = |word, what| number(word, what, 0, i64::from(LAST_FRAME));
    let operation = match name {
        "move" => {
            let (x, y) = (coordinate(words.next())?, coordinate(words.next())?);
            Operation::Move { x, y, relative: flag(&mut words, "relative")? }
        }
        "show" => Operation::Show,
        "hide" => Operation::Hide,
        "look" => {
            let code = number(words.next(), "the look", 0, i64::from(LAST_LOOK))?;
            Operation::Look(ForcedLook::from_code(code).ok_or("the look is not 0 to 3")?)
        }
        "order" => {
            let layer = match words.next() {
                Some("keep") => None,
                word => Some(number(word, "the layer", 0, i64::from(TOP_LAYER))?),
            };
            Operation::Order(layer, number(words.next(), "the order", i64::from(i8::MIN), i64::from(i8::MAX))?)
        }
        "frame" => Operation::Frame(frame(words.next(), "the frame")?),
        "cycle" => {
            let (first, last) = (frame(words.next(), "the first frame")?, frame(words.next(), "the last frame")?);
            if first > last {
                return Err(format!("a cycle's first frame, {first}, is after its last, {last}"));
            }
            let period = number(words.next(), "the period", 1, i64::from(MAX_PERIOD))?;
            Operation::Cycle { first, last, period, once: flag(&mut words, "once")? }
        }
        "stop_cycle" => Operation::StopCycle,
        "reset" => Operation::Reset,
        other => return Err(format!("{other} is not an operation")),
    };
    match words.next() {
        Some(extra) => Err(format!("{extra} is more than the operation takes")),
        None => Ok(Timed { tick, drawing, operation }),
    }
}

/// Reads a script: one timed operation a line, in ascending tick. Blank
/// lines and lines that begin with `#` say nothing.
pub(crate) fn parse(text: &str) -> Result<Vec<Timed>, String> {
    let mut script: Vec<Timed> = Vec::new();
    for (index, text) in text.lines().enumerate() {
        if text.trim().is_empty() || text.trim_start().starts_with('#') {
            continue;
        }
        let timed = line(text).map_err(|error| format!("line {}: {error}", index.saturating_add(1)))?;
        if script.last().is_some_and(|last| last.tick > timed.tick) {
            return Err(format!("line {}: tick {} comes after a later tick", index.saturating_add(1), timed.tick));
        }
        script.push(timed);
    }
    Ok(script)
}

/// A drawing changing its frame by itself (ANIM-14).
#[derive(Clone, Copy, Debug)]
struct Cycle {
    first: u8,
    last: u8,
    period: u32,
    once: bool,
    /// The tick it started: the tick after its operation ran.
    start: u32,
}

impl Cycle {
    /// The frame at a tick, and whether the cycle has then ended.
    fn frame(self, tick: u32) -> (u8, bool) {
        let Some(steps) = tick.checked_sub(self.start).map(|ticks| ticks.checked_div(self.period).unwrap_or(0)) else {
            return (self.first, false);
        };
        let count = u32::from(self.last.saturating_sub(self.first)).saturating_add(1);
        let step = if self.once { steps.min(count.saturating_sub(1)) } else { steps.checked_rem(count).unwrap_or(0) };
        let frame = self.first.saturating_add(u8::try_from(step).unwrap_or(0));
        (frame, self.once && frame == self.last)
    }
}

/// The animation state of a run: what a receiver's decoder keeps.
#[derive(Default)]
pub(crate) struct Player {
    states: BTreeMap<u16, AnimationState>,
    cycles: BTreeMap<u16, Cycle>,
    /// Operations that did nothing: their drawing does not exist, or a
    /// limit was reached.
    pub(crate) ignored: u32,
}

impl Player {
    /// Carries out one operation at a tick.
    pub(crate) fn apply(&mut self, terminal: &mut Terminal, tick: u32, timed: Timed) {
        let drawing = timed.drawing;
        // "An operation on a target that does not exist does nothing and
        // is counted" (ANIM-6).
        let Some(in_force) = terminal.drawing_position(drawing) else {
            self.ignored = self.ignored.saturating_add(1);
            return;
        };
        let mut state = self.states.get(&drawing).copied().unwrap_or_default();
        match timed.operation {
            Operation::Move { x, y, relative: false } => state.position = Some((x, y)),
            Operation::Move { x, y, relative: true } => {
                state.position = Some((in_force.0.saturating_add(x), in_force.1.saturating_add(y)));
            }
            Operation::Show => state.shown = Some(true),
            Operation::Hide => state.shown = Some(false),
            Operation::Look(look) => state.look = look,
            Operation::Order(layer, order) => {
                state.layer = layer.or(state.layer);
                state.order = Some(order);
            }
            Operation::Frame(frame) => {
                state.frame = frame;
                self.cycles.remove(&drawing);
            }
            Operation::Cycle { first, last, period, once } => {
                // A 17th cycle does nothing and is counted (ANIM-14).
                if self.cycles.len() >= MAX_CYCLING_OBJECTS && !self.cycles.contains_key(&drawing) {
                    self.ignored = self.ignored.saturating_add(1);
                    return;
                }
                self.cycles.insert(drawing, Cycle { first, last, period, once, start: tick.saturating_add(1) });
            }
            Operation::StopCycle => {
                self.cycles.remove(&drawing);
            }
            Operation::Reset => {
                self.cycles.remove(&drawing);
                self.states.remove(&drawing);
                terminal.clear_animation(drawing);
                return;
            }
        }
        self.set(terminal, drawing, state);
    }

    fn set(&mut self, terminal: &mut Terminal, drawing: u16, state: AnimationState) {
        self.states.insert(drawing, state);
        if !terminal.set_animation(drawing, state) {
            self.ignored = self.ignored.saturating_add(1);
        }
    }

    /// Gives every cycling drawing its frame for a tick.
    pub(crate) fn tick(&mut self, terminal: &mut Terminal, tick: u32) {
        let cycles: Vec<(u16, Cycle)> = self.cycles.iter().map(|(drawing, cycle)| (*drawing, *cycle)).collect();
        for (drawing, cycle) in cycles {
            if tick < cycle.start {
                continue;
            }
            let (frame, ended) = cycle.frame(tick);
            if ended {
                self.cycles.remove(&drawing);
            }
            let mut state = self.states.get(&drawing).copied().unwrap_or_default();
            if state.frame != frame {
                state.frame = frame;
                self.set(terminal, drawing, state);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{Cycle, Operation, Timed, parse};
    use regis::ForcedLook;

    #[test]
    fn every_operation_is_read() {
        let script = "# a comment\n0 move 5 10 -20\n0 move 5 1 2 relative\n\n1 show 5\n1 hide 5\n2 look 5 3\n\
                      3 order 5 2 -7\n3 order 5 keep 9\n4 frame 5 15\n5 cycle 5 0 3 6\n5 cycle 5 1 2 3600 once\n\
                      6 stop_cycle 5\n7 reset 65535\n";
        let operations: Vec<Operation> = parse(script).unwrap().iter().map(|timed| timed.operation).collect();
        assert_eq!(
            operations,
            [
                Operation::Move { x: 10, y: -20, relative: false },
                Operation::Move { x: 1, y: 2, relative: true },
                Operation::Show,
                Operation::Hide,
                Operation::Look(ForcedLook::Alternative),
                Operation::Order(Some(2), -7),
                Operation::Order(None, 9),
                Operation::Frame(15),
                Operation::Cycle { first: 0, last: 3, period: 6, once: false },
                Operation::Cycle { first: 1, last: 2, period: 3600, once: true },
                Operation::StopCycle,
                Operation::Reset,
            ]
        );
        assert_eq!(parse("7 reset 65535").unwrap(), [Timed { tick: 7, drawing: 65535, operation: Operation::Reset }]);
    }

    #[test]
    fn mistakes_are_refused_with_their_line() {
        for bad in [
            "x show 1",
            "0 show 0",
            "0 show 65536",
            "0 jump 1",
            "0 look 1 4",
            "0 order 1 4 0",
            "0 order 1 keep 128",
            "0 frame 1 16",
            "0 cycle 1 3 2 6",
            "0 cycle 1 0 3 0",
            "0 cycle 1 0 3 3601",
            "0 cycle 1 0 3 6 twice",
            "0 move 1 2",
            "0 move 1 2 3 4",
            "0 show 1 2",
            "36001 show 1",
            "0 record ANIMATE 00",
        ] {
            assert!(parse(bad).unwrap_err().starts_with("line 1: "), "{bad}");
        }
        assert!(parse("5 show 1\n4 show 1").unwrap_err().starts_with("line 2: "));
    }

    #[test]
    fn a_cycle_follows_both_formulas_at_their_edges() {
        // ANIM-14: first + ((t - s) / p) mod c, and with once
        // min(first + (t - s) / p, last), ending when last is reached.
        let cycle = Cycle { first: 2, last: 4, period: 3, once: false, start: 10 };
        let frames: Vec<u8> = [9, 10, 12, 13, 16, 18, 19, 22].iter().map(|tick| cycle.frame(*tick).0).collect();
        assert_eq!(frames, [2, 2, 2, 3, 4, 4, 2, 3]);
        assert!(!cycle.frame(16).1);
        let once = Cycle { once: true, ..cycle };
        let frames: Vec<(u8, bool)> = [10, 13, 15, 16, 400].iter().map(|tick| once.frame(*tick)).collect();
        assert_eq!(frames, [(2, false), (3, false), (3, false), (4, true), (4, true)]);
    }
}
