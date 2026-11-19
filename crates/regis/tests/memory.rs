//! What a stream can make the renderer hold, and the figure it reports
//! (`SOLVEDSECURITYISSUES` 015; `docs/regis-v2.md`, "Lifetime and
//! memory"). The resident size the same streams cause is checked in
//! `resident.rs`, alone in its process.

use std::fmt::Write as _;

use regis::limits::{
    DRAWING_EXTRAS_BYTES, DRAWING_RECORD_BYTES, MAX_DRAWING_CONTENT_BYTES, MAX_DRAWING_RECORD_BYTES, MAX_HELD_BYTES,
    MAX_MACROGRAPH_BYTES, MOST_DRAWINGS_HELD,
};
use regis::{CommitError, Event, Screen, Terminal, Validator};

/// `D(S1,S2,...,Sn` with each drawing given `attributes`, unclosed.
fn drawings(first: usize, last: usize, attributes: &str) -> String {
    let mut stream = String::from("D(");
    for number in first..=last {
        write!(stream, "S{number}{attributes},").unwrap();
    }
    stream
}

fn failure(events: &[Event]) -> Option<(CommitError, u16)> {
    events.iter().find_map(|event| match event {
        Event::CommitFailed { error, drawing } => Some((*error, *drawing)),
        _ => None,
    })
}

fn feed(validator: &mut Validator, stream: &str) -> Vec<Event> {
    validator.feed_regis(stream.as_bytes())
}

#[test]
fn the_issue_stream_of_65535_empty_drawings_is_refused_at_the_cap_and_held_within_the_bound() {
    let mut validator = Validator::new();
    let stream = drawings(1, 65_535, "");
    // Before its commit the staged scene holds what the cap allows.
    assert_eq!(feed(&mut validator, &stream), []);
    let held = validator.held_bytes();
    assert!(held <= MAX_HELD_BYTES, "{held} held, past the bound {MAX_HELD_BYTES}");
    assert!(held >= MOST_DRAWINGS_HELD * DRAWING_RECORD_BYTES, "{held} held does not count the records");
    let events = feed(&mut validator, "C)");
    let next = u16::try_from(MOST_DRAWINGS_HELD + 1).unwrap();
    assert_eq!(failure(&events), Some((CommitError::Drawings, next)));
    assert_eq!(CommitError::Drawings.code(), 8);
    assert_eq!(validator.commits(), 0);
    // The failed batch is dropped, and its memory with it.
    let after = validator.held_bytes();
    assert!(after < 64 * 1024, "{after} still held after the failed commit");
}

#[test]
fn as_many_drawings_as_the_cap_pays_for_commit_and_one_more_does_not() {
    let mut validator = Validator::new();
    let events = feed(&mut validator, &format!("{}C)", drawings(1, MOST_DRAWINGS_HELD, "")));
    assert_eq!(events, [Event::Committed(1)]);
    let held = validator.held_bytes();
    // The committed and the staged scene each hold every record.
    assert!(held >= 2 * MOST_DRAWINGS_HELD * DRAWING_RECORD_BYTES, "{held}");
    assert!(held <= MAX_HELD_BYTES, "{held}");
    let one_more = MOST_DRAWINGS_HELD + 1;
    let events = feed(&mut validator, &format!("D(S{one_more},C)"));
    assert_eq!(failure(&events), Some((CommitError::Drawings, u16::try_from(one_more).unwrap())));
    assert_eq!(validator.commits(), 1);
}

#[test]
fn deleting_drawings_gives_their_records_back() {
    let mut validator = Validator::new();
    let all = format!("{}C)", drawings(1, MOST_DRAWINGS_HELD, ""));
    assert_eq!(feed(&mut validator, &all), [Event::Committed(1)]);
    assert_eq!(feed(&mut validator, "D(Z,C)"), [Event::Committed(2)]);
    let others = format!("{}C)", drawings(30_001, 30_000 + MOST_DRAWINGS_HELD, ""));
    assert_eq!(feed(&mut validator, &others), [Event::Committed(3)]);
    let one = MOST_DRAWINGS_HELD + 30_000;
    assert_eq!(feed(&mut validator, &format!("D(K{one},S1,C)")), [Event::Committed(4)]);
}

