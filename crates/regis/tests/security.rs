//! Regression tests for the issues recorded in `SOLVEDSECURITYISSUES`.

mod common;

use std::time::Instant;

use regis::{Event, Screen};

/// Feeds commands in one call and returns the events.
fn events_of(commands: &[u8], macrograph_reports: bool) -> Vec<Event> {
    let mut terminal = common::terminal();
    terminal.allow_macrograph_reports(macrograph_reports);
    let mut screen = Screen::new();
    terminal.feed_regis(&mut screen, commands)
}

#[test]
fn a_stream_of_erases_is_bounded_by_the_work_allowance() {
    // Issue 001. A megabyte of erases: unbounded, that is 96 GB of writes.
    let stream = b"S(E)".repeat(250_000);
    let started = Instant::now();
    let events = events_of(&stream, false);
    assert!(events.contains(&Event::LimitReached));
    assert!(started.elapsed().as_secs() < 10);
}

/// Issue 002's stream: five megabytes of screen-sized fills, 25 bytes
/// each.
fn screen_fills() -> Vec<u8> {
    b"F(V[0,0][799,0][799,479])".repeat(200_000)
}

#[test]
fn fills_after_the_allowance_is_spent_do_no_work() {
    // Issue 002. Only as many fills are drawn as the bytes pay for, and
    // the rest cost nothing but their parsing. The claim is held in work
    // units, which are the same on every machine and in every build: the
    // time such a stream takes is held by the test below.
    let stream = screen_fills();
    let mut terminal = common::terminal();
    let mut screen = Screen::new();
    let events = terminal.feed_regis(&mut screen, &stream);
    assert!(events.contains(&Event::LimitReached));
    // Everything the stream could have paid for: the allowance at the
    // start and what each byte earns.
    let bytes = u64::try_from(stream.len()).unwrap();
    let allowance = regis::limits::WORK_CAP + bytes * regis::limits::WORK_PER_BYTE;
    let spent = terminal.base_work();
    // One fill visits each pixel of the screen twice, to mark it and to
    // write it, and pays a little more for its edges; three visits a
    // pixel is above that.
    let one_fill = u64::from(regis::SCREEN_WIDTH) * u64::from(regis::SCREEN_HEIGHT) * 3;
    println!("{bytes} bytes of fills: {spent} work units spent of {allowance} earned");
    assert!(spent <= allowance, "{spent} work units were spent and {allowance} earned");
    // The allowance was used as far as it went: what is left would not
    // pay for one fill more, so the bound above is not met by drawing
    // nothing.
    assert!(allowance - spent < one_fill, "{} work units were left unspent", allowance - spent);
}

/// The longest the stream of fills may take in an optimised build. It
/// took 0.48 to 0.53 seconds on the development machine (18 November
/// 2026, eight runs under load); ten times that leaves room for a slower
/// or busier machine and still fails long before unbounded filling
/// would end, which is 200,000 fills of 384,000 pixels.
const FILLS_SECONDS: u64 = 5;

#[test]
#[cfg_attr(
    debug_assertions,
    ignore = "timed in an optimised build, where the bound means something: cargo test --release, which scripts/check.sh full runs"
)]
fn the_stream_of_fills_is_read_within_its_time() {
    // Issue 002, in time. A debug build spends 14 seconds on the 1.3
    // thousand million work units this stream pays for, which is the
    // price of the build and not of the stream, so no bound in seconds
    // says anything there; the work is bounded in both builds by the
    // test above.
    let stream = screen_fills();
    let started = Instant::now();
    let events = events_of(&stream, false);
    let taken = started.elapsed();
    println!("{} bytes of fills took {taken:?}; the bound is {FILLS_SECONDS} s", stream.len());
    assert!(events.contains(&Event::LimitReached));
    assert!(taken.as_secs() < FILLS_SECONDS, "the fills took {taken:?}");
}

#[test]
fn reply_bytes_in_one_call_are_capped() {
    // Issue 003.
    let mut stream = b"@:A".to_vec();
    stream.extend(std::iter::repeat_n(b'x', 16_000));
    stream.extend_from_slice(b"@;");
    stream.extend(b"R(M(A))".repeat(1000));
    let events = events_of(&stream, true);
    let reply_bytes: usize =
        events.iter().map(|event| if let Event::Reply(bytes) = event { bytes.len() } else { 0 }).sum();
    assert!(reply_bytes <= regis::limits::MAX_REPLY_BYTES, "{reply_bytes} bytes of replies");
    assert!(events.contains(&Event::LimitReached));
}

#[test]
fn a_set_name_cannot_put_control_characters_in_a_reply() {
    // Issue 004.
    let events = events_of(b"L(A1'ok\rrm -rf /\n\x08\t''\"x')R(L)", false);
    assert_eq!(events, [Event::Reply(b"A'okrm -rf /'\r".to_vec())]);
}

#[test]
fn the_delay_one_call_can_ask_for_is_capped() {
    // Issue 006, and issue 009 which made the budget refill with input:
    // a minute saved up, and a tick for each byte fed.
    let stream = b"S(T32767)".repeat(500);
    let events = events_of(&stream, false);
    let ticks: u64 =
        events.iter().map(|event| if let Event::Wait(ticks) = event { u64::from(*ticks) } else { 0 }).sum();
    let allowed = u64::from(regis::limits::MAX_SAVED_WAIT_TICKS) + u64::try_from(stream.len()).unwrap();
    assert!(ticks <= allowed, "{ticks} ticks");
}

