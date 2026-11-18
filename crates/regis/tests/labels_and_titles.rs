//! ReGIS v2: a numbered drawing's label, the attribute `U`, and a picture's
//! title, `D(T'...')` (`docs/regis-v2.md`, "Attributes", "Title" and
//! "Grammar"; DEC-6638 items 3 and 6). Neither is ever drawn.

use regis::limits::{MAX_LABEL_BYTES, MAX_TITLE_BYTES};
use regis::{Event, Picture, Screen, Terminal, Validator};

/// A terminal fed `input` as ReGIS commands, the events, and the picture
/// composed afterwards.
fn run(input: &[u8]) -> (Terminal, Vec<Event>, Picture) {
    let mut terminal = Terminal::new();
    let mut screen = Screen::new();
    let mut events = terminal.feed_regis(&mut screen, input);
    events.extend(terminal.finish(&mut screen));
    let mut picture = Picture::new();
    terminal.compose(&screen, &mut picture);
    (terminal, events, picture)
}

fn failure(events: &[Event]) -> Option<(u8, u16)> {
    events.iter().find_map(|event| match event {
        Event::CommitFailed { error, drawing } => Some((error.code(), *drawing)),
        _ => None,
    })
}

/// A square of side 20 outlined in colour 3 at the drawing's origin.
const SQUARE: &str = "W(I3)P[0,0]V[19,0][19,19][0,19][0,0]";
/// The code of a malformed `D` operation (`docs/regis-v2.md`, "Failed commits").
const MALFORMED: u8 = 7;
/// After a malformed operation reading resumes at the next semicolon, so
/// the batch's commit is sent after one (`docs/regis-v2.md`, "Grammar").
const COMMIT_AFTER_RESYNC: &str = ";D(C)";

#[test]
fn a_label_is_kept_at_the_commit_and_never_drawn() {
    let plain = format!("D(B3){SQUARE}D(E)D(S3(X100,Y50,I1),C)");
    let labelled = format!("D(B3){SQUARE}D(E)D(S3(X100,Y50,I1,U'Next page'),C)");
    let (_, _, without) = run(plain.as_bytes());
    let (terminal, events, with) = run(labelled.as_bytes());
    assert_eq!(events, [Event::Committed(1)]);
    assert_eq!(terminal.label(3).as_deref(), Some("Next page"));
    assert_eq!(terminal.labels(), [(3, "Next page".to_owned())]);
    assert_eq!(with.canvas(), without.canvas(), "a label changes no pixel");
    assert_eq!(with.pick_record(), without.pick_record(), "nor what can be picked");
}

#[test]
fn a_label_waits_for_its_commit_and_can_be_changed_and_removed() {
    let (terminal, ..) = run(b"D(S4(U'Draft'))");
    assert_eq!(terminal.label(4), None, "nothing is committed yet");
    let (terminal, ..) = run(b"D(S4(U'Draft'),C)D(S4(U\"Final, \"\"signed\"\"\"),C)");
    assert_eq!(terminal.label(4).as_deref(), Some("Final, \"signed\""));
    let (terminal, ..) = run(b"D(S4(U'Draft'),C)D(S4(U''),C)");
    assert_eq!(terminal.label(4), None, "an empty label removes it");
    assert_eq!(terminal.labels(), []);
    let (terminal, ..) = run(b"D(S4(U'Draft'),C)D(K4,C)");
    assert_eq!(terminal.label(4), None, "a deleted drawing has none");
}

#[test]
fn a_label_is_read_as_latin_1() {
    // 0xE9 is é in ISO/IEC 8859-1, the right half the VT340's DEC
    // Multinational set follows.
    let (terminal, ..) = run(b"D(S5(U'Caf\xe9'),C)");
    assert_eq!(terminal.label(5).as_deref(), Some("Café"));
}

