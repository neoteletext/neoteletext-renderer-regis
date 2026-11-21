//! Painting as a show in the renderer: the paint rate a drawing leaves
//! unset, the receiver's default paint rate (REQ-7025), and the report of
//! a watched drawing's painting finishing (REQ-7026), as DEC-7031 item 2
//! builds them, with the builder's readings in `docs/regis-v2.md`, "What
//! building changed", item 16.

use std::fmt::Write as _;

use regis::limits::{MAX_HELD_BYTES, MAX_MACROGRAPH_BYTES, MAX_PAINT_RATE, MAX_WATCHED_PAINTINGS};
use regis::{AnimationState, CommittedDrawing, Event, ForcedLook, Picture, Screen, Terminal, Validator};

const NONE: AnimationState =
    AnimationState { shown: None, position: None, layer: None, order: None, frame: 0, look: ForcedLook::None };

/// The colour every line of these tests is drawn in.
const INK: u8 = 3;

/// A terminal whose default paint rate is set before any byte is fed.
fn with_default(rate: u16) -> (Terminal, Screen) {
    let mut terminal = Terminal::new();
    terminal.set_default_paint_rate(rate);
    (terminal, Screen::new())
}

fn feed(terminal: &mut Terminal, screen: &mut Screen, stream: &str) -> Vec<Event> {
    terminal.feed_regis(screen, stream.as_bytes())
}

fn composed(terminal: &mut Terminal, screen: &Screen) -> Picture {
    let mut picture = Picture::new();
    terminal.compose(screen, &mut picture);
    picture
}

/// How many pixels of a row are inked.
fn run(picture: &Picture, row: u32) -> usize {
    (0..800).filter(|x| picture.canvas().pixel(*x, row) == Some(INK)).count()
}

/// The inked pixels of each row, as the terminal paints now.
fn rows<const N: usize>(terminal: &mut Terminal, screen: &Screen, rows: [u32; N]) -> [usize; N] {
    let picture = composed(terminal, screen);
    rows.map(|row| run(&picture, row))
}

/// The definition of drawing `number`: a line of `length` pixels along
/// `row`. Each pixel is one work unit, so at `T6` (600 units a second, 10
/// a tick) a line of 600 takes 60 ticks (DEC-6382 item 1).
fn line(number: u16, row: u32, length: u32) -> String {
    format!("D(B{number})W(I{INK})P[0,{row}]V[{},{row}]D(E)", length - 1)
}

/// The pixels and the pick record of a picture.
fn seen(picture: &Picture) -> (Vec<u8>, Vec<u16>) {
    (picture.canvas().pixels().to_vec(), picture.pick_record().to_vec())
}

fn committed_failure(events: &[Event]) -> bool {
    events.iter().any(|event| matches!(event, Event::CommitFailed { .. }))
}

const NOTHING: [u16; 0] = [];

// ---- REQ-7025: the default paint rate ----

#[test]
fn under_a_default_a_drawing_with_no_t_paints_at_it_one_with_t0_at_once_and_one_with_its_own_t_at_that() {
    let (mut terminal, mut screen) = with_default(6);
    let stream = format!("{}{}{}D(S2(T0),S3(T12),C)", line(1, 0, 600), line(2, 10, 600), line(3, 20, 600));
    assert_eq!(feed(&mut terminal, &mut screen, &stream), [Event::Committed(1)]);
    // T12 is 20 pixels a tick; the default 6 is 10.
    assert_eq!(rows(&mut terminal, &screen, [0, 10, 20]), [0, 600, 0]);
    assert!(terminal.advance_ticks(10));
    assert_eq!(rows(&mut terminal, &screen, [0, 10, 20]), [100, 600, 200]);
    assert!(terminal.advance_ticks(20));
    assert_eq!(rows(&mut terminal, &screen, [0, 10, 20]), [300, 600, 600]);
    assert!(terminal.advance_ticks(29));
    assert_eq!(rows(&mut terminal, &screen, [0, 10, 20]), [590, 600, 600]);
    assert!(!terminal.advance_ticks(1), "the 60th tick finishes the drawing at the default");
    assert_eq!(rows(&mut terminal, &screen, [0, 10, 20]), [600, 600, 600]);
}

#[test]
fn committed_drawing_gives_no_rate_t0_or_the_rate_the_stream_gave_and_a_validator_the_same() {
    let stream = "D(S1(X1),S2(T0),S3(T9),C)";
    let (mut terminal, mut screen) = with_default(6);
    let _ = feed(&mut terminal, &mut screen, stream);
    let mut validator = Validator::new();
    let _ = validator.feed_regis(stream.as_bytes());
    for (number, rate) in [(1, None), (2, Some(0)), (3, Some(9))] {
        // The default is the receiver's: it is not reported as the drawing's.
        let expected = Some(CommittedDrawing { shown: true, layer: 1, order: 0, rate });
        assert_eq!(terminal.committed_drawing(number), expected, "drawing {number}");
        assert_eq!(validator.committed_drawing(number), expected, "the validator's drawing {number}");
    }
    assert_eq!(terminal.default_paint_rate(), 6);
}

