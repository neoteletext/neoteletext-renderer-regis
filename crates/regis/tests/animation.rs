//! Animation of numbered drawings: the attributes `G`, `N`, `F` and `T`,
//! the receiver's animation state, layers, bounding rectangles and the
//! paint rate (neoteletext-common `docs/animation.md`, ANIM-9, ANIM-10,
//! ANIM-12, ANIM-13 and ANIM-15; DEC-6382; REQ-6530).

mod common;

use std::fmt::Write as _;

use regis::limits::{MAX_ANIMATED_DRAWINGS, MAX_REBUILD_WORK};
use regis::{AnimationState, CommittedDrawing, Event, ForcedLook, Picture, Rectangle, Screen, Terminal, Validator};

/// A terminal fed `input` as ReGIS commands, its screen and the events.
fn fed(input: &str) -> (Terminal, Screen, Vec<Event>) {
    let mut terminal = Terminal::new();
    let mut screen = Screen::new();
    let mut events = terminal.feed_regis(&mut screen, input.as_bytes());
    events.extend(terminal.finish(&mut screen));
    (terminal, screen, events)
}

fn composed(terminal: &mut Terminal, screen: &Screen) -> Picture {
    let mut picture = Picture::new();
    terminal.compose(screen, &mut picture);
    picture
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

/// A filled square of side 20 in a colour at the drawing's origin.
fn square(colour: u8) -> String {
    format!("W(I{colour})F(V[0,0][19,0][19,19][0,19])")
}

const NONE: AnimationState =
    AnimationState { shown: None, position: None, layer: None, order: None, frame: 0, look: ForcedLook::None };

// ---- ANIM-10: the four attributes ----

#[test]
fn each_attribute_is_read_at_its_bounds_in_either_case() {
    for good in [
        "D(S1(G0),C)",
        "D(S1(G3),C)",
        "D(S1(g2),C)",
        "D(S1(N0),C)",
        "D(S1(N65535),C)",
        "D(S1(n2),C)",
        "D(S1(F0),C)",
        "D(S1(F(2)),C)",
        "D(S1(f(2,3)),C)",
        "D(S1(F(2,3,4,5,6,7,8,9,10,11,12,13,14,15,16,17)),C)",
        "D(S1(T0),C)",
        "D(S1(T32767),C)",
        "D(S1(t50),C)",
        "D(S1(G2,N3,F(4,5),T7,X1),C)",
    ] {
        let (_, _, events) = fed(good);
        assert_eq!(events, [Event::Committed(1)], "{good}");
    }
}

#[test]
fn a_malformed_attribute_fails_the_batch_with_code_7() {
    for bad in [
        "D(S1(G4),C)",
        "D(S1(G-1),C)",
        "D(S1(N65536),C)",
        "D(S1(N-1),C)",
        "D(S1(F1),C)",
        "D(S1(F()),C)",
        "D(S1(F(0)),C)",
        "D(S1(F(65536)),C)",
        "D(S1(F(2,3,4,5,6,7,8,9,10,11,12,13,14,15,16,17,18)),C)",
        "D(S1(F(X2)),C)",
        "D(S1(F((2))),C)",
        "D(S1(T32768),C)",
        "D(S1(T-1),C)",
        "D(S1(G(1)),C)",
    ] {
        let (_, _, events) = fed(&format!("{bad};D(C)"));
        assert_eq!(failure(&events), Some((7, 1)), "{bad}");
    }
}

#[test]
fn frames_and_looks_may_not_form_a_ring_in_any_mixture() {
    // 1 frames 2, 2's alternative look is 3, 3's source is 1.
    let (_, _, events) = fed("D(S1(F(2)),S2(N3),S3(R1),C)");
    assert_eq!(failure(&events).map(|(code, _)| code), Some(4));
    let (_, _, events) = fed("D(S1(N1),C)");
    assert_eq!(failure(&events), Some((4, 1)));
    let (_, _, events) = fed("D(S1(F(2,1)),C)");
    assert_eq!(failure(&events).map(|(code, _)| code), Some(4));
}

#[test]
fn chains_of_frames_count_toward_the_nesting_cap() {
    let chain = |length: u16| {
        let links: Vec<String> = (1..length).map(|number| format!("S{number}(F({}))", number + 1)).collect();
        format!("D({},S{length}(X1),C)", links.join(","))
    };
    let (_, _, events) = fed(&chain(16));
    assert_eq!(events, [Event::Committed(1)]);
    let (_, _, events) = fed(&chain(17));
    assert_eq!(failure(&events).map(|(code, _)| code), Some(3));
}

#[test]
fn a_commit_counts_the_dearest_frame_and_look_of_each_drawing() {
    // One screen-filling fill costs about 384,000 units; 300 uses of it
    // pass the cap of 100,000,000 only if the frame is counted.
    let fill = "F(V[0,0][799,0][799,479][0,479])";
    let users = |attribute: &str| {
        let sets: Vec<String> = (10..310).map(|number| format!("S{number}({attribute})")).collect();
        format!("D(B1){fill}D(E)D(S1(V0),{},C)", sets.join(","))
    };
    for attribute in ["F(2,1)", "N1"] {
        let (_, _, events) = fed(&users(attribute));
        assert_eq!(failure(&events), Some((6, 0)), "{attribute}");
    }
    let (_, _, events) = fed(&users("X1"));
    assert_eq!(events, [Event::Committed(1)]);
}

// ---- ANIM-10's examples and the order of precedence ----

/// Drawing 40 at (100, 50) with frames 41 to 44 in colours 1 to 4, a
/// hover look 45 (colour 5), a pressed look 46 (6), an alternative look
/// 47 (7) and a source 48 (8); its own contents are colour 9.
fn sprite() -> String {
    let mut stream = format!("D(B40){}D(E)", square(9));
    for (number, colour) in (41..=48).zip(1..) {
        write!(stream, "D(B{number}){}D(E)D(S{number}(V0))", square(colour)).unwrap();
    }
    stream.push_str("D(S40(X100,Y50,G2,I1,F(41,42,43,44),H45,A46,N47,R48),C)");
    stream
}

fn shown_colour(terminal: &mut Terminal, screen: &Screen, state: AnimationState) -> u8 {
    assert!(terminal.set_animation(40, state) || state == NONE);
    pixel(&composed(terminal, screen), 105, 55)
}

#[test]
fn the_frame_in_force_names_the_entry_drawn_and_one_beyond_the_list_draws_the_last() {
    let (mut terminal, screen, events) = fed(&sprite());
    assert_eq!(events, [Event::Committed(1)]);
    // Without animation state the frame in force is 0.
    assert_eq!(pixel(&composed(&mut terminal, &screen), 105, 55), 1);
    for (frame, colour) in [(1, 2), (2, 3), (3, 4), (4, 4), (15, 4), (200, 4), (0, 1)] {
        assert_eq!(shown_colour(&mut terminal, &screen, AnimationState { frame, ..NONE }), colour, "frame {frame}");
    }
    // The pixels a frame writes belong to the drawing it stands for.
    assert_eq!(composed(&mut terminal, &screen).pick(105, 55).drawing, 40);
}

#[test]
fn a_forced_look_wins_over_the_pointer_the_frame_and_the_source() {
    let (mut terminal, screen, _) = fed(&sprite());
    let pointer = regis::ViewerState { hovered: 40, pressed: 40, hidden: 0 };
    for (look, colour) in [(ForcedLook::Hover, 5), (ForcedLook::Pressed, 6), (ForcedLook::Alternative, 7)] {
        let _ = terminal.set_viewer_state(pointer);
        assert_eq!(shown_colour(&mut terminal, &screen, AnimationState { look, frame: 2, ..NONE }), colour);
        let _ = terminal.set_viewer_state(regis::ViewerState::default());
        assert_eq!(shown_colour(&mut terminal, &screen, AnimationState { look, frame: 2, ..NONE }), colour);
    }
    // No forced look: pressed, then hover, then the frame.
    let state = AnimationState { frame: 2, ..NONE };
    let _ = terminal.set_viewer_state(pointer);
    assert_eq!(shown_colour(&mut terminal, &screen, state), 6);
    let _ = terminal.set_viewer_state(regis::ViewerState { hovered: 40, pressed: 0, hidden: 0 });
    assert_eq!(shown_colour(&mut terminal, &screen, state), 5);
    let _ = terminal.set_viewer_state(regis::ViewerState::default());
    assert_eq!(shown_colour(&mut terminal, &screen, state), 3);
    // No frame list: the source; no source: its own contents.
    let _ = terminal.feed_regis(&mut Screen::new(), b"D(S40(F0),C)");
    assert_eq!(shown_colour(&mut terminal, &screen, state), 8);
    let _ = terminal.feed_regis(&mut Screen::new(), b"D(S40(R0),C)");
    assert_eq!(shown_colour(&mut terminal, &screen, state), 9);
    // A forced look that names no drawing leaves the next in the order.
    let _ = terminal.feed_regis(&mut Screen::new(), b"D(S40(N0),C)");
    assert_eq!(shown_colour(&mut terminal, &screen, AnimationState { look: ForcedLook::Alternative, ..NONE }), 9);
}

#[test]
fn forced_look_codes_are_the_operation_bytes() {
    for (code, look) in
        [(0, ForcedLook::None), (1, ForcedLook::Hover), (2, ForcedLook::Pressed), (3, ForcedLook::Alternative)]
    {
        assert_eq!(ForcedLook::from_code(code), Some(look));
        assert_eq!(look.code(), code);
    }
    assert_eq!(ForcedLook::from_code(4), None);
}

// ---- ANIM-9 and ANIM-6: the animation state ----

#[test]
fn each_part_set_stands_in_for_the_committed_attribute() {
    let stream = format!("D(B1){}D(E)D(B2){}D(E)D(S1(X100,Y50),S2(X110,Y60),C)", square(3), square(5));
    let (mut terminal, screen, _) = fed(&stream);
    assert_eq!(terminal.drawing_position(1), Some((100, 50)));
    assert_eq!(pixel(&composed(&mut terminal, &screen), 115, 65), 5);
    // Position.
    assert!(terminal.set_animation(1, AnimationState { position: Some((300, 200)), ..NONE }));
    let picture = composed(&mut terminal, &screen);
    assert_eq!((pixel(&picture, 105, 55), pixel(&picture, 305, 205)), (0, 3));
    assert_eq!(terminal.drawing_position(1), Some((300, 200)));
    // Hidden, then shown again by clearing.
    assert!(terminal.set_animation(1, AnimationState { shown: Some(false), ..NONE }));
    let picture = composed(&mut terminal, &screen);
    assert_eq!((pixel(&picture, 105, 55), pixel(&picture, 305, 205)), (0, 0));
    terminal.clear_animation(1);
    assert_eq!(terminal.animation(1), None);
    // Order: 1 over 2.
    assert!(terminal.set_animation(1, AnimationState { order: Some(5), ..NONE }));
    assert_eq!(pixel(&composed(&mut terminal, &screen), 115, 65), 3);
    // Layer wins over order: 2 in the overlay.
    assert!(terminal.set_animation(2, AnimationState { layer: Some(3), order: Some(-128), ..NONE }));
    assert_eq!(pixel(&composed(&mut terminal, &screen), 115, 65), 5);
    // A layer above 3 is the overlay.
    assert!(terminal.set_animation(1, AnimationState { layer: Some(9), ..NONE }));
    assert_eq!(terminal.animation(1).unwrap().layer, Some(3));
}

#[test]
fn a_state_is_refused_for_a_drawing_that_does_not_exist_and_beyond_64_drawings() {
    let sets: Vec<String> = (1..=70).map(|number| format!("S{number}(X{number})")).collect();
    let (mut terminal, _, _) = fed(&format!("D({},C)", sets.join(",")));
    let state = AnimationState { position: Some((1, 1)), ..NONE };
    assert!(!terminal.set_animation(71, state));
    assert!(!terminal.set_animation(0, state));
    for number in 1..=u16::try_from(MAX_ANIMATED_DRAWINGS).unwrap() {
        assert!(terminal.set_animation(number, state), "{number}");
    }
    assert!(!terminal.set_animation(65, state));
    assert_eq!(terminal.animation(65), None);
    // One that holds state may change it; clearing one frees a place.
    assert!(terminal.set_animation(64, AnimationState { frame: 3, ..NONE }));
    terminal.clear_animation(1);
    assert!(terminal.set_animation(65, state));
    // A state that sets nothing is no state.
    assert!(terminal.set_animation(2, NONE));
    assert_eq!(terminal.animation(2), None);
    assert!(terminal.set_animation(66, state));
}

#[test]
fn the_last_writer_wins_part_by_part() {
    let (mut terminal, mut screen, _) = fed("D(S1(X1),S2(X2),C)");
    let state = AnimationState {
        shown: Some(false),
        position: Some((9, 9)),
        layer: Some(2),
        order: Some(4),
        frame: 5,
        look: ForcedLook::Alternative,
    };
    let set = |terminal: &mut Terminal| assert!(terminal.set_animation(1, state));
    set(&mut terminal);
    // A commit that sets nothing of drawing 1 leaves its state.
    let _ = terminal.feed_regis(&mut screen, b"D(S2(X5,V0,L3),C)");
    assert_eq!(terminal.animation(1), Some(state));
    let _ = terminal.feed_regis(&mut screen, b"D(S1(X7),C)");
    assert_eq!(terminal.animation(1), Some(AnimationState { position: None, ..state }));
    set(&mut terminal);
    let _ = terminal.feed_regis(&mut screen, b"D(S1(Y7),C)");
    assert_eq!(terminal.animation(1), Some(AnimationState { position: None, ..state }));
    set(&mut terminal);
    let _ = terminal.feed_regis(&mut screen, b"D(S1(V1),C)");
    assert_eq!(terminal.animation(1), Some(AnimationState { shown: None, ..state }));
    for attribute in ["L2", "G1"] {
        set(&mut terminal);
        let _ = terminal.feed_regis(&mut screen, format!("D(S1({attribute}),C)").as_bytes());
        assert_eq!(terminal.animation(1), Some(AnimationState { layer: None, order: None, ..state }), "{attribute}");
    }
    // Other attributes write over nothing.
    set(&mut terminal);
    let _ = terminal.feed_regis(&mut screen, b"D(S1(I1,R2,T4,C0),C)");
    assert_eq!(terminal.animation(1), Some(state));
    // A commit that fails writes over nothing.
    let _ = terminal.feed_regis(&mut screen, b"D(S1(X3,P99),C)");
    assert_eq!(terminal.animation(1), Some(state));
    // A deletion forgets the whole state, even if the number is used again.
    let _ = terminal.feed_regis(&mut screen, b"D(K1,S1(I1),C)");
    assert_eq!(terminal.animation(1), None);
    set(&mut terminal);
    let _ = terminal.feed_regis(&mut screen, b"D(Z,C)");
    assert_eq!(terminal.animation(1), None);
}

/// A linear congruential generator, so the tests need no library and
/// always run the same sequences.
struct Random(u64);

impl Random {
    fn below(&mut self, bound: u32) -> u32 {
        self.0 = self.0.wrapping_mul(6_364_136_223_846_793_005).wrapping_add(1_442_695_040_888_963_407);
        u32::try_from(self.0 >> 33).unwrap() % bound
    }

    fn signed(&mut self, bound: u32) -> i16 {
        i16::try_from(self.below(bound * 2)).unwrap() - i16::try_from(bound).unwrap()
    }
}

const CONTENTS: [&str; 8] = [
    "W(I3)P[0,0]V[60,0][60,40][0,40][0,0]",
    "W(C,I7)P[5,5]V[80,5][80,60]",
    "W(E)P[0,0]V[40,40]",
    "W(I6)F(C[+30])",
    "W(I9)T'Paint'",
    "W(I1,R)T(S2)'Aa'",
    "W(I4,S1)P[10,0]V[10,50]W(S0)",
    "F(P[0,0]V[50,0][25,45])",
];

/// The drawings a random scene uses.
const DRAWINGS: u16 = 8;

/// What a random scene gave one drawing: enough to say what an animation
/// state makes of it.
#[derive(Clone, Default)]
struct Model {
    hover: u16,
    pressed: u16,
    alternative: u16,
    frames: Vec<u16>,
    source: u16,
    /// Another drawing names it as a source, a look or a frame. Painted
    /// in another's place it uses its frame list and not its looks, which
    /// no committed attribute says, so the property is checked on the
    /// drawings painted only as themselves.
    reused: bool,
}

/// A random scene of eight drawings. Parents have lower numbers and
/// sources, looks and frames higher ones, so no ring can form.
fn scene(random: &mut Random) -> (String, Vec<Model>) {
    let mut stream = String::new();
    let mut models = vec![Model::default(); usize::from(DRAWINGS) + 1];
    for number in 1..=DRAWINGS {
        let contents = CONTENTS[usize::try_from(random.below(8)).unwrap()];
        write!(stream, "D(B{number}){contents}D(E)").unwrap();
        let higher = |random: &mut Random| {
            let span = u32::from(DRAWINGS - number);
            if span == 0 || random.below(3) != 0 { 0 } else { number + 1 + u16::try_from(random.below(span)).unwrap() }
        };
        let model = &mut models[usize::from(number)];
        model.hover = higher(random);
        model.pressed = higher(random);
        model.alternative = higher(random);
        model.source = higher(random);
        for _ in 0..random.below(4) {
            let frame = higher(random);
            if frame != 0 {
                model.frames.push(frame);
            }
        }
        let frames = if model.frames.is_empty() {
            "F0".to_string()
        } else {
            format!("F({})", model.frames.iter().map(u16::to_string).collect::<Vec<_>>().join(","))
        };
        write!(
            stream,
            "D(S{number}(X{},Y{},L{},V{},P{},R{},I{},G{},H{},A{},N{},{frames}))",
            random.below(700),
            random.below(400),
            random.signed(3),
            u8::from(random.below(4) != 0),
            if random.below(3) == 0 {
                u32::from(number) - 1 - random.below(u32::from(number)).min(u32::from(number) - 1)
            } else {
                0
            },
            model.source,
            random.below(2),
            random.below(4),
            model.hover,
            model.pressed,
            model.alternative,
        )
        .unwrap();
        if random.below(4) == 0 {
            write!(stream, "D(S{number}(C(X0,Y0,W{},H{})))", random.below(90), random.below(90)).unwrap();
        }
    }
    stream.push_str("D(C)");
    let named: Vec<u16> = models
        .iter()
        .flat_map(|model| {
            [model.hover, model.pressed, model.alternative, model.source].into_iter().chain(model.frames.clone())
        })
        .collect();
    for number in named {
        models[usize::from(number)].reused = true;
    }
    (stream, models)
}

fn random_state(random: &mut Random, model: &Model) -> AnimationState {
    let state = whole_state(random);
    if model.reused { AnimationState { frame: 0, look: ForcedLook::None, ..state } } else { state }
}

fn whole_state(random: &mut Random) -> AnimationState {
    AnimationState {
        shown: (random.below(3) == 0).then(|| random.below(2) == 0),
        position: (random.below(2) == 0).then(|| (random.signed(400) + 300, random.signed(300) + 200)),
        layer: (random.below(3) == 0).then(|| u8::try_from(random.below(4)).unwrap()),
        order: (random.below(3) == 0).then(|| i8::try_from(random.signed(100)).unwrap()),
        frame: u8::try_from(random.below(6)).unwrap(),
        look: ForcedLook::from_code(u8::try_from(random.below(4)).unwrap()).unwrap(),
    }
}

/// The `D` operation that commits what an animation state says, for a
/// drawing the scene gave `model`.
fn as_committed(number: u16, state: AnimationState, model: &Model) -> String {
    let mut attributes = Vec::new();
    if let Some((x, y)) = state.position {
        attributes.push(format!("X{x},Y{y}"));
    }
    if let Some(shown) = state.shown {
        attributes.push(format!("V{}", u8::from(shown)));
    }
    if let Some(layer) = state.layer {
        attributes.push(format!("G{layer}"));
    }
    if let Some(order) = state.order {
        attributes.push(format!("L{order}"));
    }
    let forced = match state.look {
        ForcedLook::None => 0,
        ForcedLook::Hover => model.hover,
        ForcedLook::Pressed => model.pressed,
        ForcedLook::Alternative => model.alternative,
    };
    let frame = model.frames.get(usize::from(state.frame)).or(model.frames.last()).copied();
    // A forced look or a frame is drawn "as a source is".
    if forced != 0 {
        attributes.push(format!("R{forced},F0"));
    } else if let Some(frame) = frame.filter(|_| state.frame != 0) {
        attributes.push(format!("R{frame},F0"));
    }
    if attributes.is_empty() { String::new() } else { format!("D(S{number}({}))", attributes.join(",")) }
}

/// The pixels, the pick record and the origins.
type Seen<'a> = (&'a [u8], &'a [u16], Vec<(u16, i32, i32)>);

fn seen(picture: &Picture) -> Seen<'_> {
    (picture.canvas().pixels(), picture.pick_record(), picture.origins().collect())
}

