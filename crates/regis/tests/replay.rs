//! Replay at a pace (REQ-7024; DEC-7031 item 2): a stream's bytes held
//! and given to the terminal a few each tick, as a serial line would
//! deliver them, with the builder's readings in `docs/regis-v2.md`, "What
//! building changed", item 16.

use regis::limits::MAX_REPLAY_HELD_BYTES;
use regis::{Event, Pace, Picture, Replay, Screen, Terminal};

/// A ReGIS v1 picture of several commands: lines, a circle and a fill.
const PICTURE: &str =
    "W(I3)P[0,0]V[599,0]P[0,5]V[100,5]W(I5)P[300,200]C[+50]W(I6)F(V[10,300][90,300][50,380])P[20,20]V[+40,+40]";

/// The same picture in a terminal data stream: two device control
/// strings, with bytes between them that ReGIS does not see.
fn framed_picture() -> Vec<u8> {
    let (first, second) = PICTURE.split_at(40);
    [b"\x1bPp", first.as_bytes(), b"\x1b\\ignored text\x1bPp", second.as_bytes(), b"\x1b\\"].concat()
}

/// A terminal fed `bytes` at once, as `framed` says, and its screen.
fn fed_at_once(framed: bool, bytes: &[u8]) -> (Terminal, Screen, Vec<Event>) {
    let mut terminal = Terminal::new();
    let mut screen = Screen::new();
    let events = if framed { terminal.feed(&mut screen, bytes) } else { terminal.feed_regis(&mut screen, bytes) };
    (terminal, screen, events)
}

fn replay(framed: bool) -> Replay {
    if framed { Replay::framed() } else { Replay::new() }
}

/// No events, to compare with.
const NO_EVENTS: Vec<Event> = Vec::new();

/// The pixels of the displayed page.
fn page(screen: &Screen) -> Vec<u8> {
    screen.displayed().pixels().to_vec()
}

/// The pixels and the pick record of the composed picture.
fn seen(terminal: &mut Terminal, screen: &Screen) -> (Vec<u8>, Vec<u16>) {
    let mut picture = Picture::new();
    terminal.compose(screen, &mut picture);
    (picture.canvas().pixels().to_vec(), picture.pick_record().to_vec())
}

/// The bytes a pace owes after `ticks`: `ticks × bytes ÷ ticks of the
/// pace`, rounded down from the total, each part 0 taken as 1.
fn owed(pace: Pace, ticks: u64) -> u64 {
    let owed = u128::from(ticks) * u128::from(pace.bytes.max(1)) / u128::from(pace.ticks.max(1));
    u64::try_from(owed).unwrap_or(u64::MAX)
}

#[test]
fn at_each_tick_the_screen_is_a_terminal_fed_the_bytes_owed_by_then() {
    for framed in [false, true] {
        let stream = if framed { framed_picture() } else { PICTURE.as_bytes().to_vec() };
        for pace in [Pace { bytes: 2, ticks: 3 }, Pace { bytes: 7, ticks: 5 }, Pace { bytes: 1, ticks: 1 }] {
            // Steps of varied size, so that rounding from the total is
            // seen to differ from rounding each step.
            for steps in [&[1_u32][..], &[1, 2, 3, 5, 1, 4]] {
                let mut terminal = Terminal::new();
                let mut screen = Screen::new();
                let mut replay = replay(framed);
                replay.set_pace(Some(pace));
                assert_eq!(replay.push(&mut terminal, &mut screen, &stream), NO_EVENTS);
                let mut ticks = 0_u64;
                for step in steps.iter().cycle() {
                    let _ = replay.advance(&mut terminal, &mut screen, *step);
                    ticks += u64::from(*step);
                    let want = usize::try_from(owed(pace, ticks)).unwrap().min(stream.len());
                    assert_eq!(stream.len() - replay.held_bytes(), want, "{pace:?} {steps:?} after {ticks} ticks");
                    let (_, prefix_screen, _) = fed_at_once(framed, &stream[..want]);
                    assert!(
                        page(&screen) == page(&prefix_screen),
                        "{pace:?} {steps:?}: the screen after {ticks} ticks"
                    );
                    if replay.idle() {
                        break;
                    }
                }
            }
        }
    }
}