#[test]
fn a_drawing_deleted_and_made_again_has_no_rate_and_takes_the_default() {
    let (mut terminal, mut screen) = with_default(6);
    let _ = feed(&mut terminal, &mut screen, &format!("{}D(S1(T9),C)", line(1, 0, 600)));
    assert_eq!(terminal.committed_drawing(1).map(|drawing| drawing.rate), Some(Some(9)));
    let _ = feed(&mut terminal, &mut screen, "D(K1,C)");
    assert_eq!(terminal.committed_drawing(1), None);
    let _ = feed(&mut terminal, &mut screen, &format!("{}D(C)", line(1, 0, 600)));
    assert_eq!(terminal.committed_drawing(1).map(|drawing| drawing.rate), Some(None));
    // At the default's 10 pixels a tick, not T9's 15.
    assert!(terminal.advance_ticks(10));
    assert_eq!(rows(&mut terminal, &screen, [0]), [100]);
}

#[test]
fn children_wait_for_a_parent_painting_at_the_default() {
    let (mut terminal, mut screen) = with_default(6);
    // The parent has no T; child 2 has T0; child 3 has no T.
    let stream = format!("{}{}{}D(S2(P1,T0),S3(P1),C)", line(1, 0, 60), line(2, 10, 60), line(3, 20, 60));
    let _ = feed(&mut terminal, &mut screen, &stream);
    assert!(terminal.advance_ticks(5));
    assert_eq!(rows(&mut terminal, &screen, [0, 10, 20]), [50, 0, 0]);
    // The parent finishes at tick 6: the child with T0 is then whole, and
    // the child with no T starts at the default.
    assert!(terminal.advance_ticks(1));
    assert_eq!(rows(&mut terminal, &screen, [0, 10, 20]), [60, 60, 0]);
    assert!(terminal.advance_ticks(5));
    assert_eq!(rows(&mut terminal, &screen, [0, 10, 20]), [60, 60, 50]);
    assert!(!terminal.advance_ticks(1));
    assert_eq!(rows(&mut terminal, &screen, [0, 10, 20]), [60, 60, 60]);
}

#[test]
fn a_drawing_placed_as_a_source_a_look_or_a_frame_paints_at_the_placers_rate_in_force() {
    // Drawing 2 is hidden and has a rate of its own, T60 (100 pixels a
    // tick), which is not used: it is painted at drawing 1's rate in
    // force, here the default 6 (10 pixels a tick).
    for (placing, look) in [("F(2)", ForcedLook::None), ("R2", ForcedLook::None), ("N2", ForcedLook::Alternative)] {
        let (mut terminal, mut screen) = with_default(6);
        // Drawing 1's own contents, a line on row 100, are what the look,
        // the source or the frame replaces: they give it a cost when its
        // painting starts at the commit, before the look is forced.
        let stream = format!("{}{}D(S2(V0,T60),S1({placing}),C)", line(2, 0, 600), line(1, 100, 600));
        assert_eq!(feed(&mut terminal, &mut screen, &stream), [Event::Committed(1)], "{placing}");
        if look != ForcedLook::None {
            assert!(terminal.set_animation(1, AnimationState { look, ..NONE }));
        }
        assert!(terminal.advance_ticks(30), "{placing}");
        assert_eq!(rows(&mut terminal, &screen, [0, 100]), [300, 0], "{placing}");
        assert!(!terminal.advance_ticks(30), "{placing}");
        assert_eq!(rows(&mut terminal, &screen, [0]), [600], "{placing}");
    }
    // A placer with T0 paints what it places at once, whatever the
    // default and the placed drawing's own rate.
    for placed_rate in ["", ",T6"] {
        let (mut terminal, mut screen) = with_default(6);
        let stream = format!("{}D(S2(V0{placed_rate}),S1(R2,T0),C)", line(2, 0, 600));
        let _ = feed(&mut terminal, &mut screen, &stream);
        assert!(!terminal.advance_ticks(0));
        assert_eq!(rows(&mut terminal, &screen, [0]), [600], "placed drawing with {placed_rate:?}");
    }
}

/// Drawing 1 with no T and its child 2 with T6; drawing 3 with T6 and its
/// child 4 with no T: each a line of 60 pixels on its own row.
fn four_drawings() -> String {
    format!("{}{}{}{}D(S2(P1,T6),S3(T6),S4(P3),C)", line(1, 0, 60), line(2, 10, 60), line(3, 20, 60), line(4, 30, 60))
}