/// ANIM-9's property: for random scenes and random animation states, the
/// picture and the pick record equal those of the same scene committed
/// with those attribute values. The animated picture is made by damage
/// repaint of the picture before the states were set.
#[test]
fn a_picture_under_animation_state_is_the_picture_of_the_scene_committed_with_those_values() {
    let mut random = Random(0x5DEE_CE66_D1CE_4E5D);
    let mut compared = 0;
    for case in 0..150 {
        let (stream, models) = scene(&mut random);
        let mut terminal = common::terminal();
        let mut screen = Screen::new();
        let events = terminal.feed_regis(&mut screen, stream.as_bytes());
        if events != [Event::Committed(1)] {
            continue;
        }
        let mut animated = Picture::new();
        terminal.compose(&screen, &mut animated);
        for round in 0..3 {
            let mut reference = stream.clone();
            for number in 1..=DRAWINGS {
                terminal.clear_animation(number);
                if random.below(2) == 0 {
                    let state = random_state(&mut random, &models[usize::from(number)]);
                    assert!(terminal.set_animation(number, state));
                    reference.push_str(&as_committed(number, state, &models[usize::from(number)]));
                }
            }
            reference.push_str("D(C)");
            terminal.compose(&screen, &mut animated);
            let mut other = common::terminal();
            let mut other_screen = Screen::new();
            let events = other.feed_regis(&mut other_screen, reference.as_bytes());
            assert!(matches!(events.last(), Some(Event::Committed(_))), "case {case}: {reference}");
            let committed = composed(&mut other, &other_screen);
            assert!(seen(&animated) == seen(&committed), "case {case}, round {round}: {reference}");
            compared += 1;
        }
    }
    assert!(compared > 300, "only {compared} pictures were compared");
}