/// Feeds a stream in pieces and returns the screen's pixels and all events.
fn fed_in_pieces(stream: &[u8], piece: usize, macrograph_reports: bool) -> (Vec<u8>, Vec<Event>) {
    let mut terminal = common::terminal();
    terminal.allow_macrograph_reports(macrograph_reports);
    let mut screen = Screen::new();
    let mut events = Vec::new();
    for chunk in stream.chunks(piece) {
        events.extend(terminal.feed_regis(&mut screen, chunk));
    }
    events.extend(terminal.finish(&mut screen));
    (screen.displayed().pixels().to_vec(), events)
}

#[test]
fn arcs_of_no_radius_are_paid_for() {
    // Issue 007. Each `0` is a circle of no radius: 360 steps, one pixel.
    // A megabyte of invocations, fed as a network would deliver it.
    let mut stream = b"@:A".to_vec();
    stream.extend(std::iter::repeat_n(b'0', 16_000));
    stream.extend_from_slice(b"@;W(M0)C");
    stream.extend(b"@A".repeat(500_000));
    let started = Instant::now();
    let (_, events) = fed_in_pieces(&stream, 4096, false);
    assert!(events.contains(&Event::LimitReached));
    assert!(started.elapsed().as_secs() < 12, "unpaid arcs took {:?}", started.elapsed());
}

#[test]
fn the_picture_does_not_depend_on_chunking_even_at_the_work_limit() {
    // Issue 008. Two hundred screen-sized fills, each followed by padding
    // that earns allowance; far more work than the allowance saved up.
    let mut stream = Vec::new();
    for index in 0..200 {
        stream.extend(format!("W(I{})F(V[0,0][799,0][799,479][0,479])", index % 16).into_bytes());
        stream.extend(std::iter::repeat_n(b' ', 1500));
    }
    let (whole, _) = fed_in_pieces(&stream, stream.len(), false);
    for piece in [1000, 4096] {
        let (pieces, _) = fed_in_pieces(&stream, piece, false);
        assert_eq!(whole.iter().zip(&pieces).position(|(a, b)| a != b), None, "pieces of {piece}");
    }
}

#[test]
fn reply_and_delay_budgets_hold_however_the_input_is_divided() {
    // Issue 009.
    let mut stream = b"@:AR(P)R(P)R(P)R(P)R(P)R(P)R(P)R(P)@;P[-32768,-32768]".to_vec();
    stream.extend(b"@A".repeat(10_000));
    let (_, events) = fed_in_pieces(&stream, 2, false);
    let reply_bytes: usize =
        events.iter().map(|event| if let Event::Reply(bytes) = event { bytes.len() } else { 0 }).sum();
    assert!(reply_bytes <= stream.len() + regis::limits::MAX_REPLY_BYTES, "{reply_bytes} reply bytes");

    let stream = b"S(T3600)".repeat(1000);
    let (_, events) = fed_in_pieces(&stream, 8, false);
    let ticks: u64 =
        events.iter().map(|event| if let Event::Wait(ticks) = event { u64::from(*ticks) } else { 0 }).sum();
    let allowed = u64::from(regis::limits::MAX_SAVED_WAIT_TICKS) + u64::try_from(stream.len()).unwrap();
    assert!(ticks <= allowed, "{ticks} ticks");
}

#[test]
fn utf8_text_cannot_enter_or_leave_regis() {
    // Issue 010. U+0410 is D0 90 and U+041C is D0 9C.
    let mut terminal = common::terminal();
    let mut screen = Screen::new();
    let _ = terminal.feed(&mut screen, "plain text: \u{410}p P[10,10]V[700,400]".as_bytes());
    assert!(screen.displayed().pixels().iter().all(|pixel| *pixel == 0));
    let _ = terminal.feed(&mut screen, "\x1bPpT'\u{41c}'P[10,10]V[20,10]\x1b\\".as_bytes());
    assert_eq!(screen.displayed().pixel(15, 10), Some(7));
    // A caller that wants the 8-bit controls can ask for them.
    let mut terminal = common::terminal();
    terminal.recognise_eight_bit_controls(true);
    let mut screen = Screen::new();
    let _ = terminal.feed(&mut screen, b"\x90pP[1,1]V[]\x9c");
    assert_eq!(screen.displayed().pixel(1, 1), Some(7));
}

#[test]
fn a_macrograph_report_holds_printable_characters_only() {
    // Issue 011.
    let events = events_of(b"@:A\rrm -rf ~\r\x1b[2J\n@;R(M(A))", true);
    assert_eq!(events, [Event::Reply(b"@=Arm -rf ~[2J@;\r".to_vec())]);
}

#[test]
fn an_abandoned_macrograph_does_not_swallow_what_follows() {
    // Issue 012. The macrograph is one long string; the allowance runs out
    // part way through a later copy of it. What follows in the stream must
    // still be read as commands, not as the rest of that string.
    let mut stream = b"@:AT'".to_vec();
    stream.extend(std::iter::repeat_n(b'a', 15_000));
    stream.extend_from_slice(b"'@;");
    stream.extend(b"@A".repeat(40));
    stream.extend_from_slice(b" R(P)");
    let (_, events) = fed_in_pieces(&stream, stream.len(), false);
    assert!(events.contains(&Event::LimitReached));
    assert!(
        events.iter().any(|event| matches!(event, Event::Reply(_))),
        "the report after the macrograph was swallowed"
    );
}
