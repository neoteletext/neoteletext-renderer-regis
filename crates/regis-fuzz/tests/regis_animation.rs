//! The fuzz target `regis_animation` (neoteletext-common
//! `docs/animation.md`, ANIM-23; REQ-6530): any stream with any sequence
//! of animation states and ticks. The picture repainted where it changed
//! equals the whole rebuild, in pixels and in the pick record, and its
//! work stays under the cap on one repaint.
//!
//! [`target`] takes one input's bytes. It is run first on every input
//! kept in `corpus/regis_animation/`, where each input that ever found a
//! fault is kept, and then on inputs a property-testing library makes:
//! new ones on every run, so that the gates search and do not only
//! repeat. An input that fails is reduced to the least that still fails
//! and written into the corpus folder, to be committed with the fix.
//!
//! cargo-fuzz would guide the search by coverage, but its engine is C++
//! and the renderer takes no native code (DEC-6579).

use std::path::{Path, PathBuf};

use proptest::collection::vec;
use proptest::prelude::any;
use proptest::test_runner::{Config, TestError, TestRunner};
use regis::limits::MAX_REBUILD_WORK;
use regis::{AnimationState, Font, ForcedLook, Picture, Screen, Terminal, ViewerState};

/// A terminal with the shipped font.
fn terminal() -> Terminal {
    let mut terminal = Terminal::new();
    terminal.set_font(Font::parse(regis_fonts::VT340).unwrap());
    terminal
}

/// The bytes that ReGIS v2 operations are made of: an input's bytes are
/// mapped onto these as well as fed raw, so that inputs reach the `D`
/// command's grammar and not only its refusals.
const ALPHABET: &[u8] = b"D(B1)D(E)D(S2(,))C0123456789XYLVPRIHANFGTKZW[]-+'@:;aV[30,40]F(V[0,0][50,0][25,45])";

/// The drawing numbers animation states are given to: few, so that they
/// meet the drawings the stream makes.
const DRAWINGS: u8 = 12;

/// An input's bytes, read in order; zeros once they run out.
struct Bytes<'a>(core::slice::Iter<'a, u8>);

impl Bytes<'_> {
    fn next(&mut self) -> u8 {
        self.0.next().copied().unwrap_or(0)
    }

    fn drawing(&mut self) -> u16 {
        u16::from(self.next() % DRAWINGS)
    }

    fn state(&mut self) -> AnimationState {
        let parts = self.next();
        let mut signed = || i16::from(self.next().cast_signed()) * 4;
        let position = (signed(), signed());
        AnimationState {
            shown: (parts & 1 != 0).then_some(parts & 2 != 0),
            position: (parts & 4 != 0).then_some(position),
            layer: (parts & 8 != 0).then_some(self.next()),
            order: (parts & 16 != 0).then_some(self.next().cast_signed()),
            frame: self.next(),
            look: ForcedLook::from_code(parts >> 6).unwrap_or_default(),
        }
    }
}

/// The pixels, the pick record, the origins and the commit count.
type Seen<'a> = (&'a [u8], &'a [u16], Vec<(u16, i32, i32)>, u32);

/// Everything a caller can read of a picture but its work.
fn seen(picture: &Picture) -> Seen<'_> {
    (picture.canvas().pixels(), picture.pick_record(), picture.origins().collect(), picture.commits())
}

/// Composes into the picture kept throughout and into a new one, and
/// holds them to the property.
fn check(terminal: &mut Terminal, screen: &Screen, kept: &mut Picture) {
    terminal.compose(screen, kept);
    let mut fresh = Picture::new();
    terminal.compose(screen, &mut fresh);
    assert!(seen(kept) == seen(&fresh), "the picture repainted where it changed is not the whole rebuild");
    assert!(fresh.work() <= MAX_REBUILD_WORK, "a whole repaint cost {}", fresh.work());
    assert!(kept.work() <= fresh.work(), "a partial repaint cost {} and a whole one {}", kept.work(), fresh.work());
}

/// The fuzz target: one input. It panics when the property fails.
fn target(input: &[u8]) {
    let mut bytes = Bytes(input.iter());
    let mut terminal = terminal();
    let mut screen = Screen::new();
    let mut kept = Picture::new();
    while !bytes.0.as_slice().is_empty() {
        match bytes.next() % 10 {
            0..=3 => {
                let mapped: Vec<u8> =
                    (0..bytes.next() % 48).map(|_| ALPHABET[usize::from(bytes.next()) % ALPHABET.len()]).collect();
                let _ = terminal.feed_regis(&mut screen, &mapped);
            }
            4 => {
                let raw: Vec<u8> = (0..bytes.next() % 48).map(|_| bytes.next()).collect();
                let _ = terminal.feed_regis(&mut screen, &raw);
            }
            5 | 6 => {
                let number = bytes.drawing();
                let state = bytes.state();
                let _ = terminal.set_animation(number, state);
                let _ = terminal.bounds(number);
                let _ = terminal.drawing_position(number);
            }
            7 => terminal.clear_animation(bytes.drawing()),
            8 => {
                let _ = terminal.advance_ticks(u32::from(bytes.next()));
            }
            _ => {
                let state = ViewerState { hovered: bytes.drawing(), pressed: bytes.drawing(), hidden: bytes.drawing() };
                let _ = terminal.set_viewer_state(state);
            }
        }
        if bytes.next() % 3 == 0 {
            check(&mut terminal, &screen, &mut kept);
        }
    }
    let _ = terminal.finish(&mut screen);
    check(&mut terminal, &screen, &mut kept);
    // With every tick there will ever be, nothing is still painting.
    let _ = terminal.advance_ticks(u32::MAX);
    assert!(!terminal.advance_ticks(u32::MAX), "a drawing never finishes painting");
    check(&mut terminal, &screen, &mut kept);
}

/// Where the kept inputs are.
fn corpus() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("corpus/regis_animation")
}

/// The 64-bit FNV-1a hash (Fowler, Noll and Vo; the constants are the
/// algorithm's): it names a kept input by its contents.
fn fnv(bytes: &[u8]) -> u64 {
    const OFFSET_BASIS: u64 = 0xCBF2_9CE4_8422_2325;
    const PRIME: u64 = 0x0000_0100_0000_01B3;
    bytes.iter().fold(OFFSET_BASIS, |hash, &byte| (hash ^ u64::from(byte)).wrapping_mul(PRIME))
}

/// The most attempts made at reducing an input that fails. The library's
/// own default is four times the cases, which leaves an input of hundreds
/// of bytes nearly as it was found; an attempt is one run of the target
/// on a smaller input, a few milliseconds.
const MAX_REDUCTIONS: u32 = 20_000;

/// Runs the target on `cases` inputs of at most `length` bytes, made
/// anew on every run. An input that fails is reduced, kept in the corpus
/// folder and reported.
fn search(cases: u32, length: usize) {
    // The reduced input is kept as a file, which says more than the seed
    // the library would keep.
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
const QUICK: (u32, usize) = (48, 600);
/// The inputs of a run of the full gate, and their longest.
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
#[ignore = "the long run of the full gate: scripts/check.sh full"]
fn generated_inputs_hold_the_property_at_length() {
    search(LONG.0, LONG.1);
}
