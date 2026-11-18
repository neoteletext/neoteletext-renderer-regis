//! The fuzz target `regis_animation` (neoteletext-common
//! `docs/animation.md`, ANIM-23; REQ-6530): any stream with any sequence
//! of animation states and ticks. The picture repainted where it changed
//! equals the whole rebuild, in pixels and in the pick record, and its
//! work stays under the cap on one repaint.
//!
//! The workspace has no library, so no fuzzing engine drives this yet:
//! [`target`] is the function such an engine would call with its bytes.
//! Until one is approved it is run on every input kept in
//! `tests/data/regis_animation/` (each input that ever found a fault is
//! kept there and replayed first) and on generated inputs.

mod common;

use regis::limits::MAX_REBUILD_WORK;
use regis::{AnimationState, ForcedLook, Picture, Screen, Terminal, ViewerState};

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
    let mut terminal = common::terminal();
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

/// Generated inputs: the same on every run.
fn generated(seed: u64, inputs: u32, length: usize) {
    let mut state = seed;
    for _ in 0..inputs {
        let input: Vec<u8> = (0..length)
            .map(|_| {
                state = state.wrapping_mul(6_364_136_223_846_793_005).wrapping_add(1_442_695_040_888_963_407);
                u8::try_from((state >> 33) & 0xFF).unwrap()
            })
            .collect();
        target(&input);
    }
}

#[test]
fn every_kept_input_is_replayed() {
    let mut names: Vec<_> =
        std::fs::read_dir(common::data("regis_animation")).unwrap().map(|entry| entry.unwrap().path()).collect();
    names.sort();
    assert!(!names.is_empty(), "the kept inputs are missing");
    for name in names {
        target(&std::fs::read(name).unwrap());
    }
}

#[test]
fn generated_inputs_hold_the_property() {
    generated(0x0123_4567_89AB_CDEF, 40, 600);
}

#[test]
#[ignore = "the long run of the full gate: scripts/check.sh full"]
fn generated_inputs_hold_the_property_at_length() {
    generated(0xFEDC_BA98_7654_3210, 4000, 1500);
}