#[test]
fn changing_the_default_restarts_unset_drawings_and_rated_ones_below_them_but_not_others_with_their_own_t() {
    let (mut terminal, mut screen) = with_default(6);
    let _ = feed(&mut terminal, &mut screen, &four_drawings());
    assert!(!terminal.advance_ticks(1000));
    assert_eq!(rows(&mut terminal, &screen, [0, 10, 20, 30]), [60; 4]);
    terminal.set_default_paint_rate(12);
    assert_eq!(terminal.default_paint_rate(), 12);
    assert!(terminal.advance_ticks(0), "a drawing is painting again");
    // Drawing 1 starts again, and its child waits for it; drawing 3 has
    // its own T and is left whole, and its unset child starts again.
    assert_eq!(rows(&mut terminal, &screen, [0, 10, 20, 30]), [0, 0, 60, 0]);
    // At 12, 20 pixels a tick: drawings 1 and 4 take three ticks.
    assert!(terminal.advance_ticks(3));
    assert_eq!(rows(&mut terminal, &screen, [0, 10, 20, 30]), [60, 0, 60, 60]);
    assert!(terminal.advance_ticks(5));
    assert_eq!(rows(&mut terminal, &screen, [0, 10, 20, 30]), [60, 50, 60, 60]);
    assert!(!terminal.advance_ticks(1));
}

#[test]
fn setting_the_default_it_already_has_restarts_nothing() {
    let (mut terminal, mut screen) = with_default(6);
    let _ = feed(&mut terminal, &mut screen, &four_drawings());
    assert!(!terminal.advance_ticks(1000));
    terminal.set_default_paint_rate(6);
    assert!(!terminal.advance_ticks(0), "the same default started a painting");
    assert_eq!(rows(&mut terminal, &screen, [0, 10, 20, 30]), [60; 4]);
}

#[test]
fn a_default_of_0_makes_unset_drawings_whole_at_once_and_leaves_drawings_with_their_own_t_painting() {
    let (mut terminal, mut screen) = with_default(6);
    let _ = feed(&mut terminal, &mut screen, &format!("{}{}D(S2(T6),C)", line(1, 0, 600), line(2, 10, 600)));
    assert!(terminal.advance_ticks(30));
    assert_eq!(rows(&mut terminal, &screen, [0, 10]), [300, 300]);
    terminal.set_default_paint_rate(0);
    assert!(terminal.advance_ticks(0));
    assert_eq!(rows(&mut terminal, &screen, [0, 10]), [600, 300]);
    assert!(!terminal.advance_ticks(30));
    assert_eq!(rows(&mut terminal, &screen, [0, 10]), [600, 600]);
}

#[test]
fn a_default_above_32767_is_taken_as_32767() {
    let (mut terminal, mut screen) = with_default(40_000);
    assert_eq!(terminal.default_paint_rate(), MAX_PAINT_RATE);
    let _ = feed(&mut terminal, &mut screen, &format!("{}D(C)", line(1, 0, 600)));
    // T32767 is 54,611 units a tick: one tick paints the line.
    assert!(!terminal.advance_ticks(1));
    assert_eq!(rows(&mut terminal, &screen, [0]), [600]);
    // The same rate, however it is asked for, restarts nothing.
    for same in [MAX_PAINT_RATE, MAX_PAINT_RATE + 1, u16::MAX] {
        terminal.set_default_paint_rate(same);
        assert_eq!(terminal.default_paint_rate(), MAX_PAINT_RATE);
        assert!(!terminal.advance_ticks(0), "setting {same} started a painting");
    }
}

/// A scene of every kind: drawings with no T, T0 and T3, children, a
/// fill, a source and a frame.
fn mixed_scene() -> String {
    let mut stream = String::new();
    stream.push_str(&line(1, 0, 600));
    stream.push_str(&line(2, 10, 300));
    stream.push_str(&line(3, 20, 400));
    stream.push_str("D(B4)W(I5)F(V[100,100][300,100][300,200][100,200])D(E)");
    stream.push_str("D(B5)W(I6)P[400,300]C[+60]D(E)");
    stream.push_str(&line(6, 40, 500));
    stream.push_str("D(S2(T0,P1),S3(T3),S4(P3),S5(Y20),S6(V0),S7(R6,Y30),S8(F(6,5),X5),C)");
    stream
}

#[test]
fn the_final_picture_under_a_default_is_the_picture_painted_at_once() {
    let (mut gradual, mut gradual_screen) = with_default(7);
    let _ = feed(&mut gradual, &mut gradual_screen, &mixed_scene());
    let (mut at_once, mut at_once_screen) = with_default(0);
    let _ = feed(&mut at_once, &mut at_once_screen, &mixed_scene());
    // Drawing 3 has its own T3, which the default leaves as it is.
    let _ = at_once.advance_ticks(u32::MAX);
    let whole = composed(&mut at_once, &at_once_screen);
    assert!(gradual.advance_ticks(5));
    assert!(seen(&composed(&mut gradual, &gradual_screen)) != seen(&whole), "the default painted nothing gradually");
    let mut kept = Picture::new();
    while gradual.advance_ticks(7) {
        gradual.compose(&gradual_screen, &mut kept);
    }
    gradual.compose(&gradual_screen, &mut kept);
    assert!(seen(&kept) == seen(&whole), "the finished picture is not the picture painted at once");
    assert!(seen(&composed(&mut gradual, &gradual_screen)) == seen(&whole), "a whole repaint differs");
}

