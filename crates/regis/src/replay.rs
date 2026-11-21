//! Replaying a stream at a pace: its bytes are held and given to the
//! terminal a few each tick, as a serial line of a stated speed would
//! have delivered them, so that a picture is seen being drawn (DEC-7031
//! item 2; REQ-7024).
//!
//! The pacing is here, beside the terminal, and not in the program that
//! shows the picture, so that the render tool and a receiver show the
//! same thing at the same tick. It knows nothing of line speeds: its
//! caller turns a speed into bytes per ticks.

use alloc::vec::Vec;

use crate::event::Event;
use crate::limits::{MAX_EVENTS, MAX_REPLAY_HELD_BYTES};
use crate::surface::Screen;
use crate::terminal::Terminal;

/// The pace of a replay: `bytes` bytes are fed in every `ticks` ticks of
/// the presentation clock, 60 to the second. A line of 1,200 bits a
/// second at ten bits a byte is 120 bytes a second: `Pace { bytes: 1200,
/// ticks: 600 }`, or `Pace { bytes: 2, ticks: 1 }`. Either part given as
/// 0 is taken as 1, so that no pace divides by nothing or holds its bytes
/// for ever.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Pace {
    /// The bytes fed in each period.
    pub bytes: u32,
    /// The ticks of a period.
    pub ticks: u32,
}

impl Pace {
    /// The bytes a run has been owed after `run_ticks` ticks: `run_ticks ×
    /// bytes ÷ ticks`, rounded down from the total of ticks so that
    /// nothing is lost to rounding from one tick to the next.
    fn owed_after(self, run_ticks: u64) -> u64 {
        let total = run_ticks.saturating_mul(u64::from(self.bytes.max(1)));
        total.checked_div(u64::from(self.ticks.max(1))).unwrap_or(u64::MAX)
    }

    /// The first tick of a run by which more than `fed` bytes are owed:
    /// `(fed + 1) × ticks ÷ bytes`, rounded up.
    fn tick_owing_more_than(self, fed: u64) -> u64 {
        let total = fed.saturating_add(1).saturating_mul(u64::from(self.ticks.max(1)));
        total.div_ceil(u64::from(self.bytes.max(1)))
    }
}

/// Holds a stream's bytes and feeds them to a [`Terminal`] at a
/// [`Pace`] (DEC-7031 item 2; REQ-7024). With no pace, as at first, every
/// byte pushed is fed at once, exactly as [`Terminal::feed_regis`] or
/// [`Terminal::feed`] would feed it.
///
/// A run begins when bytes are pushed while none are held. After `n`
/// ticks of a run, `n × bytes ÷ ticks` bytes of it, rounded down, have
/// been fed, or all there were. When the last held byte is fed the run
/// is over and what the pace had left unspent is forgotten: bytes pushed
/// later begin a new run and wait their full time, as bytes arriving on
/// an idle line do.
///
/// It holds at most [`MAX_REPLAY_HELD_BYTES`]: a push that would hold
/// more ends the pacing, and everything held and pushed is fed at once.
/// The memory it holds is its own structure and one allocation of the
/// bytes held.
///
/// The picture at the end is the picture of the same bytes fed at once:
/// the terminal takes its bytes in pieces of any size. The caller still
/// calls [`Terminal::advance_ticks`] for the paint rates, and
/// [`Terminal::finish`] when the stream has ended and the replay is
/// [idle](Replay::idle).
#[derive(Debug, Default)]
pub struct Replay {
    /// The bytes pushed and not yet fed, after the first `fed` of them.
    held: Vec<u8>,
    /// How many bytes at the front of `held` have been fed.
    fed: usize,
    pace: Option<Pace>,
    /// Whether the bytes are a terminal data stream, fed by
    /// [`Terminal::feed`], and not bare ReGIS commands.
    framed: bool,
    /// The ticks of the run so far.
    run_ticks: u64,
    /// The bytes of the run fed so far.
    run_fed: u64,
}

/// Feeds bytes to the terminal, as the replay was made to.
fn feed(framed: bool, terminal: &mut Terminal, screen: &mut Screen, bytes: &[u8]) -> Vec<Event> {
    if framed { terminal.feed(screen, bytes) } else { terminal.feed_regis(screen, bytes) }
}

impl Replay {
    /// A replay of bare ReGIS commands, which it feeds as
    /// [`Terminal::feed_regis`] does; it holds nothing and has no pace.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// A replay of a terminal data stream, which it feeds as
    /// [`Terminal::feed`] does: ReGIS is taken from device control
    /// strings and everything else ignored, though every byte takes its
    /// time on the line.
    #[must_use]
    pub fn framed() -> Self {
        Self { framed: true, ..Self::default() }
    }