#[test]
fn a_label_too_long_or_with_a_control_character_fails_the_batch() {
    let longest = "a".repeat(MAX_LABEL_BYTES);
    let (terminal, events, _) = run(format!("D(S6(U'{longest}'),C)").as_bytes());
    assert_eq!(events, [Event::Committed(1)]);
    assert_eq!(terminal.label(6), Some(longest));

    let too_long = "a".repeat(MAX_LABEL_BYTES + 1);
    let (terminal, events, _) = run(format!("D(S6(U'{too_long}')){COMMIT_AFTER_RESYNC}").as_bytes());
    assert_eq!(failure(&events), Some((MALFORMED, 6)));
    assert_eq!(terminal.label(6), None);

    // A tab is kept in a v1 string, and refused in a label.
    let (_, events, _) = run(format!("D(S6(U'a\tb')){COMMIT_AFTER_RESYNC}").as_bytes());
    assert_eq!(failure(&events), Some((MALFORMED, 6)));
    // A string after any other attribute letter is malformed too.
    let (_, events, _) = run(format!("D(S6(X'a')){COMMIT_AFTER_RESYNC}").as_bytes());
    assert_eq!(failure(&events), Some((MALFORMED, 6)));
}

#[test]
fn a_title_is_committed_kept_through_delete_all_and_never_drawn() {
    let (terminal, events, picture) = run(b"D(T'Harbour webcam')");
    assert_eq!(events, []);
    assert_eq!(terminal.title(), None, "nothing is committed yet");
    let (_, _, blank) = run(b"");
    assert_eq!(picture.canvas(), blank.canvas());

    let (terminal, events, picture) = run(b"D(T'Harbour webcam',C)");
    assert_eq!(events, [Event::Committed(1)]);
    assert_eq!(terminal.title().as_deref(), Some("Harbour webcam"));
    assert_eq!(picture.canvas(), blank.canvas(), "a title changes no pixel");

    let (terminal, ..) = run(b"D(T'Harbour webcam',C)D(Z,C)");
    assert_eq!(terminal.title().as_deref(), Some("Harbour webcam"), "D(Z) deletes drawings, not the title");
    let (terminal, ..) = run(b"D(T'Harbour webcam',C)D(T'',C)");
    assert_eq!(terminal.title(), None, "an empty title removes it");
}

#[test]
fn a_title_too_long_or_with_a_control_character_fails_the_batch() {
    let longest = "t".repeat(MAX_TITLE_BYTES);
    let (terminal, events, _) = run(format!("D(T'{longest}',C)").as_bytes());
    assert_eq!(events, [Event::Committed(1)]);
    assert_eq!(terminal.title(), Some(longest));
    let too_long = "t".repeat(MAX_TITLE_BYTES + 1);
    let (terminal, events, _) = run(format!("D(T'{too_long}'){COMMIT_AFTER_RESYNC}").as_bytes());
    assert_eq!(failure(&events), Some((MALFORMED, 0)));
    assert_eq!(terminal.title(), None);
    let (_, events, _) = run(format!("D(T'line\nbreak'){COMMIT_AFTER_RESYNC}").as_bytes());
    assert_eq!(failure(&events), Some((MALFORMED, 0)));
    // `T` names the title only directly inside `D(...)`; a string after
    // another operation letter is malformed.
    let (_, events, _) = run(format!("D(K'x'){COMMIT_AFTER_RESYNC}").as_bytes());
    assert_eq!(failure(&events), Some((MALFORMED, 0)));
}

#[test]
fn the_validator_gives_the_same_labels_and_title() {
    let stream = format!("D(B3){SQUARE}D(E)D(S3(U'Play'),T'Arcade',C)");
    let mut validator = Validator::new();
    let events = validator.feed_regis(stream.as_bytes());
    assert_eq!(events, [Event::Committed(1)]);
    let (terminal, ..) = run(stream.as_bytes());
    assert_eq!(validator.label(3), terminal.label(3));
    assert_eq!(validator.labels(), [(3, "Play".to_owned())]);
    assert_eq!(validator.title().as_deref(), Some("Arcade"));
}