// ---- ANIM-12: layers ----

#[test]
fn layers_are_drawn_in_order_whatever_the_depths_and_children_follow_their_top_level_ancestor() {
    // Four squares on one spot, the lower layers given the higher depths
    // and numbers.
    let mut stream = String::new();
    for (number, layer, colour) in [(4, 0, 1), (3, 1, 2), (2, 2, 3), (1, 3, 4)] {
        write!(stream, "D(B{number}){}D(E)D(S{number}(X50,Y50,G{layer},L{}))", square(colour), 9 - layer).unwrap();
    }
    let (mut terminal, mut screen, _) = fed(&format!("{stream}D(C)"));
    assert_eq!(pixel(&composed(&mut terminal, &screen), 55, 55), 4);
    let _ = terminal.feed_regis(&mut screen, b"D(S1(V0),C)");
    assert_eq!(pixel(&composed(&mut terminal, &screen), 55, 55), 3);
    let _ = terminal.feed_regis(&mut screen, b"D(S2(V0),C)");
    assert_eq!(pixel(&composed(&mut terminal, &screen), 55, 55), 2);
    let _ = terminal.feed_regis(&mut screen, b"D(S3(V0),C)");
    assert_eq!(pixel(&composed(&mut terminal, &screen), 55, 55), 1);
    // A child's own `G` is not used: it is in its parent's layer.
    let child = format!("D(B5){}D(E)D(S5(P4,G3),S1(V1),C)", square(9));
    let _ = terminal.feed_regis(&mut screen, child.as_bytes());
    assert_eq!(pixel(&composed(&mut terminal, &screen), 55, 55), 4);
    // The default layer is 1: over layer 0, under layer 2.
    let default = format!("D(B6){}D(E)D(S6(X50,Y50),S1(V0),S2(V1),C)", square(10));
    let _ = terminal.feed_regis(&mut screen, default.as_bytes());
    assert_eq!(pixel(&composed(&mut terminal, &screen), 55, 55), 3);
    let _ = terminal.feed_regis(&mut screen, b"D(S2(V0),C)");
    assert_eq!(pixel(&composed(&mut terminal, &screen), 55, 55), 10);
}