#[test]
fn the_last_frame_equals_the_stream_fed_at_once_for_new_and_framed() {
    for framed in [false, true] {
        let stream = if framed { framed_picture() } else { PICTURE.as_bytes().to_vec() };
        let (mut whole, whole_screen, _) = fed_at_once(framed, &stream);
        let mut terminal = Terminal::new();
        let mut screen = Screen::new();
        let mut replay = replay(framed);
        replay.set_pace(Some(Pace { bytes: 3, ticks: 2 }));
        let _ = replay.push(&mut terminal, &mut screen, &stream);
        assert!(page(&screen) != page(&whole_screen), "framed {framed}: everything was fed at the push");
        while !replay.idle() {
            let _ = replay.advance(&mut terminal, &mut screen, 1);
        }
        assert!(page(&screen) == page(&whole_screen), "framed {framed}: the last frame differs");
        assert!(
            seen(&mut terminal, &screen) == seen(&mut whole, &whole_screen),
            "framed {framed}: the picture differs"
        );
    }
}

#[test]
fn what_the_pace_left_unspent_is_forgotten_once_the_hold_empties() {
    let mut terminal = Terminal::new();
    let mut screen = Screen::new();
    let mut replay = Replay::new();
    replay.set_pace(Some(Pace { bytes: 1, ticks: 10 }));
    let _ = replay.push(&mut terminal, &mut screen, b" ");
    let _ = replay.advance(&mut terminal, &mut screen, 100);
    assert!(replay.idle());
    // Bytes arriving on an idle line wait their full time.
    let _ = replay.push(&mut terminal, &mut screen, b"P[0,0]");
    assert_eq!(replay.ticks_to_next(), Some(10));
    let _ = replay.advance(&mut terminal, &mut screen, 9);
    assert_eq!(replay.held_bytes(), 6);
    let _ = replay.advance(&mut terminal, &mut screen, 1);
    assert_eq!(replay.held_bytes(), 5);
}

#[test]
fn bytes_pushed_during_a_run_join_it() {
    let stream = PICTURE.as_bytes();
    let (first, second) = stream.split_at(30);
    let mut terminal = Terminal::new();
    let mut screen = Screen::new();
    let mut replay = Replay::new();
    replay.set_pace(Some(Pace { bytes: 3, ticks: 1 }));
    let _ = replay.push(&mut terminal, &mut screen, first);
    let _ = replay.advance(&mut terminal, &mut screen, 4);
    assert_eq!(replay.held_bytes(), 18);
    assert_eq!(replay.push(&mut terminal, &mut screen, second), NO_EVENTS);
    assert_eq!(replay.held_bytes(), stream.len() - 12);
    // The run goes on at its pace, without starting again.
    let _ = replay.advance(&mut terminal, &mut screen, 1);
    assert_eq!(replay.held_bytes(), stream.len() - 15);
    for tick in 6.. {
        let _ = replay.advance(&mut terminal, &mut screen, 1);
        let want = (tick * 3).min(stream.len());
        assert_eq!(stream.len() - replay.held_bytes(), want);
        let (_, prefix_screen, _) = fed_at_once(false, &stream[..want]);
        assert!(page(&screen) == page(&prefix_screen), "after {tick} ticks");
        if replay.idle() {
            break;
        }
    }
}