#[test]
fn with_no_default_drawings_with_no_t_appear_at_once_as_with_t0() {
    let mut terminal = Terminal::new();
    assert_eq!(terminal.default_paint_rate(), 0);
    let mut screen = Screen::new();
    let stream = mixed_scene();
    let _ = feed(&mut terminal, &mut screen, &stream);
    assert!(terminal.advance_ticks(0), "the drawing with T3 paints");
    // Every drawing but 3 and its child 4 is whole before any tick:
    // drawing 6 is hidden, and drawn as drawing 7's source on row 70 and
    // as drawing 8's frame on row 40, from x 5.
    assert_eq!(rows(&mut terminal, &screen, [0, 10, 20, 40, 70]), [600, 300, 0, 500, 500]);
    // The same stream with T0 given to every drawing with none.
    let explicit = stream.replace(
        "S2(T0,P1),S3(T3),S4(P3),S5(Y20),S6(V0),S7(R6,Y30),S8(F(6,5),X5)",
        "S1(T0),S2(T0,P1),S3(T3),S4(P3,T0),S5(Y20,T0),S6(V0,T0),S7(R6,Y30,T0),S8(F(6,5),X5,T0)",
    );
    assert_ne!(explicit, stream);
    let mut other = Terminal::new();
    let mut other_screen = Screen::new();
    let _ = feed(&mut other, &mut other_screen, &explicit);
    for ticks in [0, 3, 10, 100, 1000] {
        let _ = terminal.advance_ticks(ticks);
        let _ = other.advance_ticks(ticks);
        // Compared without printing two pictures on a failure.
        let same = seen(&composed(&mut terminal, &screen)) == seen(&composed(&mut other, &other_screen));
        assert!(same, "the pictures differ after {ticks} more ticks");
    }
}

// ---- REQ-7026: the painted report ----

#[test]
fn a_watched_drawing_is_reported_once_in_the_call_in_which_its_painting_ends() {
    let (mut terminal, mut screen) = with_default(0);
    assert!(terminal.watch_painted(&[1]));
    let _ = feed(&mut terminal, &mut screen, &format!("{}D(S1(T6),C)", line(1, 0, 600)));
    assert_eq!(terminal.take_painted(), NOTHING);
    for tick in 1..60 {
        assert!(terminal.advance_ticks(1));
        let _ = composed(&mut terminal, &screen);
        assert_eq!(terminal.take_painted(), NOTHING, "reported at tick {tick}, before its painting ended");
    }
    assert!(!terminal.advance_ticks(1));
    assert_eq!(terminal.take_painted(), [1]);
    assert_eq!(terminal.take_painted(), NOTHING, "reported twice");
    assert!(!terminal.advance_ticks(100));
    assert_eq!(terminal.take_painted(), NOTHING, "reported twice");
}

#[test]
fn a_drawing_painted_at_once_is_reported_by_the_commit_that_makes_it() {
    for (default, rate) in [(0, ""), (0, "T0"), (6, "T0")] {
        let (mut terminal, mut screen) = with_default(default);
        assert!(terminal.watch_painted(&[1]));
        let _ = feed(&mut terminal, &mut screen, &format!("{}D(S1({rate}),C)", line(1, 0, 600)));
        assert_eq!(terminal.take_painted(), [1], "default {default}, rate {rate:?}");
        assert!(!terminal.advance_ticks(10));
        assert_eq!(terminal.take_painted(), NOTHING);
    }
}

#[test]
fn a_redefined_drawing_is_reported_again() {
    let (mut terminal, mut screen) = with_default(0);
    assert!(terminal.watch_painted(&[1]));
    let _ = feed(&mut terminal, &mut screen, &format!("{}D(C)", line(1, 0, 600)));
    assert_eq!(terminal.take_painted(), [1]);
    // Moving it does not redefine it.
    let _ = feed(&mut terminal, &mut screen, "D(S1(X5,L2),C)");
    assert_eq!(terminal.take_painted(), NOTHING);
    let _ = feed(&mut terminal, &mut screen, &format!("{}D(C)", line(1, 0, 300)));
    assert_eq!(terminal.take_painted(), [1]);
}