/// ANIM-12's counter test: moving a layer-2 drawing over a costly `G0`
/// drawing replays none of that drawing's contents.
#[test]
fn moving_a_drawing_over_a_background_drawing_replays_none_of_the_background() {
    let background = "W(I2)F(V[0,0][799,0][799,479][0,479])W(I5)P[0,60]V[799,60]";
    let build = |layer: u8| format!("D(B1){background}D(E)D(B2){}D(E)D(S1(G{layer}),S2(G2,X100,Y50,I1),C)", square(3));
    let (mut terminal, screen, _) = fed(&build(0));
    let mut picture = Picture::new();
    terminal.compose(&screen, &mut picture);
    assert!(picture.work() > 380_000, "the first picture paints the background: {}", picture.work());
    let mut costs = Vec::new();
    for x in [110, 300, 640] {
        assert!(terminal.set_animation(2, AnimationState { position: Some((x, 50)), ..NONE }));
        terminal.compose(&screen, &mut picture);
        costs.push(picture.work());
        let column = u32::try_from(x).unwrap();
        assert_eq!(
            (pixel(&picture, column + 5, 55), pixel(&picture, column + 5, 60), pixel(&picture, 105, 60)),
            (3, 3, 5)
        );
        assert_eq!(picture.pick(column + 5, 55).drawing, 2);
    }
    // Two visits and the square's own contents, 64 a byte and its fill:
    // a hundredth of what the background's part of the damage would cost.
    for cost in &costs {
        assert!(*cost < 4000, "{cost}");
    }
    // With the same drawing in layer 1 the background's part is replayed.
    let (mut scene_layer, screen, _) = fed(&build(1));
    let mut other = Picture::new();
    scene_layer.compose(&screen, &mut other);
    assert!(scene_layer.set_animation(2, AnimationState { position: Some((110, 50)), ..NONE }));
    scene_layer.compose(&screen, &mut other);
    assert!(other.work() > costs[0] + 1000, "{} against {}", other.work(), costs[0]);
    assert_eq!(other.canvas(), {
        assert!(terminal.set_animation(2, AnimationState { position: Some((110, 50)), ..NONE }));
        terminal.compose(&screen, &mut picture);
        picture.canvas()
    });
}