#[test]
fn the_hold_takes_exactly_its_cap_and_one_byte_more_feeds_everything_and_ends_the_pace() {
    let mut stream = b"W(I3)P[0,0]V[599,0]".to_vec();
    stream.resize(MAX_REPLAY_HELD_BYTES, b' ');
    let (_, whole_screen, _) = fed_at_once(false, &stream);
    let mut terminal = Terminal::new();
    let mut screen = Screen::new();
    let blank = page(&screen);
    let mut replay = Replay::new();
    replay.set_pace(Some(Pace { bytes: 1, ticks: 1 }));
    // In two pieces, with bytes fed between them: what was fed is not held.
    let (first, second) = stream.split_at(1000);
    let _ = replay.push(&mut terminal, &mut screen, first);
    let _ = replay.advance(&mut terminal, &mut screen, 10);
    let _ = replay.push(&mut terminal, &mut screen, second);
    assert_eq!(replay.held_bytes(), MAX_REPLAY_HELD_BYTES - 10);
    let _ = replay.push(&mut terminal, &mut screen, &[b' '; 10]);
    assert_eq!(replay.held_bytes(), MAX_REPLAY_HELD_BYTES, "the cap is held");
    assert!(page(&screen) == blank, "the line was drawn before its bytes were fed");
    let _ = replay.push(&mut terminal, &mut screen, b" ");
    assert!(replay.idle() && replay.ticks_to_next().is_none());
    assert!(page(&screen) == page(&whole_screen), "the screen is not the stream fed at once");
    // The pace is now none: what follows is fed at once.
    let _ = replay.push(&mut terminal, &mut screen, b"P[0,9]V[599,9]");
    assert!(replay.idle());
    assert_eq!(screen.displayed().pixel(300, 9), Some(3));
}

#[test]
fn a_replay_with_no_pace_is_feed_regis_and_a_framed_one_feed() {
    let pieces: [&[u8]; 4] = [b"W(I3)P[0,0]V[599,0]D(B1)", b"V[10,10]D(E)D(S1(X3),C)", b"R(P)", b"D(S1(P9),C)P[5,5]"];
    for framed in [false, true] {
        let mut terminal = Terminal::new();
        let mut screen = Screen::new();
        let mut replay = replay(framed);
        let mut reference = Terminal::new();
        let mut reference_screen = Screen::new();
        let mut events = Vec::new();
        let mut expected = Vec::new();
        for (index, piece) in pieces.iter().enumerate() {
            let piece = if framed { [b"\x1bPp", *piece, b"\x1b\\"].concat() } else { piece.to_vec() };
            events.extend(replay.push(&mut terminal, &mut screen, &piece));
            assert!(replay.idle() && replay.ticks_to_next().is_none(), "piece {index}");
            expected.extend(if framed {
                reference.feed(&mut reference_screen, &piece)
            } else {
                reference.feed_regis(&mut reference_screen, &piece)
            });
            assert_eq!(events, expected, "framed {framed}, piece {index}");
        }
        assert!(events.len() >= 3, "{events:?}");
        assert!(page(&screen) == page(&reference_screen), "framed {framed}: the screen differs");
        assert!(
            seen(&mut terminal, &screen) == seen(&mut reference, &reference_screen),
            "framed {framed}: the picture differs"
        );
        assert_eq!(replay.advance(&mut terminal, &mut screen, 10), NO_EVENTS);
    }
}

#[test]
fn a_pace_of_0_bytes_or_0_ticks_is_taken_as_1() {
    for (pace, after_ten) in
        [(Pace { bytes: 0, ticks: 5 }, 2), (Pace { bytes: 3, ticks: 0 }, 30), (Pace { bytes: 0, ticks: 0 }, 10)]
    {
        let mut terminal = Terminal::new();
        let mut screen = Screen::new();
        let mut replay = Replay::new();
        replay.set_pace(Some(pace));
        let _ = replay.push(&mut terminal, &mut screen, PICTURE.as_bytes());
        let _ = replay.advance(&mut terminal, &mut screen, 10);
        assert_eq!(PICTURE.len() - replay.held_bytes(), after_ten, "{pace:?}");
    }
}

#[test]
fn ticks_to_next_names_the_tick_that_feeds_the_next_byte() {
    let mut terminal = Terminal::new();
    let mut screen = Screen::new();
    let mut replay = Replay::new();
    assert_eq!(replay.ticks_to_next(), None);
    // 2 bytes every 3 ticks: bytes become due at ticks 2, 3, 5, 6, 8 ...
    replay.set_pace(Some(Pace { bytes: 2, ticks: 3 }));
    let _ = replay.push(&mut terminal, &mut screen, PICTURE.as_bytes());
    let mut seen_due = Vec::new();
    let mut ticks = 0;
    while ticks < 12 {
        let due = replay.ticks_to_next().unwrap();
        assert!(due >= 1, "under a pace the next byte is at least a tick away");
        // One tick short feeds nothing; the tick named feeds a byte.
        let held = replay.held_bytes();
        let _ = replay.advance(&mut terminal, &mut screen, due - 1);
        assert_eq!(replay.held_bytes(), held, "a byte was fed before the tick named");
        let _ = replay.advance(&mut terminal, &mut screen, 1);
        assert!(replay.held_bytes() < held);
        ticks += due;
        seen_due.push(ticks);
    }
    assert_eq!(seen_due, [2, 3, 5, 6, 8, 9, 11, 12]);
    // More than a byte a tick: the next is always one tick away.
    replay.set_pace(Some(Pace { bytes: 5, ticks: 1 }));
    assert_eq!(replay.ticks_to_next(), Some(1));
    // Held with no pace: due now.
    replay.set_pace(None);
    assert_eq!(replay.ticks_to_next(), Some(0));
    let _ = replay.advance(&mut terminal, &mut screen, 0);
    assert_eq!(replay.ticks_to_next(), None);
}

