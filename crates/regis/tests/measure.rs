//! The measurement ANIM-9 asks for (neoteletext-common
//! `docs/animation.md`: "Measured: moving a 20-pixel drawing, before and
//! after, in work units and time"; REQ-6530). It prints its numbers and
//! holds the work, which is the same on every machine, to a budget.
//!
//! Before: a drawing is moved by a commit of the stream and the whole
//! picture rebuilt, as a receiver did before damage repaint (REQ-6329).
//! Between: the same commit, repainted where it changed. After: the
//! receiver's animation state moves the drawing with no byte of the
//! stream, repainted where it changed, over a background kept as a
//! raster of its own (ANIM-9, ANIM-12).

mod common;

use std::fmt::Write as _;
use std::time::{Duration, Instant};

use regis::{AnimationState, ForcedLook, Picture, Screen, Terminal};

/// The side of the moving drawing, in pixels: ANIM-9's "20-pixel drawing".
const SIDE: i16 = 20;
/// The moves measured, each of [`STEP`] pixels to the right: a second of
/// animation at one move a tick.
const MOVES: i16 = 60;
/// The pixels of one move: the drawing overlaps where it was.
const STEP: i16 = 4;
/// Where the moving drawing starts.
const START: (i16, i16) = (100, 200);
/// The moving drawing's number.
const MOVING: u16 = 40;

/// The most work units one move by animation state may cost; the
/// measured cost, 8,965 on average and 10,996 at most, is printed. A
/// move pays for a visit to each of the scene's 32 drawings, at 64 work
/// units each, which is how the scene is laid out to find what changed;
/// for the moving drawing's contents replayed in the damage, 64 a byte
/// and its fill; and for the same of each scene-layer square that
/// reaches into the damage, of which the moving drawing's path crosses
/// one or two at a time. The damage is the rectangle that holds the
/// drawing before and after, 24 by 20 pixels. The budget is above the
/// most measured and a hundredth of the whole rebuild's 1,178,384: the
/// background drawing and the 28 squares away from the damage are not
/// replayed.
const MOVE_WORK_BUDGET: u64 = 12_000;

/// A scene with something to lose: a background drawing (`G0`) that
/// fills the screen and rules 40 lines across it, 30 squares in the scene
/// layer, and the moving square in the sprites layer.
fn scene() -> String {
    let mut stream = String::from("D(B1)W(I1)F(V[0,0][799,0][799,479][0,479])W(I8)");
    for line in 0..40 {
        let _ = write!(stream, "P[0,{0}]V[799,{0}]", line * 12);
    }
    stream.push_str("D(E)D(S1(G0))");
    for square in 0..30_u16 {
        let (x, y) = (20 + (square % 10) * 78, 40 + (square / 10) * 150);
        let _ = write!(stream, "D(B{0})W(I{1})F(V[0,0][59,0][59,59][0,59])D(E)", square + 2, 2 + square % 6);
        let _ = write!(stream, "D(S{}(X{x},Y{y}))", square + 2);
    }
    let last = SIDE - 1;
    let _ = write!(stream, "D(B{MOVING})W(I15)F(V[0,0][{last},0][{last},{last}][0,{last}])D(E)");
    let _ = write!(stream, "D(S{MOVING}(G2,X{},Y{}))D(C)", START.0, START.1);
    stream
}

fn built() -> (Terminal, Screen) {
    let mut terminal = common::terminal();
    let mut screen = Screen::new();
    let events = terminal.feed_regis(&mut screen, scene().as_bytes());
    assert_eq!(events, [regis::Event::Committed(1)]);
    (terminal, screen)
}

/// What one way of moving cost over all its moves.
struct Cost {
    /// The work units of each move's repaint.
    work: Vec<u64>,
    /// The time of each move: the change and the repaint.
    time: Vec<Duration>,
    /// The picture after the last move.
    last: Vec<u8>,
}

impl Cost {
    fn mean_work(&self) -> u64 {
        self.work.iter().sum::<u64>() / u64::try_from(self.work.len()).unwrap()
    }

    fn most_work(&self) -> u64 {
        self.work.iter().copied().max().unwrap()
    }

    /// The middle time of the moves: the figure least moved by a busy
    /// machine.
    fn median_time(&self) -> Duration {
        let mut times = self.time.clone();
        times.sort_unstable();
        times[times.len() / 2]
    }

    fn report(&self, name: &str) {
        println!(
            "{name}: {} work units a move on average, {} at most; {:?} a move (median of {MOVES})",
            self.mean_work(),
            self.most_work(),
            self.median_time()
        );
    }
}

/// Moves the drawing [`MOVES`] times, each by `change` and one repaint
/// into the picture `repaint` chooses.
fn measure(change: impl Fn(&mut Terminal, &mut Screen, i16), whole: bool) -> Cost {
    let (mut terminal, mut screen) = built();
    let mut kept = Picture::new();
    terminal.compose(&screen, &mut kept);
    let mut cost = Cost { work: Vec::new(), time: Vec::new(), last: Vec::new() };
    for step in 1..=MOVES {
        let x = START.0 + step * STEP;
        let started = Instant::now();
        change(&mut terminal, &mut screen, x);
        if whole {
            // A picture never painted before is rebuilt whole.
            kept = Picture::new();
        }
        terminal.compose(&screen, &mut kept);
        cost.time.push(started.elapsed());
        cost.work.push(kept.work());
    }
    cost.last = kept.canvas().pixels().to_vec();
    cost
}

fn by_commit(terminal: &mut Terminal, screen: &mut Screen, x: i16) {
    let events = terminal.feed_regis(screen, format!("D(S{MOVING}(X{x}),C)").as_bytes());
    assert!(matches!(events.as_slice(), [regis::Event::Committed(_)]), "{events:?}");
}

fn by_animation_state(terminal: &mut Terminal, _: &mut Screen, x: i16) {
    let state = AnimationState {
        shown: None,
        position: Some((x, START.1)),
        layer: None,
        order: None,
        frame: 0,
        look: ForcedLook::None,
    };
    assert!(terminal.set_animation(MOVING, state));
}

#[test]
fn moving_a_20_pixel_drawing_costs_its_own_pixels_and_not_the_scenes() {
    let before = measure(by_commit, true);
    let between = measure(by_commit, false);
    let after = measure(by_animation_state, false);
    println!("moving a {SIDE}-pixel drawing {STEP} pixels over a background drawing and 30 others:");
    before.report("  before, a commit and the whole picture rebuilt");
    between.report("  a commit, repainted where it changed");
    after.report("  after, animation state, repainted where it changed");
    println!(
        "  the animation state's move costs 1/{} of the whole rebuild in work units and 1/{} in time",
        before.mean_work() / after.mean_work().max(1),
        before.median_time().as_nanos() / after.median_time().as_nanos().max(1)
    );
    // The three ways make the same picture.
    assert!(before.last == between.last && before.last == after.last, "the ways of moving disagree");
    // The work is the same on every machine, so it is held exactly.
    assert!(after.most_work() <= MOVE_WORK_BUDGET, "a move cost {} work units", after.most_work());
    assert!(after.most_work() <= between.most_work(), "animation state costs more than a commit");
    // The whole rebuild pays for the background and every drawing: a
    // hundred times a move at the least.
    assert!(before.mean_work() > after.most_work() * 100, "{} against {}", before.mean_work(), after.most_work());
    // Time depends on the machine and its load, so only the order is
    // held: a move is quicker than a whole rebuild.
    assert!(after.median_time() < before.median_time(), "{:?} against {:?}", after.median_time(), before.median_time());
}