#[test]
fn a_background_drawing_keeps_its_place_in_the_pick_record_under_a_moving_drawing() {
    let stream =
        format!("D(B1)F(V[0,0][199,0][199,199][0,199])D(E)D(B2){}D(E)D(S1(G0,I1),S2(G2,X50,Y50,I1),C)", square(3));
    let (mut terminal, screen, _) = fed(&stream);
    let mut picture = Picture::new();
    terminal.compose(&screen, &mut picture);
    assert_eq!((picture.pick(55, 55).drawing, picture.pick(150, 150).drawing), (2, 1));
    assert!(terminal.set_animation(2, AnimationState { position: Some((140, 140)), ..NONE }));
    terminal.compose(&screen, &mut picture);
    assert_eq!((picture.pick(55, 55).drawing, picture.pick(150, 150).drawing), (1, 2));
    assert_eq!(seen(&picture), seen(&composed(&mut terminal, &screen)));
}

// ---- ANIM-13: bounding rectangles ----

#[test]
fn a_drawings_rectangle_follows_its_state_and_is_none_while_it_is_not_shown() {
    let stream = format!(
        "D(B1){}D(E)D(B2){}D(E)D(B3)P[0,0]V[99,0]D(E)D(S1(X100,Y50),S2(P1,X30,Y30),S3(V0),C)",
        square(3),
        square(5)
    );
    let (mut terminal, _, _) = fed(&stream);
    // With its shown child.
    assert_eq!(terminal.bounds(1), Some(Rectangle { x: 100, y: 50, width: 50, height: 50 }));
    assert_eq!(terminal.bounds(2), Some(Rectangle { x: 130, y: 80, width: 20, height: 20 }));
    assert_eq!((terminal.bounds(3), terminal.bounds(4), terminal.bounds(0)), (None, None, None));
    assert!(terminal.set_animation(1, AnimationState { position: Some((790, 470)), ..NONE }));
    assert_eq!(terminal.bounds(1), Some(Rectangle { x: 790, y: 470, width: 10, height: 10 }));
    assert_eq!(terminal.bounds(2), None, "the child is off the screen");
    assert!(terminal.set_animation(1, AnimationState { shown: Some(false), ..NONE }));
    assert_eq!((terminal.bounds(1), terminal.bounds(2)), (None, None));
    // The look in force and the clip rectangle.
    let mut screen = Screen::new();
    let _ = terminal.feed_regis(&mut screen, b"D(S1(N3,X10,Y10),S2(V0),C)");
    assert!(terminal.set_animation(1, AnimationState { look: ForcedLook::Alternative, ..NONE }));
    assert_eq!(terminal.bounds(1), Some(Rectangle { x: 10, y: 10, width: 100, height: 1 }));
    let _ = terminal.feed_regis(&mut screen, b"D(S1(C(X5,Y0,W10,H10)),C)");
    assert_eq!(terminal.bounds(1), Some(Rectangle { x: 15, y: 10, width: 10, height: 1 }));
}

// ---- RULE-26 and ANIM-17: what the stream committed ----

