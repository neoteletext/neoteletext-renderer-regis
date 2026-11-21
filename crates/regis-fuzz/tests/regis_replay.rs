//! The fuzz target `regis_replay` (REQ-7024, REQ-7025, REQ-7026; DEC-7031
//! item 2): any sequence of pushes to a replay, ticks, changes of pace,
//! watched drawings and default paint rates. Nothing panics; the reports
//! of finished paintings are of watched drawings, in ascending order,
//! each once a call; and once every byte is fed and every painting
//! finished, the screen, the composed picture and the events are those
//! of the same bytes fed at once.
//!
//! [`target`] takes one input's bytes. It is run first on every input
//! kept in `corpus/regis_replay/`, where each input that ever found a
//! fault is kept, and then on inputs a property-testing library makes
//! (DEC-6603), new ones on every run. An input that fails is reduced to
//! the least that still fails and written into the corpus folder, to be
//! committed with the fix.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use proptest::collection::vec;
use proptest::prelude::any;
use proptest::test_runner::{Config, TestError, TestRunner};
use regis::limits::MAX_WATCHED_PAINTINGS;
use regis::{Font, Pace, Picture, Replay, Screen, Terminal};

/// A terminal with the shipped font.
fn terminal() -> Terminal {
    let mut terminal = Terminal::new();
    terminal.set_font(Font::parse(regis_fonts::VT340).unwrap());
    terminal
}

/// The pieces pushed bytes are made of, so that inputs reach numbered
/// drawings, their paint rates, parents and visibility, commits and
/// device control strings, and not only refusals.
const PIECES: &[&[u8]] = &[
    b"D(B1)",
    b"D(B2)",
    b"D(B3)",
    b"D(E)",
    b"D(C)",
    b"D(S1(T6),C)",
    b"D(S2(P1),C)",
    b"D(S3(P2,T0),C)",
    b"D(S2(V0),C)",
    b"D(S2(V1),C)",
    b"D(S3(T40),C)",
    b"D(K1,C)",
    b"D(S1(R3),C)",
    b"D(S2(F(3,1)),C)",
    b"D(S4(P9),C)",
    b"W(I3)",
    b"P[0,0]V[599,0]",
    b"P[10,10]V[+50,+90]",
    b"F(V[0,0][50,0][25,45])",
    b"C[+30]",
    b"R(P)",
    b"S(E)",
    b"\x1bPp",
    b"\x1b\\",
    b"x",
    b"(",
    b")",
    b",",
    b"T",
    b"9",
];

/// The drawing numbers watched: few, so that they meet the drawings the
/// stream makes, and 0 and one never made.
const WATCHABLE: u8 = 6;

/// An input's bytes, read in order; zeros once they run out.
struct Bytes<'a>(core::slice::Iter<'a, u8>);

impl Bytes<'_> {
    fn next(&mut self) -> u8 {
        self.0.next().copied().unwrap_or(0)
    }

    fn wide(&mut self) -> u32 {
        u32::from_be_bytes([0, 0, self.next(), self.next()])
    }

    fn ticks(&mut self) -> u32 {
        match self.next() {
            255 => u32::MAX,
            254 => self.wide(),
            small => u32::from(small % 16),
        }
    }

    fn pace(&mut self) -> Option<Pace> {
        match self.next() % 4 {
            0 => None,
            1 => Some(Pace { bytes: u32::from(self.next() % 9), ticks: u32::from(self.next() % 9) }),
            2 => Some(Pace { bytes: self.wide(), ticks: self.wide() }),
            _ => Some(Pace { bytes: u32::MAX, ticks: u32::from(self.next()) }),
        }
    }
}

/// The pixels, the pick record and the origins of a picture.
type Seen = (Vec<u8>, Vec<u16>, Vec<(u16, i32, i32)>);

/// Everything a caller reads of a picture but its work.
fn seen(terminal: &mut Terminal, screen: &Screen) -> Seen {
    let mut picture = Picture::new();
    terminal.compose(screen, &mut picture);
    (picture.canvas().pixels().to_vec(), picture.pick_record().to_vec(), picture.origins().collect())
}

/// Holds a report of finished paintings to what it may contain.
fn check_report(report: &[u16], watched: &BTreeSet<u16>) {
    assert!(report.len() <= MAX_WATCHED_PAINTINGS, "{} reports", report.len());
    assert!(report.windows(2).all(|pair| pair[0] < pair[1]), "reports out of order or twice: {report:?}");
    assert!(report.iter().all(|number| watched.contains(number)), "{report:?} were not watched: {watched:?}");
}