#[test]
fn a_change_of_rate_reports_again_and_the_same_rate_does_not() {
    let (mut terminal, mut screen) = with_default(0);
    assert!(terminal.watch_painted(&[1]));
    let _ = feed(&mut terminal, &mut screen, &format!("{}D(S1(T6),C)", line(1, 0, 600)));
    let _ = terminal.advance_ticks(60);
    assert_eq!(terminal.take_painted(), [1]);
    let _ = feed(&mut terminal, &mut screen, "D(S1(T6),C)");
    let _ = terminal.advance_ticks(100);
    assert_eq!(terminal.take_painted(), NOTHING, "the same rate reported again");
    // T12: 30 ticks.
    let _ = feed(&mut terminal, &mut screen, "D(S1(T12),C)");
    assert_eq!(terminal.take_painted(), NOTHING);
    assert!(terminal.advance_ticks(29));
    assert_eq!(terminal.take_painted(), NOTHING);
    assert!(!terminal.advance_ticks(1));
    assert_eq!(terminal.take_painted(), [1]);
    // To T0: whole, and reported, at the commit.
    let _ = feed(&mut terminal, &mut screen, "D(S1(T0),C)");
    assert_eq!(terminal.take_painted(), [1]);
}

#[test]
fn hidden_then_shown_by_the_stream_is_reported_again() {
    let (mut terminal, mut screen) = with_default(0);
    assert!(terminal.watch_painted(&[1, 2]));
    let _ = feed(&mut terminal, &mut screen, &format!("{}{}D(S2(T6),C)", line(1, 0, 600), line(2, 10, 600)));
    let _ = terminal.advance_ticks(60);
    assert_eq!(terminal.take_painted(), [1, 2]);
    let _ = feed(&mut terminal, &mut screen, "D(S1(V0),S2(V0),C)");
    assert_eq!(terminal.take_painted(), NOTHING);
    let _ = feed(&mut terminal, &mut screen, "D(S1(V1),S2(V1),C)");
    // Drawing 1, at once, at the commit; drawing 2 when painted again.
    assert_eq!(terminal.take_painted(), [1]);
    assert!(terminal.advance_ticks(59));
    assert_eq!(terminal.take_painted(), NOTHING);
    assert!(!terminal.advance_ticks(1));
    assert_eq!(terminal.take_painted(), [2]);
}

#[test]
fn hidden_then_shown_by_the_animation_state_is_reported_again() {
    let (mut terminal, mut screen) = with_default(0);
    assert!(terminal.watch_painted(&[1, 2]));
    let _ = feed(&mut terminal, &mut screen, &format!("{}{}D(S2(T6),C)", line(1, 0, 600), line(2, 10, 600)));
    let _ = terminal.advance_ticks(60);
    assert_eq!(terminal.take_painted(), [1, 2]);
    let hidden = AnimationState { shown: Some(false), ..NONE };
    assert!(terminal.set_animation(1, hidden));
    assert!(terminal.set_animation(2, hidden));
    assert_eq!(terminal.take_painted(), NOTHING);
    // Shown by set_animation, then by clear_animation.
    assert!(terminal.set_animation(1, AnimationState { shown: Some(true), ..NONE }));
    assert_eq!(terminal.take_painted(), [1]);
    terminal.clear_animation(2);
    assert_eq!(terminal.take_painted(), NOTHING);
    assert!(!terminal.advance_ticks(60));
    assert_eq!(terminal.take_painted(), [2]);
    // A state that leaves it shown opens nothing.
    assert!(terminal.set_animation(1, AnimationState { position: Some((4, 4)), ..NONE }));
    terminal.clear_animation(1);
    assert_eq!(terminal.take_painted(), NOTHING);
}

#[test]
fn showing_an_ancestor_by_the_animation_state_reports_its_descendants_again() {
    let (mut terminal, mut screen) = with_default(0);
    assert!(terminal.watch_painted(&[3]));
    let stream = format!("{}{}{}D(S2(P1),S3(P2),C)", line(1, 0, 60), line(2, 10, 60), line(3, 20, 60));
    let _ = feed(&mut terminal, &mut screen, &stream);
    assert_eq!(terminal.take_painted(), [3]);
    assert!(terminal.set_animation(1, AnimationState { shown: Some(false), ..NONE }));
    assert_eq!(terminal.take_painted(), NOTHING);
    terminal.clear_animation(1);
    assert_eq!(terminal.take_painted(), [3]);
}

#[test]
fn a_change_of_default_reports_again_the_drawings_whose_rate_in_force_it_changes() {
    let (mut terminal, mut screen) = with_default(6);
    assert!(terminal.watch_painted(&[1, 2, 3]));
    let stream = format!("{}{}{}D(S2(T6),S3(T0),C)", line(1, 0, 600), line(2, 10, 600), line(3, 20, 600));
    let _ = feed(&mut terminal, &mut screen, &stream);
    assert_eq!(terminal.take_painted(), [3]);
    let _ = terminal.advance_ticks(60);
    assert_eq!(terminal.take_painted(), [1, 2]);
    terminal.set_default_paint_rate(12);
    assert_eq!(terminal.take_painted(), NOTHING);
    assert!(terminal.advance_ticks(29));
    assert_eq!(terminal.take_painted(), NOTHING);
    assert!(!terminal.advance_ticks(1));
    // Drawings 2 and 3 have their own T: not reported again.
    assert_eq!(terminal.take_painted(), [1]);
    terminal.set_default_paint_rate(12);
    let _ = terminal.advance_ticks(100);
    assert_eq!(terminal.take_painted(), NOTHING);
    // To 0: whole, and reported, by the call.
    terminal.set_default_paint_rate(0);
    assert_eq!(terminal.take_painted(), [1]);
}