/// RULE-26 (page-rules.md): a target's shown part, layer and order are
/// the animation state's where it sets them, else what the stream
/// committed, which the renderer says apart from the animation state.
#[test]
fn the_committed_shown_layer_and_order_are_given_apart_from_the_animation_state() {
    let stream = format!("D(B1){}D(E)D(S1(V0,G3,L-5),S2(P1),S3(L7))", square(3));
    let (mut terminal, mut screen, _) = fed(&stream);
    // Operations waiting for their commit have changed nothing.
    assert_eq!(terminal.committed_drawing(1), None);
    let _ = terminal.feed_regis(&mut screen, b"D(C)");
    assert_eq!(terminal.committed_drawing(1), Some(CommittedDrawing { shown: false, layer: 3, order: -5 }));
    // The defaults: shown, layer 1, depth 0 (ANIM-10).
    assert_eq!(terminal.committed_drawing(2), Some(CommittedDrawing { shown: true, layer: 1, order: 0 }));
    assert_eq!(terminal.committed_drawing(3), Some(CommittedDrawing { shown: true, layer: 1, order: 7 }));
    assert_eq!((terminal.committed_drawing(4), terminal.committed_drawing(0)), (None, None));
    // The animation state stands in for the attributes without changing
    // what was committed.
    let state = AnimationState { shown: Some(true), layer: Some(0), order: Some(9), ..NONE };
    assert!(terminal.set_animation(1, state));
    assert_eq!(terminal.committed_drawing(1), Some(CommittedDrawing { shown: false, layer: 3, order: -5 }));
    assert_eq!(terminal.animation(1), Some(state));
    // A later commit is the last writer; a deletion ends the drawing.
    let _ = terminal.feed_regis(&mut screen, b"D(S1(V1,G2,L4),C)");
    assert_eq!(terminal.committed_drawing(1), Some(CommittedDrawing { shown: true, layer: 2, order: 4 }));
    let _ = terminal.feed_regis(&mut screen, b"D(K1,C)");
    assert_eq!((terminal.committed_drawing(1), terminal.committed_drawing(2)), (None, None));
    // A failed commit leaves what was committed.
    let _ = terminal.feed_regis(&mut screen, b"D(S3(V0,P99),C)");
    assert_eq!(terminal.committed_drawing(3), Some(CommittedDrawing { shown: true, layer: 1, order: 7 }));
}

/// ANIM-17: "A sheet draws nothing itself: a ReGIS sheet's drawings that
/// are left visible ... are a fault the server refuses". The validator
/// names the drawing a sheet leaves able to write on the screen.
#[test]
fn a_validator_names_the_drawing_a_sheet_leaves_visible() {
    let visible = |sheet: &str| {
        let mut validator = Validator::new();
        let events = validator.feed_regis(sheet.as_bytes());
        assert_eq!(failure(&events), None, "{sheet}");
        validator.visible_drawing()
    };
    let masters = format!("D(B41){}D(E)D(B42){}D(E)", square(2), square(3));
    // A sheet as it should be: hidden masters, then a commit.
    assert_eq!(visible(&format!("{masters}D(S41(V0),S42(V0),C)")), None);
    // One master left shown; the lowest number is named.
    assert_eq!(visible(&format!("{masters}D(S41(V0),C)")), Some(42));
    assert_eq!(visible(&format!("{masters}D(C)")), Some(41));
    // A shown drawing that writes nothing is not a fault: an empty
    // drawing, the shown parent of hidden masters, and one whose contents
    // lie off the screen or outside its clip rectangle.
    assert_eq!(visible(&format!("{masters}D(S41(V0),S42(V0),S5(X10),C)")), None);
    assert_eq!(visible(&format!("{masters}D(S5(X10),S41(V0,P5),S42(V0,P5),C)")), None);
    assert_eq!(visible(&format!("{masters}D(S41(X800),S42(Y-20),C)")), None);
    assert_eq!(visible(&format!("{masters}D(S41(C(X20,Y0,W5,H5)),S42(V0),C)")), None);
    // A shown drawing that draws through a hidden master does draw: as
    // its source, and as the frame in force.
    assert_eq!(visible(&format!("{masters}D(S41(V0),S42(V0),S7(R41),C)")), Some(7));
    assert_eq!(visible(&format!("{masters}D(S41(V0),S42(V0),S8(F(42,41)),C)")), Some(8));
    // A shown child of a shown parent: the top-level drawing is named.
    assert_eq!(visible(&format!("{masters}D(S6(X5),S41(V0),S42(P6),C)")), Some(6));
    // Nothing committed: nothing is left visible, whatever waits.
    let mut validator = Validator::new();
    let _ = validator.feed_regis(masters.as_bytes());
    assert_eq!((validator.visible_drawing(), validator.committed_drawing(41)), (None, None));
    let _ = validator.feed_regis(b"D(S41(V0,G2),C)");
    assert_eq!(validator.committed_drawing(41), Some(CommittedDrawing { shown: false, layer: 2, order: 0 }));
    assert_eq!(validator.visible_drawing(), Some(42));
}

/// ANIM-17: a sheet draws nothing itself, on the base picture either.
#[test]
fn a_validator_says_what_a_sheet_drew_outside_numbered_drawings() {
    let base_work = |sheet: &str| {
        let mut validator = Validator::new();
        let events = validator.feed_regis(sheet.as_bytes());
        assert_eq!(failure(&events), None, "{sheet}");
        let _ = validator.finish();
        validator.base_work()
    };
    // Definitions, attributes and a commit draw nothing on the base,
    // however much the masters hold.
    let masters = format!("D(B41){}D(E)D(B42)P[0,0]V[799,479]T'text'D(E)D(S41(V0),S42(V0),C)", square(2));
    assert_eq!(base_work(&masters), 0);
    // Nor do commands that draw nothing: a position, a colour, a
    // macrograph's definition.
    assert_eq!(base_work(&format!("P[100,100]W(I3)@:AV[5,5]@;{masters}")), 0);
    // A vector, a fill, a character and an erase of the screen are each
    // drawing on the base, before the sheet's drawings or after.
    for drawn in ["P[0,0]V[10,0]", "F(V[0,0][9,0][9,9])", "T'a'", "S(E)", "P[5,5]V[]"] {
        assert!(base_work(&format!("{drawn}{masters}")) > 0, "{drawn} before");
        assert!(base_work(&format!("{masters}{drawn}")) > 0, "{drawn} after");
    }
}

/// The terminal's own answer follows the state in force: the animation
/// state can show a hidden drawing and hide a shown one.
#[test]
fn the_visible_drawing_follows_the_animation_state() {
    let (mut terminal, _, _) = fed(&format!("D(B1){}D(E)D(S1(V0),C)", square(3)));
    assert_eq!(terminal.visible_drawing(), None);
    assert!(terminal.set_animation(1, AnimationState { shown: Some(true), ..NONE }));
    assert_eq!(terminal.visible_drawing(), Some(1));
    assert!(terminal.set_animation(1, AnimationState { shown: Some(true), position: Some((800, 0)), ..NONE }));
    assert_eq!(terminal.visible_drawing(), None, "off the screen");
}

