//! ReGIS v2 viewer state: hover and pressed looks and the hidden drawing
//! (`docs/regis-v2.md`, "Viewer state"; REQ-6367).

use regis::{Event, Picture, Screen, Terminal, ViewerState};

/// A terminal fed `input` as ReGIS commands, its screen and the events.
fn fed(input: &str) -> (Terminal, Screen, Vec<Event>) {
    let mut terminal = Terminal::new();
    let mut screen = Screen::new();
    let mut events = terminal.feed_regis(&mut screen, input.as_bytes());
    events.extend(terminal.finish(&mut screen));
    (terminal, screen, events)
}

/// The picture under a viewer state, and whether setting it said the
/// picture might differ.
fn shown(terminal: &mut Terminal, screen: &Screen, state: ViewerState) -> (Picture, bool) {
    let differs = terminal.set_viewer_state(state);
    let mut picture = Picture::new();
    terminal.compose(screen, &mut picture);
    (picture, differs)
}

fn pixel(picture: &Picture, x: u32, y: u32) -> u8 {
    picture.canvas().pixel(x, y).unwrap()
}

fn failure(events: &[Event]) -> Option<(u8, u16)> {
    events.iter().find_map(|event| match event {
        Event::CommitFailed { error, drawing } => Some((error.code(), *drawing)),
        _ => None,
    })
}

/// A square of side 20 outlined in a colour at the drawing's origin.
fn square(colour: u8) -> String {
    format!("W(I{colour})P[0,0]V[19,0][19,19][0,19][0,0]")
}

/// Drawing 1, detectable at (100, 50), outlined in colour 3; its hover
/// look is drawing 2, in colour 5, and its pressed look drawing 3, in
/// colour 6. The looks are hidden, as a channel keeps them.
fn button() -> String {
    format!(
        "D(B1){}D(E)D(B2){}D(E)D(B3){}D(E)D(S1(X100,Y50,I1,H2,A3),S2(V0),S3(V0),C)",
        square(3),
        square(5),
        square(6)
    )
}

const NONE: ViewerState = ViewerState { hovered: 0, pressed: 0, hidden: 0 };

#[test]
fn a_hovered_drawing_is_painted_with_its_hover_look_and_stays_itself_in_the_pick_record() {
    let (mut terminal, screen, events) = fed(&button());
    assert_eq!(events, [Event::Committed(1)]);
    let (plain, _) = shown(&mut terminal, &screen, NONE);
    assert_eq!(pixel(&plain, 100, 50), 3);
    assert_eq!(plain.pick(100, 50).drawing, 1);

    let (hovered, differs) = shown(&mut terminal, &screen, ViewerState { hovered: 1, ..NONE });
    assert!(differs);
    assert_eq!(pixel(&hovered, 100, 50), 5);
    assert_eq!(pixel(&hovered, 119, 69), 5);
    assert_eq!(hovered.pick(100, 50).drawing, 1, "a look gives appearance, not identity");
    assert_eq!(pixel(&hovered, 0, 0), 0, "the hidden look is not painted as itself");

    let (again, differs) = shown(&mut terminal, &screen, NONE);
    assert!(differs);
    assert_eq!(again.canvas(), plain.canvas());
}

#[test]
fn the_pressed_look_wins_over_the_hover_look() {
    let (mut terminal, screen, _) = fed(&button());
    let (pressed, differs) = shown(&mut terminal, &screen, ViewerState { hovered: 1, pressed: 1, ..NONE });
    assert!(differs);
    assert_eq!(pixel(&pressed, 100, 50), 6);
    let (released, differs) = shown(&mut terminal, &screen, ViewerState { hovered: 1, ..NONE });
    assert!(differs);
    assert_eq!(pixel(&released, 100, 50), 5);
}

#[test]
fn a_look_that_names_a_missing_drawing_leaves_the_ordinary_look() {
    let (mut terminal, screen, events) = fed(&format!("D(B1){}D(E)D(S1(X100,Y50,I1,H9,A8),C)", square(3)));
    assert_eq!(events, [Event::Committed(1)]);
    let (picture, differs) = shown(&mut terminal, &screen, ViewerState { hovered: 1, pressed: 1, ..NONE });
    assert!(!differs, "nothing would be painted differently");
    assert_eq!(pixel(&picture, 100, 50), 3);
}

