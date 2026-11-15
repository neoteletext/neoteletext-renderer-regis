//! ReGIS v2: numbered drawings (`docs/regis-v2.md`).

use regis::{CommitError, Event, Picture, Screen, Terminal};

/// A terminal fed `input` as ReGIS commands, its screen, the events, and
/// the picture composed afterwards.
fn run(input: &[u8]) -> (Terminal, Screen, Vec<Event>, Picture) {
    let mut terminal = Terminal::new();
    let mut screen = Screen::new();
    let mut events = terminal.feed_regis(&mut screen, input);
    events.extend(terminal.finish(&mut screen));
    let mut picture = Picture::new();
    terminal.compose(&screen, &mut picture);
    (terminal, screen, events, picture)
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

/// A square of side 20 outlined in colour 3 at the drawing's origin.
const SQUARE: &str = "W(I3)P[0,0]V[19,0][19,19][0,19][0,0]";

#[test]
fn a_drawing_appears_at_its_position_only_after_the_commit() {
    let define = format!("D(B3){SQUARE}D(E)D(S3(X100,Y50))");
    let (_, screen, events, picture) = run(define.as_bytes());
    assert_eq!(events, []);
    assert_eq!(pixel(&picture, 100, 50), 0);
    assert_eq!(screen.displayed().pixel(0, 0), Some(0), "the contents were not drawn on the base");

    let (terminal, screen, events, picture) = run(format!("{define}D(C)").as_bytes());
    assert_eq!(events, [Event::Committed(1)]);
    assert_eq!(terminal.commits(), 1);
    assert_eq!(pixel(&picture, 100, 50), 3);
    assert_eq!(pixel(&picture, 119, 69), 3);
    assert_eq!(pixel(&picture, 110, 60), 0);
    assert_eq!(pixel(&picture, 0, 0), 0);
    assert_eq!(screen.displayed().pixel(100, 50), Some(0), "the base holds no numbered drawing");
    assert_eq!(picture.commits(), 1);
    assert!(picture.work() > 0);
}

#[test]
fn a_stream_without_d_never_has_drawings() {
    let (terminal, screen, events, picture) = run(b"P[10,10]V[20,10]");
    assert_eq!(events, []);
    assert!(!terminal.has_drawings());
    assert_eq!(picture.canvas(), screen.displayed());
    assert_eq!(picture.work(), 0);
}

#[test]
fn moving_hiding_and_deleting_take_effect_at_their_commits() {
    let base = format!("D(B1){SQUARE}D(E)D(S1(X10,Y10),C)");
    let (.., picture) = run(format!("{base}D(S1(X300),C)").as_bytes());
    assert_eq!((pixel(&picture, 10, 10), pixel(&picture, 300, 10)), (0, 3));
    assert_eq!(picture.commits(), 2);
    let (.., picture) = run(format!("{base}D(S1(V0),C)").as_bytes());
    assert_eq!(pixel(&picture, 10, 10), 0);
    let (.., picture) = run(format!("{base}D(K1,C)").as_bytes());
    assert_eq!(pixel(&picture, 10, 10), 0);
    let (.., picture) = run(format!("{base}D(Z,C)").as_bytes());
    assert_eq!(pixel(&picture, 10, 10), 0);
}

#[test]
fn children_move_hide_and_are_deleted_with_their_parent() {
    let scene = format!("D(B2){SQUARE}D(E)D(S1(X100,Y100),S2(P1,X5,Y5),C)");
    let (.., picture) = run(scene.as_bytes());
    assert_eq!(pixel(&picture, 105, 105), 3);
    let (.., picture) = run(format!("{scene}D(S1(V0),C)").as_bytes());
    assert_eq!(pixel(&picture, 105, 105), 0);
    let (.., picture) = run(format!("{scene}D(K1,C)").as_bytes());
    assert_eq!(pixel(&picture, 105, 105), 0);
    // A drawing moved to another parent beforehand survives.
    let (.., picture) = run(format!("{scene}D(S2(P0),K1,C)").as_bytes());
    assert_eq!(pixel(&picture, 5, 5), 3);
}

#[test]
fn a_source_is_drawn_in_place_of_contents_wherever_it_is_named() {
    let scene = format!("D(B3){SQUARE}D(E)D(S3(V0),S10(R3,X200,Y100),S11(R3,X400,Y100),C)");
    let (.., picture) = run(scene.as_bytes());
    assert_eq!(pixel(&picture, 0, 0), 0, "the hidden master is not shown");
    assert_eq!(pixel(&picture, 200, 100), 3);
    assert_eq!(pixel(&picture, 400, 100), 3);
    // A source that does not exist draws nothing and is no error.
    let (_, _, events, _) = run(b"D(S5(R9),C)");
    assert_eq!(events, [Event::Committed(1)]);
}

#[test]
fn depth_orders_drawings_and_number_orders_equal_depths() {
    let block = |colour: u8| format!("W(I{colour})F(P[0,0]V[9,0][9,9][0,9])");
    let scene = format!("D(B1){}D(E)D(B2){}D(E)", block(1), block(2));
    let (.., picture) = run(format!("{scene}D(C)").as_bytes());
    assert_eq!(pixel(&picture, 5, 5), 2, "the higher number draws later");
    let (.., picture) = run(format!("{scene}D(S1(L5),C)").as_bytes());
    assert_eq!(pixel(&picture, 5, 5), 1, "the higher depth draws later");
}

#[test]
fn a_clip_rectangle_cuts_a_drawing_and_its_children() {
    let scene = format!("D(B1){SQUARE}D(E)D(B2){SQUARE}D(E)D(S1(X100,Y100,C(X0,Y0,W10,H40)),S2(P1,Y20),C)");
    let (.., picture) = run(scene.as_bytes());
    assert_eq!(pixel(&picture, 100, 100), 3);
    assert_eq!(pixel(&picture, 119, 100), 0, "outside the clip rectangle");
    assert_eq!(pixel(&picture, 100, 120), 3, "the child inside it");
    assert_eq!(pixel(&picture, 119, 125), 0, "the child outside it");
    let (.., picture) = run(format!("{scene}D(S1(C0),C)").as_bytes());
    assert_eq!(pixel(&picture, 119, 100), 3);
}

#[test]
fn each_drawing_starts_from_a_fresh_context() {
    // Drawing 1 sets a colour and moves; drawing 2 draws with the defaults.
    let scene = "D(B1)W(I5)P[300,300]D(E)D(B2)V[10,0]D(E)D(C)";
    let (.., picture) = run(scene.as_bytes());
    assert_eq!(pixel(&picture, 5, 0), 7, "the default foreground, from the origin");
    // And the base's state does not leak in either.
    let (.., picture) = run(format!("W(I2)P[50,50]{scene}").as_bytes());
    assert_eq!(pixel(&picture, 5, 0), 7);
}

#[test]
fn terminal_commands_inside_a_drawing_have_no_effect() {
    // S(E) would erase, S(I) set the background, L load a cell, R report.
    let (_, screen, events, picture) =
        run(b"P[0,0]V[50,0]D(B1)S(E)S(I4)S[10,10]L'a'FFFF;R(P)@:AV[9,9]@;V[20,0]D(E)D(C)@A");
    assert_eq!(events, [Event::Committed(1)], "no reply was sent");
    assert_eq!(screen.displayed().pixel(40, 0), Some(7), "the base was not erased");
    assert_eq!(pixel(&picture, 40, 0), 7);
    assert_eq!(pixel(&picture, 9, 9), 0, "no macrograph was defined");
}

#[test]
fn writing_lands_on_what_is_beneath() {
    // Complement writing inverts the base pixel under it: 7 becomes 8.
    let (.., picture) = run(b"P[0,5]V[30,5]D(B1)W(C)P[10,0]V[10,10]D(E)D(C)");
    assert_eq!(pixel(&picture, 10, 5), 8);
    assert_eq!(pixel(&picture, 10, 4), 15);
}

#[test]
fn macrographs_are_used_as_they_stood_at_the_commit() {
    let (.., picture) = run(b"@:AV[10,0]@;D(B1)@AD(E)D(C)@:AV[0,10]@;");
    assert_eq!(pixel(&picture, 5, 0), 7);
    assert_eq!(pixel(&picture, 0, 5), 0, "the later definition waits for a commit");
    let (.., picture) = run(b"@:AV[10,0]@;D(B1)@AD(E)D(C)@:AV[0,10]@;D(C)");
    assert_eq!(pixel(&picture, 0, 5), 7);
    assert_eq!(pixel(&picture, 5, 0), 0);
}

#[test]
fn d_from_a_macrograph_is_not_honoured() {
    let (terminal, _, events, _) = run(b"@:AD(S1(X5),C)@;@A");
    assert_eq!(events, []);
    assert!(!terminal.has_drawings());
}

#[test]
fn a_change_of_the_output_map_in_a_batch_waits_for_its_commit() {
    let mut terminal = Terminal::new();
    let mut screen = Screen::new();
    let before = terminal.palette().colour(1);
    let _ = terminal.feed_regis(&mut screen, b"D(S1(X1))S(M1(H0L100S0))");
    assert_eq!(terminal.palette().colour(1), before, "held while the batch is open");
    let _ = terminal.feed_regis(&mut screen, b"D(C)");
    assert_ne!(terminal.palette().colour(1), before);
    // A failed commit drops the change.
    let now = terminal.palette().colour(2);
    let events = terminal.feed_regis(&mut screen, b"D(S2(P9))S(M2(H0L100S0))D(C)");
    assert_eq!(failure(&events), Some((5, 2)));
    assert_eq!(terminal.palette().colour(2), now);
}

#[test]
fn each_failure_leaves_everything_as_it_was() {
    let good = format!("D(B1){SQUARE}D(E)D(S1(X10,Y10,I1),C)");
    let cases: [(&str, u8, u16); 7] = [
        ("D(S2(P9),C)", CommitError::Parent.code(), 2),
        ("D(S2(P1,R1),C)", CommitError::Cycle.code(), 1),
        ("D(S2(X1),Q,C);D(C)", CommitError::Malformed.code(), 2),
        ("D(B70000)V[5,5]D(E)D(C)", CommitError::Malformed.code(), 0),
        ("D(E)D(C)", CommitError::Malformed.code(), 0),
        ("D(B2)V[1,1]D(K1)D(E)D(C)", CommitError::Malformed.code(), 2),
        ("D(S1(V2),C);D(C)", CommitError::Malformed.code(), 1),
    ];
    let (_, _, _, before) = run(good.as_bytes());
    for (bad, code, drawing) in cases {
        let (terminal, _, events, after) = run(format!("{good}{bad}").as_bytes());
        assert_eq!(failure(&events), Some((code, drawing)), "{bad}");
        assert_eq!(terminal.commits(), 1, "{bad}");
        assert_eq!(after.canvas(), before.canvas(), "{bad}");
        assert_eq!(after.pick_record(), before.pick_record(), "{bad}");
        // The batch after a failed one starts from what is on show.
        let (terminal, _, _, later) = run(format!("{good}{bad}D(S1(X11),C)").as_bytes());
        assert_eq!(terminal.commits(), 2, "{bad}");
        assert_eq!(pixel(&later, 11, 10), 3, "{bad}");
    }
}

#[test]
fn a_chain_of_sixteen_is_allowed_and_seventeen_is_not() {
    let chain = |length: u16| {
        let links: Vec<String> = (2..=length).map(|n| format!("S{n}(P{})", n - 1)).collect();
        format!("D(S1(X1),{},C)", links.join(","))
    };
    let (_, _, events, _) = run(chain(16).as_bytes());
    assert_eq!(events, [Event::Committed(1)]);
    let (_, _, events, _) = run(chain(17).as_bytes());
    assert_eq!(failure(&events).map(|(code, _)| code), Some(CommitError::Depth.code()));
}

#[test]
fn work_over_the_cap_fails_the_commit_and_hidden_drawings_cost_nothing() {
    // One filled screen costs about 800 * 480 * 2 units; 200 uses pass 100 million.
    let master = "D(B1)F(P[0,0]V[799,0][799,479][0,479])D(E)D(S1(V0))";
    let uses = |count: u16| (0..count).map(|n| format!("D(S{}(R1))", n + 2)).collect::<Vec<_>>().concat();
    let (_, _, events, picture) = run(format!("{master}{}D(C)", uses(100)).as_bytes());
    assert_eq!(events, [Event::Committed(1)]);
    assert!(picture.work() > 70_000_000 && picture.work() <= 100_000_000, "{}", picture.work());
    let (_, _, events, _) = run(format!("{master}{}D(C)", uses(200)).as_bytes());
    assert_eq!(failure(&events), Some((CommitError::Work.code(), 0)));
    // Hidden, the same uses cost nothing.
    let hidden = (0..200).map(|n| format!("D(S{}(R1,V0))", n + 2)).collect::<Vec<_>>().concat();
    let (_, _, events, picture) = run(format!("{master}{hidden}D(C)").as_bytes());
    assert_eq!(events, [Event::Committed(1)]);
    assert_eq!(picture.work(), 0);
}

#[test]
fn a_drawings_work_is_the_same_wherever_it_is_placed_and_however_clipped() {
    let contents = "D(B1)W(S1)P[0,50]V[300,50]F(P[0,0]V[90,0][90,90])T'Hello'D(E)";
    let work = |placement: &str| run(format!("{contents}D(S1({placement}),C)").as_bytes()).3.work();
    let here = work("X0");
    assert!(here > 0);
    assert_eq!(work("X700,Y400"), here);
    assert_eq!(work("X-5000,Y9000"), here);
    assert_eq!(work("C(X0,Y0,W1,H1)"), here);
}

#[test]
fn picking_reports_the_nearest_detectable_drawing_and_the_place_in_it() {
    // Group 1 is detectable; its plain child 2 reports the group; its
    // detectable child 3 reports itself. Drawing 4 is not detectable.
    let scene = format!(
        "D(B2){SQUARE}D(E)D(B3){SQUARE}D(E)D(B4){SQUARE}D(E)\
         D(S1(X100,Y100,I1),S2(P1),S3(P1,X50,I1),S4(X300,Y300),C)"
    );
    let (.., picture) = run(scene.as_bytes());
    let pick = picture.pick(100, 119);
    assert_eq!((pick.drawing, pick.x, pick.y), (1, 0, 19));
    let pick = picture.pick(169, 100);
    assert_eq!((pick.drawing, pick.x, pick.y), (3, 19, 0));
    assert_eq!(picture.pick(300, 300).drawing, 0);
    assert_eq!(picture.pick(110, 110).drawing, 0, "picking follows drawn pixels");
    assert_eq!(picture.pick(5000, 5000).drawing, 0);
    // A reused drawing counts as part of the drawing that placed it.
    let (.., picture) = run(format!("{scene}D(S3(V0),S9(R1,X400,Y10,I1),C)").as_bytes());
    assert_eq!(picture.pick(400, 10).drawing, 9);
    assert_eq!(picture.pick(450, 10).drawing, 0, "hidden where it is reused too");
}

#[test]
fn the_picture_says_which_rows_changed() {
    let mut terminal = Terminal::new();
    let mut screen = Screen::new();
    let mut picture = Picture::new();
    let _ = terminal.feed_regis(&mut screen, format!("D(B1){SQUARE}D(E)D(S1(Y100),C)").as_bytes());
    terminal.compose(&screen, &mut picture);
    assert_eq!(picture.changed_rows(), Some((100, 119)));
    terminal.compose(&screen, &mut picture);
    assert_eq!(picture.changed_rows(), None);
    let _ = terminal.feed_regis(&mut screen, b"D(S1(Y200),C)");
    terminal.compose(&screen, &mut picture);
    assert_eq!(picture.changed_rows(), Some((100, 219)));
}

#[test]
fn the_screen_says_when_the_base_changed() {
    let mut terminal = Terminal::new();
    let mut screen = Screen::new();
    assert!(!screen.take_changed());
    let _ = terminal.feed_regis(&mut screen, b"P[5,5]V[9,9]");
    assert!(screen.take_changed());
    assert!(!screen.take_changed());
    let _ = terminal.feed_regis(&mut screen, b"D(B1)V[9,9]D(E)D(C)");
    assert!(!screen.take_changed(), "numbered drawings do not touch the base");
}

#[test]
fn a_definition_left_open_when_regis_mode_ends_fails_its_batch() {
    let mut terminal = Terminal::new();
    let mut screen = Screen::new();
    let events = terminal.feed(&mut screen, b"\x1bPpD(B1)V[5,5]\x1b\\\x1bPpV[30,0]D(C)\x1b\\");
    assert_eq!(failure(&events), Some((CommitError::Malformed.code(), 1)));
    assert_eq!(screen.displayed().pixel(20, 0), Some(7), "commands after it are read as commands again");
    // A staged batch itself survives leaving and re-entering ReGIS mode.
    let events = terminal.feed(&mut screen, b"\x1bPpD(S2(X1))\x1b\\\x1bPpD(C)\x1b\\");
    assert_eq!(events, [Event::Committed(1)]);
}

#[test]
fn the_result_does_not_depend_on_how_the_bytes_are_divided() {
    let scene = format!("D(B1){SQUARE}T'D(E)'D(E)D(S1(X40,Y40,I1,C(X0,Y0,W15,H15)),C)D(S1(X41),C)");
    let (_, _, whole_events, whole) = run(scene.as_bytes());
    let mut terminal = Terminal::new();
    let mut screen = Screen::new();
    let mut events = Vec::new();
    for byte in scene.as_bytes() {
        events.extend(terminal.feed_regis(&mut screen, &[*byte]));
    }
    let mut picture = Picture::new();
    terminal.compose(&screen, &mut picture);
    assert_eq!(events, whole_events);
    assert_eq!(picture.canvas(), whole.canvas());
    assert_eq!(picture.pick_record(), whole.pick_record());
}

#[test]
fn empty_drawings_reused_many_times_over_are_paid_for() {
    // Seven levels, each drawing holding sixteen children that all reuse
    // the next level: sixteen to the seventh uses of nothing at all.
    let mut scene = String::from("D(S701(X0))");
    for level in 0..7_u16 {
        let master = level * 100 + 1;
        let children: Vec<String> =
            (1..=16).map(|child| format!("S{}(P{master},R{})", master + child, master + 100)).collect();
        scene = format!("{scene}D(S{master}(X0),{})", children.join(","));
    }
    let (_, _, events, _) = run(format!("{scene}D(C)").as_bytes());
    assert_eq!(failure(&events), Some((CommitError::Work.code(), 0)));
}

#[test]
fn arbitrary_drawing_operations_never_panic_and_stay_within_the_work_cap() {
    // A fixed pseudo-random sequence (a linear congruential generator, so
    // the test needs no library and always runs the same streams).
    const PIECES: [&str; 24] = [
        "D(B",
        "D(E)",
        "D(S",
        "D(K",
        "D(Z)",
        "D(C)",
        "(",
        ")",
        ",",
        ";",
        "1",
        "2",
        "3",
        "65535",
        "-9",
        "X",
        "Y",
        "L",
        "P",
        "R",
        "I1",
        "C(X0,Y0,W50,H50)",
        "V[30,30]",
        "F(P[0,0]V[99,0][99,99])T'D(E)'@:A@;",
    ];
    let mut state = 0x2545_F491_4F6C_DD1D_u64;
    for _ in 0..300 {
        let mut stream = String::new();
        for _ in 0..200 {
            state = state.wrapping_mul(6_364_136_223_846_793_005).wrapping_add(1_442_695_040_888_963_407);
            let index = usize::try_from(state >> 33).unwrap() % PIECES.len();
            stream.push_str(PIECES[index]);
        }
        let (_, _, _, picture) = run(stream.as_bytes());
        assert!(picture.work() <= regis::limits::MAX_REBUILD_WORK);
    }
}

#[test]
fn the_validator_fails_the_commits_a_terminal_would_and_no_others() {
    let good = format!("D(B1){SQUARE}D(E)D(S1(X10,I1),C)");
    for (stream, expected) in [(good.clone(), None), (format!("{good}D(S2(P9),C)"), Some((5, 2)))] {
        let mut validator = regis::Validator::new();
        let mut events = validator.feed_regis(stream.as_bytes());
        events.extend(validator.finish());
        assert_eq!(failure(&events), expected);
        assert_eq!(failure(&run(stream.as_bytes()).2), expected);
        assert_eq!(validator.commits(), 1);
    }
}

#[test]
fn the_origins_are_those_of_the_detectable_drawings_painted() {
    let (.., picture) = run(format!("D(B2){SQUARE}D(E)D(S1(X100,Y50),S2(P1,X5,Y6,I1),S3(I1,V0),C)").as_bytes());
    assert_eq!(picture.origins().collect::<Vec<_>>(), [(2, 105, 56)]);
}
