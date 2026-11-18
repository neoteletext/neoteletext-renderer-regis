//! The music player's busiest frame, measured (neoteletext-common
//! `docs/page-rules.md`, RULE-65 and RULE-92; REQ-6530): on page 101 a key
//! pressed and the meter's lowest segment lit in one compose. The browser's
//! timed test (RULE-66) found that frame missing the display composition
//! its paint point was for, its compose and its drawing together longer
//! than RULE-98's 4 millisecond lead. The stream is the music player's
//! own picture of page 101 (`tests/data/musicplayer-101.regis`, made by
//! the channel's `picture()` for the synth106), and the changes are those
//! its rule table makes: the key's pressed look forced, the segment shown.
//!
//! It prints its numbers, holds the work, which is the same on every
//! machine, exactly, and holds the time in an optimised build.

mod common;

use std::time::{Duration, Instant};

use regis::{AnimationState, ForcedLook, Picture, Screen, Terminal};

/// The key A, note 60: its drawing (RULE-92).
const KEY_A: u16 = 101;
/// The meter's lowest segment (RULE-92).
const FIRST_SEGMENT: u16 = 301;
/// The frames measured of each kind, alternately changed and changed back.
const FRAMES: usize = 400;

/// The work units of the key and the segment changed together, the more
/// costly of the two ways round (pressed and lit, released and dark): the
/// pressed key's contents replayed in its rectangle, the part of the black
/// key W that lies over it, the segment's, and a visit to each of the
/// page's drawings. Measured, and the same on every machine.
const KEY_AND_METER_WORK: u64 = 59_142;

/// The most the key-and-meter frame may take in the median in an
/// optimised build. RULE-65 gives the repaint 1 millisecond; on chunko
/// (Ubuntu, 20 processors) the frame took 271 microseconds in the median
/// before fills were written a row's run at a time, the pick record was
/// tidied without searching it whole and only the repainted rectangles
/// were kept to find the changed rows, and 61 after (283 and 62 measured
/// one after the other at a load average of 14).
const KEY_AND_METER_MEDIAN: Duration = Duration::from_micros(200);

fn built() -> (Terminal, Screen) {
    let mut terminal = common::terminal();
    let mut screen = Screen::new();
    let events = terminal.feed_regis(&mut screen, &common::read("musicplayer-101.regis"));
    assert_eq!(events, [regis::Event::Committed(1)]);
    (terminal, screen)
}

/// What one kind of frame cost.
struct Cost {
    work: Vec<u64>,
    median: Duration,
}

/// Composes [`FRAMES`] frames, each changing the key's look if `key` and
/// showing or hiding the first `segments` of the meter.
fn measure(name: &str, key: bool, segments: u16) -> Cost {
    let (mut terminal, screen) = built();
    let mut picture = Picture::new();
    terminal.compose(&screen, &mut picture);
    let mut work = Vec::new();
    let mut times = Vec::new();
    for frame in 0..FRAMES {
        let on = frame % 2 == 0;
        let started = Instant::now();
        if key {
            let look = if on { ForcedLook::Pressed } else { ForcedLook::None };
            assert!(terminal.set_animation(KEY_A, AnimationState { look, ..AnimationState::default() }));
        }
        for segment in FIRST_SEGMENT..FIRST_SEGMENT + segments {
            assert!(terminal.set_animation(segment, AnimationState { shown: Some(on), ..AnimationState::default() }));
        }
        terminal.compose(&screen, &mut picture);
        times.push(started.elapsed());
        work.push(picture.work());
    }
    times.sort_unstable();
    let cost = Cost { median: times[FRAMES / 2], work };
    println!(
        "  {name}: {} to {} work units; {:?} in the median, {:?} at the 90th percentile",
        cost.work.iter().min().unwrap(),
        cost.work.iter().max().unwrap(),
        cost.median,
        times[FRAMES * 9 / 10]
    );
    cost
}

#[test]
fn a_key_pressed_and_the_meter_lit_together_cost_their_own_rectangles() {
    println!("the music player's page 101, {FRAMES} frames of each kind:");
    let nothing = measure("nothing changed", false, 0);
    let _ = measure("the segment alone", false, 1);
    let _ = measure("the key alone", true, 0);
    let both = measure("the key and the segment", true, 1);
    let _ = measure("the key and all eight segments", true, 8);
    assert!(nothing.work.iter().all(|work| *work < KEY_AND_METER_WORK / 10), "an unchanged frame repainted something");
    assert_eq!(both.work.iter().max(), Some(&KEY_AND_METER_WORK));
    if !cfg!(debug_assertions) {
        assert!(both.median <= KEY_AND_METER_MEDIAN, "{:?} against {KEY_AND_METER_MEDIAN:?}", both.median);
    }
}