#[test]
fn a_parent_starting_to_paint_reports_its_children_again_when_it_and_they_finish() {
    let (mut terminal, mut screen) = with_default(0);
    assert!(terminal.watch_painted(&[1, 2, 3]));
    // Child 2 at once, child 3 at T12 (a line of 60, three ticks).
    let stream = format!("{}{}{}D(S1(T6),S2(P1),S3(P1,T12),C)", line(1, 0, 600), line(2, 10, 60), line(3, 20, 60));
    let _ = feed(&mut terminal, &mut screen, &stream);
    let _ = terminal.advance_ticks(1000);
    assert_eq!(terminal.take_painted(), [1, 2, 3]);
    let _ = feed(&mut terminal, &mut screen, &format!("{}D(C)", line(1, 0, 600)));
    assert_eq!(terminal.take_painted(), NOTHING);
    assert!(terminal.advance_ticks(59));
    assert_eq!(terminal.take_painted(), NOTHING);
    assert!(terminal.advance_ticks(1));
    assert_eq!(terminal.take_painted(), [1, 2]);
    assert!(terminal.advance_ticks(2));
    assert_eq!(terminal.take_painted(), NOTHING);
    assert!(!terminal.advance_ticks(1));
    assert_eq!(terminal.take_painted(), [3]);
}

#[test]
fn a_child_is_not_reported_while_its_parent_paints() {
    let (mut terminal, mut screen) = with_default(0);
    assert!(terminal.watch_painted(&[2]));
    let stream = format!("{}{}D(S1(T6),S2(P1,T0),C)", line(1, 0, 600), line(2, 10, 60));
    let _ = feed(&mut terminal, &mut screen, &stream);
    assert_eq!(terminal.take_painted(), NOTHING);
    for tick in 1..60 {
        let _ = terminal.advance_ticks(1);
        assert_eq!(terminal.take_painted(), NOTHING, "the child was reported at tick {tick}");
    }
    let _ = terminal.advance_ticks(1);
    assert_eq!(terminal.take_painted(), [2]);
}

#[test]
fn a_hidden_drawing_waits_to_be_reported_until_it_is_shown() {
    let (mut terminal, mut screen) = with_default(0);
    assert!(terminal.watch_painted(&[1, 3]));
    let stream = format!("{}{}{}D(S1(V0),S2(V0),S3(P2),C)", line(1, 0, 60), line(2, 10, 60), line(3, 20, 60));
    let _ = feed(&mut terminal, &mut screen, &stream);
    assert_eq!(terminal.take_painted(), NOTHING);
    assert!(!terminal.advance_ticks(100));
    assert_eq!(terminal.take_painted(), NOTHING);
    let _ = feed(&mut terminal, &mut screen, "D(S1(V1),C)");
    assert_eq!(terminal.take_painted(), [1]);
    // Drawing 3 is shown but its parent is not.
    let _ = feed(&mut terminal, &mut screen, "D(S2(V1),C)");
    assert_eq!(terminal.take_painted(), [3]);
}

#[test]
fn a_deleted_drawings_open_episode_is_dropped() {
    let (mut terminal, mut screen) = with_default(0);
    assert!(terminal.watch_painted(&[1]));
    let _ = feed(&mut terminal, &mut screen, &format!("{}D(S1(T6),C)", line(1, 0, 600)));
    assert!(terminal.advance_ticks(30));
    let _ = feed(&mut terminal, &mut screen, "D(K1,C)");
    assert_eq!(terminal.take_painted(), NOTHING);
    let _ = terminal.advance_ticks(100);
    assert_eq!(terminal.take_painted(), NOTHING);
}

#[test]
fn a_drawing_that_became_whole_and_was_deleted_before_the_receiver_asked_is_still_reported() {
    // "What building changed" item 16.3.
    let (mut terminal, mut screen) = with_default(0);
    assert!(terminal.watch_painted(&[1]));
    let _ = feed(&mut terminal, &mut screen, &format!("{}D(C)", line(1, 0, 600)));
    let _ = feed(&mut terminal, &mut screen, "D(K1,C)");
    assert_eq!(terminal.take_painted(), [1]);
}