#[test]
fn a_drawing_with_no_looks_is_not_changed_by_the_pointer() {
    let (mut terminal, screen, _) = fed(&format!("D(B1){}D(E)D(S1(X100,Y50,I1),C)", square(3)));
    assert!(!terminal.set_viewer_state(ViewerState { hovered: 1, pressed: 1, ..NONE }));
    assert!(!terminal.set_viewer_state(ViewerState { hovered: 7, ..NONE }), "nor by a drawing that does not exist");
    let (picture, _) = shown(&mut terminal, &screen, ViewerState { hovered: 1, ..NONE });
    assert_eq!(pixel(&picture, 100, 50), 3);
}

#[test]
fn the_hidden_drawing_and_its_children_vanish_and_return() {
    let child = format!("D(B4){}D(E)D(S4(P1,X30))", square(7));
    let (mut terminal, screen, _) = fed(&format!("D(B1){}D(E)D(S1(X100,Y50)){child}D(C)", square(3)));
    let (plain, _) = shown(&mut terminal, &screen, NONE);
    assert_eq!((pixel(&plain, 100, 50), pixel(&plain, 130, 50)), (3, 7));
    let (hidden, differs) = shown(&mut terminal, &screen, ViewerState { hidden: 1, ..NONE });
    assert!(differs);
    assert_eq!((pixel(&hidden, 100, 50), pixel(&hidden, 130, 50)), (0, 0));
    let (back, differs) = shown(&mut terminal, &screen, NONE);
    assert!(differs);
    assert_eq!(back.canvas(), plain.canvas());
}

#[test]
fn a_drawing_painted_as_a_source_does_not_use_its_own_looks() {
    // Drawing 4 is painted with drawing 1's contents; hovering 1 changes 1
    // alone.
    let (mut terminal, screen, _) = fed(&format!("{}D(S4(X300,Y50,R1),C)", button()));
    let (hovered, _) = shown(&mut terminal, &screen, ViewerState { hovered: 1, ..NONE });
    assert_eq!((pixel(&hovered, 100, 50), pixel(&hovered, 300, 50)), (5, 3));
}

#[test]
fn looks_that_lead_back_to_their_drawing_fail_the_commit_and_change_nothing() {
    let (mut terminal, mut screen, _) = fed(&button());
    let mut before = Picture::new();
    terminal.compose(&screen, &mut before);
    // 1's hover look is 2; making 1 the pressed look of 2 closes a ring.
    let events = terminal.feed_regis(&mut screen, b"D(S2(A1),C)");
    assert_eq!(failure(&events), Some((4, 1)));
    let mut after = Picture::new();
    terminal.compose(&screen, &mut after);
    assert_eq!(after.canvas(), before.canvas());
    assert_eq!(terminal.commits(), 1);
}

#[test]
fn a_chain_of_looks_is_bounded_like_a_chain_of_sources() {
    // Each drawing's hover look is the next: sixteen deep is allowed.
    let chain = |length: u16| -> String {
        let links = (1..length).map(|n| format!("D(S{n}(H{}))", n + 1)).collect::<Vec<_>>().concat();
        format!("D(B{length}){}D(E){links}D(C)", square(3))
    };
    let (_, _, events) = fed(&chain(16));
    assert_eq!(events, [Event::Committed(1)]);
    let (_, _, events) = fed(&chain(17));
    assert_eq!(failure(&events).map(|(code, _)| code), Some(3));
}

#[test]
fn a_look_dearer_than_the_cap_fails_the_commit_though_the_ordinary_look_is_cheap() {
    // A filled box of 30,000 by 30,000 pixels costs far more than a whole
    // repaint may; it is only ever a pressed look.
    let dear = "W(S1)P[0,0]F(V[29999,0][29999,29999][0,29999])";
    let stream = format!("D(B1){}D(E)D(B2){dear}D(E)D(S2(V0)", square(3));
    let (_, _, events) = fed(&format!("{stream},C)"));
    assert_eq!(events, [Event::Committed(1)], "hidden and unused, it costs nothing");
    let (_, _, events) = fed(&format!("{stream},S1(A2),C)"));
    assert_eq!(failure(&events), Some((6, 0)));
}