/// ANIM-13's test: every pixel a drawing writes lies inside its rectangle.
#[test]
fn every_pixel_a_drawing_writes_lies_inside_its_rectangle() {
    let mut random = Random(0x1234_5678_9ABC_DEF1);
    let mut checked = 0;
    for case in 0..60 {
        let (stream, _) = scene(&mut random);
        let mut terminal = common::terminal();
        let mut screen = Screen::new();
        if terminal.feed_regis(&mut screen, stream.as_bytes()) != [Event::Committed(1)] {
            continue;
        }
        for number in 1..=DRAWINGS {
            let _ = terminal.set_animation(number, whole_state(&mut random));
        }
        let with = composed(&mut terminal, &screen);
        for number in 1..=DRAWINGS {
            // Hiding the drawing changes pixels only inside its rectangle.
            let Some(bounds) = terminal.bounds(number) else { continue };
            let state = terminal.animation(number).unwrap_or(NONE);
            let _ = terminal.set_animation(number, AnimationState { shown: Some(false), ..state });
            let without = composed(&mut terminal, &screen);
            let _ = terminal.set_animation(number, state);
            if state == NONE {
                terminal.clear_animation(number);
            }
            for (index, (one, other)) in with.canvas().pixels().iter().zip(without.canvas().pixels()).enumerate() {
                if one != other {
                    let (x, y) = (u32::try_from(index % 800).unwrap(), u32::try_from(index / 800).unwrap());
                    let inside = (bounds.x..bounds.x + bounds.width).contains(&x)
                        && (bounds.y..bounds.y + bounds.height).contains(&y);
                    assert!(inside, "case {case}, drawing {number}: ({x}, {y}) outside {bounds:?}");
                }
            }
            checked += 1;
        }
    }
    assert!(checked > 100, "only {checked} rectangles were checked");
}

// ---- ANIM-15 and DEC-6382: the paint rate ----

/// How many pixels of a row hold a colour.
fn run(picture: &Picture, row: u32, colour: u8) -> usize {
    (0..800).filter(|x| pixel(picture, *x, row) == colour).count()
}

#[test]
fn a_line_is_painted_at_its_rate_and_the_finished_picture_is_the_picture_at_once() {
    // A line of 600 pixels at T6: 600 work units a second, 10 a tick.
    let line = "W(I3)P[0,0]V[599,0]";
    let (mut terminal, screen, _) = fed(&format!("D(B1){line}D(E)D(S1(X100,Y50,T6,I1),C)"));
    let (mut at_once, once_screen, _) = fed(&format!("D(B1){line}D(E)D(S1(X100,Y50,I1),C)"));
    let whole = composed(&mut at_once, &once_screen);
    let mut picture = Picture::new();
    terminal.compose(&screen, &mut picture);
    assert_eq!(run(&picture, 50, 3), 0, "nothing is painted before the first tick");
    assert_eq!(picture.pick(100, 50).drawing, 0);
    let mut ticks = 0;
    for step in [1, 1, 5, 13, 20, 19] {
        assert!(terminal.advance_ticks(step));
        ticks += step;
        terminal.compose(&screen, &mut picture);
        let painted = usize::try_from(ticks * 10).unwrap();
        assert_eq!(run(&picture, 50, 3), painted, "after {ticks} ticks");
        // The pick record follows the pixels painted so far.
        let last = 100 + u32::try_from(painted).unwrap() - 1;
        assert_eq!((picture.pick(last, 50).drawing, picture.pick(last + 1, 50).drawing), (1, 0));
        assert_eq!(seen(&picture), seen(&composed(&mut terminal, &screen)), "a partial repaint is the whole one");
    }
    assert!(terminal.advance_ticks(0));
    assert!(!terminal.advance_ticks(1), "the 60th tick pays for the 600th pixel");
    terminal.compose(&screen, &mut picture);
    assert_eq!(seen(&picture), seen(&whole));
    assert!(!terminal.advance_ticks(100));
}

#[test]
fn the_allowance_is_counted_from_the_total_of_ticks() {
    // T1 paints 100 units a second: 5 pixels every 3 ticks.
    let (mut terminal, screen, _) = fed("D(B1)W(I3)P[0,0]V[599,0]D(E)D(S1(T1),C)");
    let mut picture = Picture::new();
    for (ticks, painted) in [(1, 1), (2, 3), (3, 5), (4, 6), (60, 100)] {
        while terminal.commits() == 1 && run(&picture, 0, 3) < painted {
            let _ = terminal.advance_ticks(1);
            terminal.compose(&screen, &mut picture);
        }
        let (mut other, other_screen, _) = fed("D(B1)W(I3)P[0,0]V[599,0]D(E)D(S1(T1),C)");
        let _ = other.advance_ticks(ticks);
        assert_eq!(run(&composed(&mut other, &other_screen), 0, 3), painted, "after {ticks} ticks at once");
    }
}

#[test]
fn a_filled_area_appears_whole_once_its_cost_is_earned() {
    // A line of 100 pixels, then a fill of 20 by 20.
    let contents = format!("W(I3)P[0,30]V[99,30]{}", square(5));
    let (mut terminal, screen, _) = fed(&format!("D(B1){contents}D(E)D(S1(T6),C)"));
    let _ = terminal.advance_ticks(10);
    let picture = composed(&mut terminal, &screen);
    assert_eq!((run(&picture, 30, 3), pixel(&picture, 10, 10)), (100, 0));
    // The fill is not begun until all of it is paid for.
    let mut filled_at = None;
    for tick in 11..200 {
        let _ = terminal.advance_ticks(1);
        let picture = composed(&mut terminal, &screen);
        let inked = (0..20).map(|row| run(&picture, row, 5)).sum::<usize>();
        assert!(inked == 0 || inked == 400, "the fill appeared in part: {inked} pixels at tick {tick}");
        if inked == 400 {
            filled_at = Some(tick);
            break;
        }
    }
    assert!(filled_at.is_some_and(|tick| tick > 40), "a fill of 400 pixels costs at least 40 ticks: {filled_at:?}");
}