#[test]
fn a_drawing_deleted_then_made_again_in_a_later_commit_is_reported_again() {
    let (mut terminal, mut screen) = with_default(0);
    assert!(terminal.watch_painted(&[1]));
    let _ = feed(&mut terminal, &mut screen, &format!("{}D(C)", line(1, 0, 600)));
    assert_eq!(terminal.take_painted(), [1]);
    let _ = feed(&mut terminal, &mut screen, "D(K1,C)");
    assert_eq!(terminal.take_painted(), NOTHING);
    let _ = feed(&mut terminal, &mut screen, &format!("{}D(C)", line(1, 0, 600)));
    assert_eq!(terminal.take_painted(), [1]);
}

#[test]
fn a_failed_commit_reports_nothing() {
    let (mut terminal, mut screen) = with_default(0);
    assert!(terminal.watch_painted(&[1]));
    // The parent named does not exist: the batch fails.
    let events = feed(&mut terminal, &mut screen, &format!("{}D(S1(P99),C)", line(1, 0, 600)));
    assert!(committed_failure(&events), "{events:?}");
    assert_eq!(terminal.committed_drawing(1), None);
    assert_eq!(terminal.take_painted(), NOTHING);
    let _ = feed(&mut terminal, &mut screen, &format!("{}D(C)", line(1, 0, 600)));
    assert_eq!(terminal.take_painted(), [1]);
    // Redefined, given a rate and hidden then shown, all in failed batches.
    for failing in
        [format!("{}D(S1(P99),C)", line(1, 0, 300)), "D(S1(T6,P99),C)".into(), "D(S1(V0),S1(V1),S1(P99),C)".into()]
    {
        let events = feed(&mut terminal, &mut screen, &failing);
        assert!(committed_failure(&events), "{failing}: {events:?}");
        assert!(!terminal.advance_ticks(100));
        assert_eq!(terminal.take_painted(), NOTHING, "{failing}");
    }
}

/// The empty drawings 1 to `last`, made at once in one commit.
fn empty_drawings(last: u16) -> String {
    let mut stream = String::from("D(");
    for number in 1..=last {
        write!(stream, "S{number}(X1),").unwrap();
    }
    stream.push_str("C)");
    stream
}

#[test]
fn thirty_three_distinct_numbers_are_refused_with_the_old_set_kept_and_duplicates_count_once() {
    assert_eq!(MAX_WATCHED_PAINTINGS, 32);
    let (mut terminal, mut screen) = with_default(0);
    assert!(terminal.watch_painted(&[40]));
    let thirty_three: Vec<u16> = (1..=33).collect();
    assert!(!terminal.watch_painted(&thirty_three));
    // With the duplicates, 32 distinct numbers in 96 entries.
    let mut duplicated: Vec<u16> = (1..=32).chain((1..=32).rev()).chain(1..=32).collect();
    duplicated.push(33);
    assert!(!terminal.watch_painted(&duplicated), "33 distinct numbers among duplicates were accepted");
    let _ = feed(&mut terminal, &mut screen, &empty_drawings(40));
    assert_eq!(terminal.take_painted(), [40], "the refused set took the place of the old one");
    duplicated.pop();
    assert!(terminal.watch_painted(&duplicated));
    let _ = feed(&mut terminal, &mut screen, &format!("D(K40,C){}", empty_drawings(40)));
    let expected: Vec<u16> = (1..=32).collect();
    assert_eq!(terminal.take_painted(), expected);
}

#[test]
fn watching_a_drawing_again_keeps_its_episode_and_its_report_not_yet_taken() {
    let (mut terminal, mut screen) = with_default(0);
    assert!(terminal.watch_painted(&[1, 3]));
    let stream = format!("{}{}D(S1(T6),C)", line(1, 0, 600), line(3, 10, 60));
    let _ = feed(&mut terminal, &mut screen, &stream);
    // Drawing 3 is whole and not yet taken; drawing 1 is half painted.
    assert!(terminal.advance_ticks(30));
    assert!(terminal.watch_painted(&[3, 2, 1]));
    assert_eq!(terminal.take_painted(), [3]);
    assert!(terminal.advance_ticks(29));
    assert_eq!(terminal.take_painted(), NOTHING);
    assert!(!terminal.advance_ticks(1));
    assert_eq!(terminal.take_painted(), [1]);
    // Watched again once reported, they are not reported again.
    assert!(terminal.watch_painted(&[1, 3]));
    let _ = terminal.advance_ticks(1);
    assert_eq!(terminal.take_painted(), NOTHING);
}

#[test]
fn no_longer_watching_a_drawing_drops_its_report_and_watching_it_anew_reports_it_at_the_next_call() {
    let (mut terminal, mut screen) = with_default(0);
    assert!(terminal.watch_painted(&[1]));
    let _ = feed(&mut terminal, &mut screen, &format!("{}D(C)", line(1, 0, 600)));
    assert!(terminal.watch_painted(&[]));
    assert_eq!(terminal.take_painted(), NOTHING);
    // "What building changed" item 16.1: newly watched and already whole,
    // it is reported at the next of the five calls, not by the naming.
    assert!(terminal.watch_painted(&[1]));
    assert_eq!(terminal.take_painted(), NOTHING);
    let _ = terminal.advance_ticks(0);
    assert_eq!(terminal.take_painted(), [1]);
}