#[test]
fn a_label_or_a_frame_list_is_charged_once_given() {
    let fits = MAX_DRAWING_RECORD_BYTES / (DRAWING_RECORD_BYTES + DRAWING_EXTRAS_BYTES);
    let mut validator = Validator::new();
    let events = feed(&mut validator, &format!("{}C)", drawings(1, fits, "(U'a',F(65000))")));
    assert_eq!(events, [Event::Committed(1)]);
    assert_eq!(validator.label(fits.try_into().unwrap()).as_deref(), Some("a"));
    let mut validator = Validator::new();
    let events = feed(&mut validator, &format!("{}C)", drawings(1, fits + 1, "(U'a')")));
    assert_eq!(failure(&events), Some((CommitError::Drawings, u16::try_from(fits + 1).unwrap())));
    // A label removed gives its memory back.
    let mut validator = Validator::new();
    assert_eq!(feed(&mut validator, "D(S1(U'a'),C)"), [Event::Committed(1)]);
    let labelled = validator.held_bytes();
    assert_eq!(feed(&mut validator, "D(S1(U''),C)"), [Event::Committed(2)]);
    assert_eq!(validator.label(1), None);
    assert!(validator.held_bytes() + 2 * DRAWING_EXTRAS_BYTES <= labelled);
}

#[test]
fn contents_are_counted_once_while_both_scenes_share_them() {
    const CONTENTS: usize = 100_000;
    let mut validator = Validator::new();
    let empty = validator.held_bytes();
    let body = "V[]".repeat(CONTENTS / 3);
    let events = feed(&mut validator, &format!("D(B1){body}D(E)D(C)"));
    assert_eq!(events, [Event::Committed(1)]);
    let shared = validator.held_bytes() - empty;
    assert!((CONTENTS - 3..CONTENTS + 4 * DRAWING_RECORD_BYTES).contains(&shared), "{shared}");
    // Defined again and not yet committed: the old and the new are held.
    assert_eq!(feed(&mut validator, &format!("D(B1){body}D(E)")), []);
    let both = validator.held_bytes() - empty;
    assert!(both >= 2 * (CONTENTS - 3), "{both}");
}

#[test]
fn the_heaviest_state_a_stream_can_make_stays_within_the_bound() {
    let mut validator = Validator::new();
    // Every macrograph byte used.
    let macrograph = format!("@:A{}@;", "V".repeat(MAX_MACROGRAPH_BYTES));
    assert_eq!(feed(&mut validator, &macrograph), []);
    // Every record paid for, and committed with half the contents cap.
    let half = MAX_DRAWING_CONTENT_BYTES / 2;
    let contents = format!("D(B1){}D(E)", "V[]".repeat(half / 3));
    // Hidden, so that painting them is not the limit.
    let all = format!("{contents}D(S1(V0),S2(V0),{}C)", &drawings(3, MOST_DRAWINGS_HELD, "")[2..]);
    assert_eq!(feed(&mut validator, &all), [Event::Committed(1)]);
    // Then defined again and left open, with a definition being received
    // up to the cap, a polygon fill open and a string open.
    let mut open = format!("D(B1){}D(E)D(B2){}D(E)", "V[]".repeat(half / 3), "V[]".repeat(half / 3));
    open.push_str("D(B3)");
    open.push_str(&"V[]".repeat(MAX_DRAWING_CONTENT_BYTES / 3));
    let _ = feed(&mut validator, &open);
    let held = validator.held_bytes();
    assert!(held <= MAX_HELD_BYTES, "{held} held, past the bound {MAX_HELD_BYTES}");
    assert!(held >= 2 * MOST_DRAWINGS_HELD * DRAWING_RECORD_BYTES + 3 * half, "{held}");
    let mut fill = String::from("D(E)D(C)F(V");
    for vertex in 0..5000 {
        write!(fill, "[{},{}]", vertex % 800, vertex % 480).unwrap();
    }
    fill.push_str("T'");
    fill.push_str(&"x".repeat(2000));
    let _ = feed(&mut validator, &fill);
    let held = validator.held_bytes();
    assert!(held <= MAX_HELD_BYTES, "{held} held, past the bound {MAX_HELD_BYTES}");
}

#[test]
fn a_terminal_reports_what_a_validator_does_but_its_absent_screen() {
    let stream = format!("{}C)", drawings(1, 1000, "(X1)"));
    let mut validator = Validator::new();
    let _ = feed(&mut validator, &stream);
    let mut terminal = Terminal::new();
    let mut screen = Screen::new();
    let _ = terminal.feed_regis(&mut screen, stream.as_bytes());
    assert_eq!(validator.held_bytes() - terminal.held_bytes(), size_of::<Screen>());
}