    /// Sets the pace, or none. A change of pace begins a new run from the
    /// bytes still held. With none, bytes still held are fed by the next
    /// [`Replay::push`] or [`Replay::advance`], which has the terminal
    /// and the screen to feed them to.
    pub fn set_pace(&mut self, pace: Option<Pace>) {
        if pace != self.pace {
            self.pace = pace;
            self.run_ticks = 0;
            self.run_fed = 0;
        }
    }

    /// Takes bytes of the stream. With a pace they are held, behind any
    /// held already, and nothing is fed: a run that begins here has had
    /// no tick yet. With no pace, or when the bytes held would pass
    /// [`MAX_REPLAY_HELD_BYTES`], everything held and then these bytes
    /// are fed at once; in the second case the pace is also set to none,
    /// so that a stream too long to hold is not paced in part.
    ///
    /// The events are the terminal's, for the bytes fed in this call.
    #[must_use = "the events hold replies and limit notices the caller must act on"]
    pub fn push(&mut self, terminal: &mut Terminal, screen: &mut Screen, bytes: &[u8]) -> Vec<Event> {
        let waiting = self.held_bytes();
        if self.pace.is_none() || waiting.saturating_add(bytes.len()) > MAX_REPLAY_HELD_BYTES {
            self.pace = None;
            let mut events = self.release(terminal, screen);
            events.extend(feed(self.framed, terminal, screen, bytes));
            // One call returns no more events than one feeding call does.
            events.truncate(MAX_EVENTS);
            return events;
        }
        if waiting == 0 {
            self.forget();
        } else {
            // What was fed makes room for what arrives, so the
            // allocation never holds more than the cap.
            self.held.drain(..self.fed.min(self.held.len()));
            self.fed = 0;
        }
        self.held.reserve_exact(bytes.len());
        self.held.extend_from_slice(bytes);
        Vec::new()
    }

    /// Says that `ticks` of the presentation clock have passed, and feeds
    /// the bytes the run is owed by then. With no pace it feeds
    /// everything still held. The events are the terminal's, for the
    /// bytes fed in this call.
    #[must_use = "the events hold replies and limit notices the caller must act on"]
    pub fn advance(&mut self, terminal: &mut Terminal, screen: &mut Screen, ticks: u32) -> Vec<Event> {
        let Some(pace) = self.pace else { return self.release(terminal, screen) };
        if self.idle() {
            return Vec::new();
        }
        self.run_ticks = self.run_ticks.saturating_add(u64::from(ticks));
        let owed = pace.owed_after(self.run_ticks).saturating_sub(self.run_fed);
        let count = usize::try_from(owed).unwrap_or(usize::MAX).min(self.held_bytes());
        let end = self.fed.saturating_add(count);
        let events = match self.held.get(self.fed..end) {
            Some(piece) if !piece.is_empty() => feed(self.framed, terminal, screen, piece),
            _ => Vec::new(),
        };
        self.fed = end;
        self.run_fed = self.run_fed.saturating_add(u64::try_from(count).unwrap_or(u64::MAX));
        if self.idle() {
            self.forget();
        }
        events
    }

    /// Feeds everything held, at once.
    fn release(&mut self, terminal: &mut Terminal, screen: &mut Screen) -> Vec<Event> {
        let events = match self.held.get(self.fed..) {
            Some(rest) if !rest.is_empty() => feed(self.framed, terminal, screen, rest),
            _ => Vec::new(),
        };
        self.forget();
        events
    }

    /// Ends the run: nothing is held, its memory is given back, and what
    /// the pace had left unspent is forgotten.
    fn forget(&mut self) {
        self.held = Vec::new();
        self.fed = 0;
        self.run_ticks = 0;
        self.run_fed = 0;
    }

    /// The bytes held, pushed and not yet fed: at most
    /// [`MAX_REPLAY_HELD_BYTES`].
    #[must_use]
    pub fn held_bytes(&self) -> usize {
        self.held.len().saturating_sub(self.fed)
    }

    /// Whether nothing is held: every byte pushed has been fed. A
    /// receiver takes a picture's replay as finished when the stream has
    /// ended and this is true.
    #[must_use]
    pub fn idle(&self) -> bool {
        self.held_bytes() == 0
    }

    /// The ticks until [`Replay::advance`] next has a byte to feed, at
    /// least 1 under a pace, so that a caller with a clock sleeps until
    /// then; 0 when bytes are held with no pace; `None` when nothing is
    /// held.
    #[must_use]
    pub fn ticks_to_next(&self) -> Option<u32> {
        if self.idle() {
            return None;
        }
        let Some(pace) = self.pace else { return Some(0) };
        let due = pace.tick_owing_more_than(self.run_fed).saturating_sub(self.run_ticks);
        Some(u32::try_from(due).unwrap_or(u32::MAX))
    }
}