#[test]
fn children_wait_for_their_parent_then_start_at_their_own_rates() {
    // The parent: 60 pixels at T6, six ticks. Child 2 has a rate, child 3
    // none, grandchild 4 a rate.
    let line = |colour: u8| format!("W(I{colour})P[0,0]V[59,0]");
    let stream = format!(
        "D(B1){}D(E)D(B2){}D(E)D(B3){}D(E)D(B4){}D(E)D(S1(T6),S2(P1,Y10,T6),S3(P1,Y20),S4(P2,Y5,T12),C)",
        line(1),
        line(2),
        line(3),
        line(4)
    );
    let (mut terminal, screen, _) = fed(&stream);
    let rows = |terminal: &mut Terminal| {
        let picture = composed(terminal, &screen);
        [run(&picture, 0, 1), run(&picture, 10, 2), run(&picture, 20, 3), run(&picture, 15, 4)]
    };
    assert!(terminal.advance_ticks(5));
    assert_eq!(rows(&mut terminal), [50, 0, 0, 0]);
    // The parent finishes at tick 6; the child without a rate is whole.
    assert!(terminal.advance_ticks(1));
    assert_eq!(rows(&mut terminal), [60, 0, 60, 0]);
    assert!(terminal.advance_ticks(4));
    assert_eq!(rows(&mut terminal), [60, 40, 60, 0]);
    // Ticks left over when a drawing finishes go to its children: child 2
    // ends at tick 12, and the grandchild has had three of these five.
    assert!(!terminal.advance_ticks(5), "the grandchild's 60 pixels at T12 take three ticks");
    assert_eq!(rows(&mut terminal), [60, 60, 60, 60]);
    // In one call the same ticks give the same picture.
    let (mut other, _, _) = fed(&stream);
    assert!(other.advance_ticks(10));
    assert_eq!(rows(&mut other), [60, 40, 60, 0]);
}

#[test]
fn painting_starts_again_only_when_the_contents_the_rate_or_the_visibility_change() {
    let stream = "D(B1)W(I3)P[0,0]V[599,0]D(E)D(S1(T6),C)";
    let (mut terminal, mut screen, _) = fed(stream);
    assert!(!terminal.advance_ticks(1000));
    let whole = |terminal: &mut Terminal, screen: &Screen| run(&composed(terminal, screen), 50, 3) == 600;
    // Moving, clipping, reordering and re-parenting do not start it again.
    let _ = terminal.feed_regis(&mut screen, b"D(S1(Y50,L4,G2,C(X0,Y0,W700,H9)),S2(X1),S1(P2),C)");
    assert!(!terminal.advance_ticks(0));
    assert!(whole(&mut terminal, &screen));
    assert!(terminal.set_animation(1, AnimationState { position: Some((5, 50)), order: Some(3), ..NONE }));
    assert!(!terminal.advance_ticks(0));
    // The same rate again does not; another rate does.
    let _ = terminal.feed_regis(&mut screen, b"D(S1(T6),C)");
    assert!(!terminal.advance_ticks(0));
    let _ = terminal.feed_regis(&mut screen, b"D(S1(T7),C)");
    assert!(terminal.advance_ticks(0));
    assert!(!whole(&mut terminal, &screen));
    assert!(!terminal.advance_ticks(1000));
    // New contents do.
    let _ = terminal.feed_regis(&mut screen, b"D(B1)W(I3)P[0,0]V[599,0]D(E)D(C)");
    assert!(terminal.advance_ticks(0));
    assert!(!terminal.advance_ticks(1000));
    // Hidden, it is not painting; shown again, by the stream or by the
    // animation state, it starts.
    let _ = terminal.feed_regis(&mut screen, b"D(S1(V0),C)");
    assert!(!terminal.advance_ticks(0));
    let _ = terminal.feed_regis(&mut screen, b"D(S1(V1),C)");
    assert!(terminal.advance_ticks(0));
    assert!(!terminal.advance_ticks(1000));
    assert!(terminal.set_animation(1, AnimationState { shown: Some(false), ..NONE }));
    assert!(!terminal.advance_ticks(0));
    terminal.clear_animation(1);
    assert!(terminal.advance_ticks(0));
    assert!(!whole(&mut terminal, &screen));
    // A rate of 0 paints at once.
    let _ = terminal.feed_regis(&mut screen, b"D(S1(T0),C)");
    assert!(!terminal.advance_ticks(0));
    assert!(whole(&mut terminal, &screen));
}

#[test]
fn a_frame_drawn_in_a_drawings_place_is_painted_at_the_drawings_rate() {
    let stream = "D(B2)W(I5)P[0,0]V[599,0]D(E)D(S2(V0),S1(F(2),T6),C)";
    let (mut terminal, screen, _) = fed(stream);
    assert!(terminal.advance_ticks(30));
    assert_eq!(run(&composed(&mut terminal, &screen), 0, 5), 300);
    assert!(!terminal.advance_ticks(30));
    assert_eq!(run(&composed(&mut terminal, &screen), 0, 5), 600);
}

// ---- SOLVEDSECURITYISSUES 014 ----

#[test]
fn showing_hidden_drawings_cannot_take_a_repaint_over_the_cap() {
    // 300 hidden drawings that each fill the screen five times: the
    // commit counts none of them, and 64 of them shown would cost
    // 123,000,000 work units.
    let fill = "F(V[0,0][799,0][799,479][0,479])".repeat(5);
    let sets: Vec<String> = (10..310).map(|number| format!("S{number}(R1,V0)")).collect();
    let (mut terminal, mut screen, events) = fed(&format!("D(B1){fill}D(E)D(S1(V0),{},C)", sets.join(",")));
    assert_eq!(events, [Event::Committed(1)]);
    let show = AnimationState { shown: Some(true), position: Some((1, 1)), ..NONE };
    let mut kept = 0_u16;
    for number in 10..10 + u16::try_from(MAX_ANIMATED_DRAWINGS).unwrap() {
        if terminal.set_animation(number, show) {
            kept += 1;
        } else {
            // The rest of the state is kept.
            assert_eq!(terminal.animation(number), Some(AnimationState { shown: None, ..show }));
        }
    }
    assert!(kept > 0 && usize::from(kept) < MAX_ANIMATED_DRAWINGS, "{kept} were shown");
    let picture = composed(&mut terminal, &screen);
    assert!(picture.work() <= MAX_REBUILD_WORK, "{}", picture.work());
    // A commit that makes the shown drawings dearer takes shown parts away.
    let _ = terminal.feed_regis(&mut screen, format!("D(B1){fill}{fill}D(E)D(C)").as_bytes());
    let still = (10..74).filter(|number| terminal.animation(*number).unwrap().shown == Some(true)).count();
    assert!(still > 0 && still < usize::from(kept), "{still} of {kept} still shown");
    assert!(composed(&mut terminal, &screen).work() <= MAX_REBUILD_WORK);
}