#[test]
fn advancing_by_0_ticks_feeds_nothing_under_a_pace() {
    let mut terminal = Terminal::new();
    let mut screen = Screen::new();
    let mut replay = Replay::new();
    replay.set_pace(Some(Pace { bytes: 100, ticks: 1 }));
    let _ = replay.push(&mut terminal, &mut screen, PICTURE.as_bytes());
    for _ in 0..5 {
        assert_eq!(replay.advance(&mut terminal, &mut screen, 0), NO_EVENTS);
    }
    assert_eq!(replay.held_bytes(), PICTURE.len());
}

#[test]
fn advancing_by_the_most_ticks_again_and_again_feeds_everything_and_overflows_nothing() {
    for pace in [Pace { bytes: 1, ticks: 1 }, Pace { bytes: u32::MAX, ticks: 1 }, Pace { bytes: 1, ticks: u32::MAX }] {
        let mut terminal = Terminal::new();
        let mut screen = Screen::new();
        let mut replay = Replay::new();
        replay.set_pace(Some(pace));
        let _ = replay.push(&mut terminal, &mut screen, PICTURE.as_bytes());
        for round in 1..=4_u64 {
            let _ = replay.advance(&mut terminal, &mut screen, u32::MAX);
            let want = owed(pace, round * u64::from(u32::MAX)).min(PICTURE.len() as u64);
            assert_eq!((PICTURE.len() - replay.held_bytes()) as u64, want, "{pace:?} round {round}");
            let _ = replay.ticks_to_next();
        }
        // Pushed again, a new run.
        let _ = replay.push(&mut terminal, &mut screen, PICTURE.as_bytes());
        let _ = replay.advance(&mut terminal, &mut screen, u32::MAX);
        let _ = replay.advance(&mut terminal, &mut screen, u32::MAX);
    }
}

#[test]
fn a_change_of_pace_begins_a_new_run_the_same_pace_does_not_and_none_feeds_the_rest() {
    let stream = PICTURE.as_bytes();
    let (_, whole_screen, _) = fed_at_once(false, stream);
    let mut terminal = Terminal::new();
    let mut screen = Screen::new();
    let mut replay = Replay::new();
    replay.set_pace(Some(Pace { bytes: 1, ticks: 2 }));
    let _ = replay.push(&mut terminal, &mut screen, stream);
    let _ = replay.advance(&mut terminal, &mut screen, 1);
    // The same pace again keeps the tick already passed.
    replay.set_pace(Some(Pace { bytes: 1, ticks: 2 }));
    let _ = replay.advance(&mut terminal, &mut screen, 1);
    assert_eq!(replay.held_bytes(), stream.len() - 1);
    // Another pace starts its run from what is held: 2 bytes every 3
    // ticks, the first due at its second tick.
    replay.set_pace(Some(Pace { bytes: 2, ticks: 3 }));
    let _ = replay.advance(&mut terminal, &mut screen, 1);
    assert_eq!(replay.held_bytes(), stream.len() - 1);
    let _ = replay.advance(&mut terminal, &mut screen, 1);
    assert_eq!(replay.held_bytes(), stream.len() - 2);
    // None: held until the next call, which feeds the rest.
    replay.set_pace(None);
    assert_eq!(replay.held_bytes(), stream.len() - 2);
    let _ = replay.advance(&mut terminal, &mut screen, 0);
    assert!(replay.idle());
    assert!(page(&screen) == page(&whole_screen), "the screen is not the stream fed at once");
}