#[test]
fn reports_are_in_ascending_order_of_number() {
    let (mut terminal, mut screen) = with_default(0);
    assert!(terminal.watch_painted(&[9, 3, 7]));
    let _ = feed(&mut terminal, &mut screen, "D(S9(X1),S7(X1),S3(X1),C)");
    assert_eq!(terminal.take_painted(), [3, 7, 9]);
    // Finishing in the same tick, made in the other order.
    let stream = format!("{}{}{}D(S9(T6),S7(T6),S3(T6),C)", line(9, 0, 60), line(7, 10, 60), line(3, 20, 60));
    let _ = feed(&mut terminal, &mut screen, &stream);
    let _ = terminal.advance_ticks(6);
    assert_eq!(terminal.take_painted(), [3, 7, 9]);
}

#[test]
fn watching_drawing_0_and_numbers_that_do_not_exist_is_harmless() {
    let (mut terminal, mut screen) = with_default(0);
    assert!(terminal.watch_painted(&[0, u16::MAX, 1, 500]));
    assert_eq!(terminal.take_painted(), NOTHING);
    let _ = terminal.advance_ticks(10);
    assert_eq!(terminal.take_painted(), NOTHING);
    // With T0, so that the change of default below does not report it again.
    let _ = feed(&mut terminal, &mut screen, &format!("{}D(S1(T0),C)", line(1, 0, 60)));
    assert_eq!(terminal.take_painted(), [1]);
    assert!(!terminal.set_animation(500, AnimationState { shown: Some(true), ..NONE }));
    terminal.clear_animation(0);
    terminal.set_default_paint_rate(5);
    let _ = terminal.advance_ticks(u32::MAX);
    assert_eq!(terminal.take_painted(), NOTHING);
}

#[test]
fn a_terminal_watching_32_drawings_holds_no_more_than_the_bound_and_watching_none_holds_nothing_for_them() {
    let (mut terminal, mut screen) = with_default(1);
    // The heaviest state of `memory.rs`, every drawing rated by the
    // default so that each has painting state, and 64 animation states.
    let macrograph = format!("@:A{}@;", "V".repeat(MAX_MACROGRAPH_BYTES));
    let _ = feed(&mut terminal, &mut screen, &macrograph);
    let half = regis::limits::MAX_DRAWING_CONTENT_BYTES / 2;
    // Drawing 1 hidden, so that painting it is not the limit.
    let mut all = format!("D(B1){}D(E)D(S1(V0),", "V[]".repeat(half / 3));
    for number in 2..=regis::limits::MOST_DRAWINGS_HELD {
        write!(all, "S{number},").unwrap();
    }
    all.push_str("C)");
    assert_eq!(feed(&mut terminal, &mut screen, &all), [Event::Committed(1)]);
    for number in 1..=64 {
        assert!(terminal.set_animation(number, AnimationState { position: Some((1, 1)), ..NONE }));
    }
    let unwatched = terminal.held_bytes();
    let watched: Vec<u16> = (1..=32).collect();
    assert!(terminal.watch_painted(&watched));
    let held = terminal.held_bytes();
    assert!(held > unwatched, "the watched drawings are not counted");
    assert!(held <= MAX_HELD_BYTES, "{held} held, past the bound {MAX_HELD_BYTES}");
    let _ = terminal.advance_ticks(1);
    assert!(terminal.held_bytes() <= MAX_HELD_BYTES);
    assert!(terminal.watch_painted(&[]));
    let _ = terminal.take_painted();
    assert!(terminal.held_bytes() <= unwatched, "watching nothing still holds memory for it");
}

#[test]
fn a_drawing_starting_at_the_default_is_reported_at_once_when_the_default_becomes_0_before_any_tick() {
    // The builder's case of the starting set: drawing 1 is noted as
    // starting by the commit, and a drawing with its own T keeps the
    // scene rated, so the note stands when the default goes to 0; with
    // no rate in force left, drawing 1 is whole and reported by the call.
    let (mut terminal, mut screen) = with_default(6);
    assert!(terminal.watch_painted(&[1, 2]));
    let _ = feed(&mut terminal, &mut screen, &format!("{}{}D(S2(T6),C)", line(1, 0, 600), line(2, 10, 600)));
    assert_eq!(terminal.take_painted(), NOTHING);
    terminal.set_default_paint_rate(0);
    assert_eq!(terminal.take_painted(), [1]);
    assert_eq!(rows(&mut terminal, &screen, [0, 10]), [600, 0]);
    let _ = terminal.advance_ticks(60);
    assert_eq!(terminal.take_painted(), [2]);
}