/// The fuzz target: one input. It panics when the property fails.
fn target(input: &[u8]) {
    let mut bytes = Bytes(input.iter());
    let framed = bytes.next() % 2 == 1;
    let mut terminal = terminal();
    let mut screen = Screen::new();
    let mut replay = if framed { Replay::framed() } else { Replay::new() };
    let mut pushed = Vec::new();
    let mut events = Vec::new();
    let mut watched = BTreeSet::new();
    let mut default_rate = 0;
    while !bytes.0.as_slice().is_empty() {
        match bytes.next() % 8 {
            0..=2 => {
                let count = bytes.next() % 12;
                let piece: Vec<u8> =
                    (0..count).flat_map(|_| PIECES[usize::from(bytes.next()) % PIECES.len()].iter().copied()).collect();
                pushed.extend_from_slice(&piece);
                events.extend(replay.push(&mut terminal, &mut screen, &piece));
            }
            3 => events.extend(replay.advance(&mut terminal, &mut screen, bytes.ticks())),
            4 => replay.set_pace(bytes.pace()),
            5 => {
                let names: Vec<u16> = (0..bytes.next() % 40).map(|_| u16::from(bytes.next() % WATCHABLE)).collect();
                // A list of more than 32 distinct numbers: refused. Some
                // inputs name more than the watchable few by adding 100s.
                let wide: Vec<u16> = if bytes.next() % 4 == 0 {
                    names.iter().enumerate().map(|(at, number)| number + 100 * u16::try_from(at).unwrap()).collect()
                } else {
                    names
                };
                let wide_distinct: BTreeSet<u16> = wide.iter().copied().collect();
                let accepted = terminal.watch_painted(&wide);
                assert_eq!(accepted, wide_distinct.len() <= MAX_WATCHED_PAINTINGS, "{} distinct", wide_distinct.len());
                if accepted {
                    watched = wide_distinct;
                }
            }
            6 => {
                default_rate = match bytes.next() % 4 {
                    0 => 0,
                    1 => u16::from(bytes.next() % 20),
                    2 => u16::try_from(bytes.wide()).unwrap_or(u16::MAX),
                    _ => u16::MAX,
                };
                terminal.set_default_paint_rate(default_rate);
            }
            _ => {
                let _ = terminal.advance_ticks(bytes.ticks());
            }
        }
        check_report(&terminal.take_painted(), &watched);
        assert!(replay.held_bytes() <= pushed.len());
        assert_eq!(replay.ticks_to_next().is_none(), replay.idle());
    }
    // Every byte fed, the input ended, and every painting finished.
    replay.set_pace(None);
    events.extend(replay.advance(&mut terminal, &mut screen, 0));
    assert!(replay.idle() && replay.ticks_to_next().is_none());
    events.extend(terminal.finish(&mut screen));
    let _ = terminal.advance_ticks(u32::MAX);
    assert!(!terminal.advance_ticks(u32::MAX), "a drawing never finishes painting");
    check_report(&terminal.take_painted(), &watched);

    let mut reference = terminal_with_default(default_rate);
    let mut reference_screen = Screen::new();
    let mut expected = if framed {
        reference.feed(&mut reference_screen, &pushed)
    } else {
        reference.feed_regis(&mut reference_screen, &pushed)
    };
    expected.extend(reference.finish(&mut reference_screen));
    let _ = reference.advance_ticks(u32::MAX);
    let _ = reference.advance_ticks(u32::MAX);
    assert!(events == expected, "the events differ from feeding at once:\n{events:?}\n{expected:?}");
    assert!(
        screen.displayed().pixels() == reference_screen.displayed().pixels(),
        "the screen differs from feeding at once"
    );
    assert!(
        seen(&mut terminal, &screen) == seen(&mut reference, &reference_screen),
        "the picture differs from feeding at once"
    );
    for number in 0..u16::from(WATCHABLE) {
        assert_eq!(terminal.committed_drawing(number), reference.committed_drawing(number), "drawing {number}");
    }
}

fn terminal_with_default(rate: u16) -> Terminal {
    let mut terminal = terminal();
    terminal.set_default_paint_rate(rate);
    terminal
}

/// Where the kept inputs are.
fn corpus() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("corpus/regis_replay")
}

/// The 64-bit FNV-1a hash (Fowler, Noll and Vo; the constants are the
/// algorithm's): it names a kept input by its contents.
fn fnv(bytes: &[u8]) -> u64 {
    const OFFSET_BASIS: u64 = 0xCBF2_9CE4_8422_2325;
    const PRIME: u64 = 0x0000_0100_0000_01B3;
    bytes.iter().fold(OFFSET_BASIS, |hash, &byte| (hash ^ u64::from(byte)).wrapping_mul(PRIME))
}

/// The most attempts made at reducing an input that fails, as for
/// `regis_animation`.
const MAX_REDUCTIONS: u32 = 20_000;

/// Runs the target on `cases` inputs of at most `length` bytes, made
/// anew on every run. An input that fails is reduced, kept in the corpus
/// folder and reported.
fn search(cases: u32, length: usize) {
    let config = Config { cases, failure_persistence: None, max_shrink_iters: MAX_REDUCTIONS, ..Config::default() };
    let mut runner = TestRunner::new(config);
    let result = runner.run(&vec(any::<u8>(), 0..=length), |input| {
        target(&input);
        Ok(())
    });
    match result {
        Ok(()) => {}
        Err(TestError::Fail(reason, input)) => {
            let kept = corpus().join(format!("found-{:016x}", fnv(&input)));
            std::fs::write(&kept, &input).unwrap();
            panic!(
                "an input of {} bytes fails: {reason}\nit is kept as {}: commit it with the fix",
                input.len(),
                kept.display()
            );
        }
        Err(TestError::Abort(reason)) => panic!("the search stopped: {reason}"),
    }
}

/// The inputs of a run of the push gate, and their longest.
const QUICK: (u32, usize) = (64, 600);
/// The inputs of a longer run, by hand, and their longest.
const LONG: (u32, usize) = (4000, 1500);

#[test]
fn every_kept_input_is_replayed() {
    let mut names: Vec<_> = std::fs::read_dir(corpus()).unwrap().map(|entry| entry.unwrap().path()).collect();
    names.sort();
    assert!(!names.is_empty(), "the kept inputs are missing");
    for name in names {
        target(&std::fs::read(name).unwrap());
    }
}

#[test]
fn generated_inputs_hold_the_property() {
    search(QUICK.0, QUICK.1);
}

#[test]
#[ignore = "a long run, by hand: cargo test -p regis-fuzz --test regis_replay --release -- --ignored"]
fn generated_inputs_hold_the_property_at_length() {
    search(LONG.0, LONG.1);
}