#[test]
fn the_events_come_out_of_the_call_that_fed_their_bytes() {
    let stream = b"D(B1)P[0,0]V[9,9]D(E)D(C)R(P)D(S1(X5),C)D(S1(P7),C)".as_slice();
    // The events of each byte, fed one at a time.
    let mut reference = Terminal::new();
    let mut reference_screen = Screen::new();
    let per_byte: Vec<Vec<Event>> =
        stream.iter().map(|byte| reference.feed_regis(&mut reference_screen, &[*byte])).collect();
    assert!(per_byte.iter().filter(|events| !events.is_empty()).count() >= 4);
    for bytes_a_tick in [1_usize, 3] {
        let mut terminal = Terminal::new();
        let mut screen = Screen::new();
        let mut replay = Replay::new();
        replay.set_pace(Some(Pace { bytes: u32::try_from(bytes_a_tick).unwrap(), ticks: 1 }));
        assert_eq!(replay.push(&mut terminal, &mut screen, stream), NO_EVENTS);
        for (tick, chunk) in per_byte.chunks(bytes_a_tick).enumerate() {
            let expected: Vec<Event> = chunk.concat();
            assert_eq!(
                replay.advance(&mut terminal, &mut screen, 1),
                expected,
                "{bytes_a_tick} a tick, tick {}",
                tick + 1
            );
        }
        assert!(replay.idle());
    }
}

#[test]
fn a_regis_v2_stream_under_a_pace_shows_each_drawing_at_its_commit_then_paints_it_at_the_rate_in_force() {
    // Drawing 1 has no T and paints at the default 6, 10 pixels a tick;
    // drawing 2 has T0 and appears whole at its commit.
    let first = "D(B1)W(I3)P[0,0]V[599,0]D(E)D(C)";
    let second = "D(B2)W(I3)P[0,10]V[599,10]D(E)D(S2(T0),C)";
    let stream = format!("{first}{second}");
    let pace = Pace { bytes: 4, ticks: 1 };
    // A commit is made when its option letter `C` is read.
    let commit_at = |end: &str| (end.rfind('C').unwrap() + 1).div_ceil(4);
    let first_commit_tick = commit_at(first);
    let second_commit_tick = commit_at(&stream);
    let mut terminal = Terminal::new();
    terminal.set_default_paint_rate(6);
    let mut screen = Screen::new();
    let mut replay = Replay::new();
    replay.set_pace(Some(pace));
    let _ = replay.push(&mut terminal, &mut screen, stream.as_bytes());
    let row = |terminal: &mut Terminal, screen: &Screen, row: u32| {
        let mut picture = Picture::new();
        terminal.compose(screen, &mut picture);
        (0..800).filter(|x| picture.canvas().pixel(*x, row) == Some(3)).count()
    };
    for tick in 1..=200 {
        // As regis-render does: the paintings earn the tick, then the
        // line delivers its bytes.
        let _ = terminal.advance_ticks(1);
        let events = replay.advance(&mut terminal, &mut screen, 1);
        let committed: Vec<&Event> = events.iter().filter(|event| matches!(event, Event::Committed(_))).collect();
        let expected_commit = tick == first_commit_tick || tick == second_commit_tick;
        assert_eq!(!committed.is_empty(), expected_commit, "tick {tick}: {events:?}");
        assert_eq!(terminal.committed_drawing(1).is_some(), tick >= first_commit_tick, "tick {tick}");
        assert_eq!(terminal.committed_drawing(2).is_some(), tick >= second_commit_tick, "tick {tick}");
        let painted = (tick.saturating_sub(first_commit_tick) * 10).min(600);
        let drawing_1 = if tick >= first_commit_tick { painted } else { 0 };
        assert_eq!(row(&mut terminal, &screen, 0), drawing_1, "drawing 1 at tick {tick}");
        assert_eq!(
            row(&mut terminal, &screen, 10),
            if tick >= second_commit_tick { 600 } else { 0 },
            "drawing 2 at tick {tick}"
        );
    }
    assert!(replay.idle());
}
