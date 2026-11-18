//! Damage repaint: a picture repainted where it changed is the picture
//! painted whole (`docs/regis-v2.md`, "Commit and repaint"; REQ-6329),
//! under any animation state, in every layer and part-way through gradual
//! painting too (animation.md, ANIM-9, ANIM-12 and ANIM-15; REQ-6530).

mod common;

use core::fmt::Write as _;

use regis::{AnimationState, ForcedLook, Picture, Screen, Terminal, ViewerState};

/// A linear congruential generator, so the tests need no library and
/// always run the same sequences.
struct Random(u64);

impl Random {
    fn below(&mut self, bound: u32) -> u32 {
        self.0 = self.0.wrapping_mul(6_364_136_223_846_793_005).wrapping_add(1_442_695_040_888_963_407);
        u32::try_from(self.0 >> 33).unwrap() % bound
    }

    fn pick<'a>(&mut self, from: &[&'a str]) -> &'a str {
        from[usize::try_from(self.below(u32::try_from(from.len()).unwrap())).unwrap()]
    }

    /// One of the few drawing numbers the sequences use, so that they
    /// collide: redefinitions, parents, sources and looks all meet.
    fn drawing(&mut self) -> u32 {
        1 + self.below(8)
    }

    fn place(&mut self) -> (i32, i32) {
        (i32::try_from(self.below(900)).unwrap() - 50, i32::try_from(self.below(560)).unwrap() - 40)
    }
}

/// Contents of every kind: lines, patterns, complement and erase writing,
/// shading, fills, text, curves, plane selection, a macrograph, and
/// positions far off the page.
const CONTENTS: [&str; 14] = [
    "W(I3)P[0,0]V[60,0][60,40][0,40][0,0]",
    "W(I5,P2)P[0,0]V[120,90]",
    "W(C,I7)P[5,5]V[80,5][80,60]",
    "W(E)P[0,0]V[40,40]",
    "W(I2,S1)P[0,30]V[70,30]",
    "W(I4,S1)P[10,0]V[10,50]W(S0)",
    "F(P[0,0]V[50,0][25,45])",
    "W(I6)F(C[+30])",
    "W(I9)T'Paint'",
    "W(I1,R)T(S2)'Aa'",
    "W(I11)P[20,20]C[+25]",
    "W(F3,I15)P[0,0]V[90,10]",
    "@AW(I8)P[-300,-200]V[400,300]",
    "W(I12,P4(M3))P[0,0]V[0,70][70,70]",
];

/// One random step of a stream: a definition, attributes, a deletion, base
/// drawing, a macrograph change, or nothing.
fn operation(random: &mut Random) -> String {
    let drawing = random.drawing();
    let other = random.below(9);
    let (x, y) = random.place();
    match random.below(20) {
        0..=2 => format!("D(B{drawing}){}D(E)", random.pick(&CONTENTS)),
        3..=5 => format!("D(S{drawing}(X{x},Y{y}))"),
        6 => format!("D(S{drawing}(P{other}))"),
        7 => format!("D(S{drawing}(R{other}))"),
        8 => format!("D(S{drawing}(H{other},A{}))", random.below(9)),
        9 => format!("D(S{drawing}(V{},I{},L{}))", random.below(2), random.below(2), random.below(5)),
        10 => format!(
            "D(S{drawing}(C(X{},Y{},W{},H{})))",
            random.below(30),
            random.below(30),
            random.below(90),
            random.below(90)
        ),
        11 => format!("D(S{drawing}(C0))"),
        12 => format!("D(K{drawing})"),
        13 => format!(
            "P[{},{}]W(I{})V[+{},+{}]",
            x.max(0),
            y.max(0),
            random.below(16),
            random.below(200),
            random.below(100)
        ),
        14 => format!("@:A{}@;", random.pick(&CONTENTS[..6])),
        // The attributes of animation (animation.md, ANIM-10): a layer,
        // an alternative look, a frame list and a paint rate.
        15 => format!("D(S{drawing}(G{}))", random.below(4)),
        16 => format!("D(S{drawing}(N{other}))"),
        17 if other == 0 => format!("D(S{drawing}(F0))"),
        17 => format!("D(S{drawing}(F({other},{})))", random.drawing()),
        18 => format!("D(S{drawing}(T{}))", random.below(3) * (1 + random.below(40))),
        _ => "D(Z)".to_string(),
    }
}

/// A random animation state.
fn animation(random: &mut Random) -> AnimationState {
    let small = |random: &mut Random, bound: u32| u8::try_from(random.below(bound)).unwrap();
    AnimationState {
        shown: (random.below(3) == 0).then(|| random.below(2) == 0),
        position: (random.below(2) == 0).then(|| {
            let (x, y) = random.place();
            (i16::try_from(x).unwrap(), i16::try_from(y).unwrap())
        }),
        layer: (random.below(3) == 0).then(|| small(random, 4)),
        order: (random.below(3) == 0).then(|| i8::try_from(random.below(5)).unwrap()),
        frame: small(random, 3),
        look: ForcedLook::from_code(small(random, 4)).unwrap(),
    }
}

/// The pixels, the pick record, the origins and the commit count.
type Seen<'a> = (&'a [u8], &'a [u16], Vec<(u16, i32, i32)>, u32);

/// Everything a caller can read of a picture but its work, which a
/// partial repaint is meant to lower.
fn seen(picture: &Picture) -> Seen<'_> {
    (picture.canvas().pixels(), picture.pick_record(), picture.origins().collect(), picture.commits())
}

/// Runs random streams, composing after every commit, every base command
/// and every change of viewer state into one picture kept throughout, and
/// compares it each time with a picture painted whole.
fn compare(seed: u64, streams: u32, steps: u32) {
    let mut random = Random(seed);
    let (mut partial, mut whole_repaints) = (0_u32, 0_u32);
    for stream in 0..streams {
        let mut terminal = common::terminal();
        let mut screen = Screen::new();
        let mut kept = Picture::new();
        for step in 0..steps {
            let mut bytes = String::new();
            for _ in 0..=random.below(4) {
                bytes.push_str(&operation(&mut random));
            }
            if random.below(3) != 0 {
                bytes.push_str("D(C)");
            }
            let _ = terminal.feed_regis(&mut screen, bytes.as_bytes());
            if random.below(2) == 0 {
                let mut number = || u16::try_from(random.below(9)).unwrap();
                let _ =
                    terminal.set_viewer_state(ViewerState { hovered: number(), pressed: number(), hidden: number() });
            }
            // The receiver's animation state and its clock (ANIM-9,
            // ANIM-15): neither is in the stream.
            if random.below(2) == 0 {
                let number = u16::try_from(random.drawing()).unwrap();
                let _ = terminal.set_animation(number, animation(&mut random));
            }
            if random.below(8) == 0 {
                terminal.clear_animation(u16::try_from(random.drawing()).unwrap());
            }
            if random.below(2) == 0 {
                let _ = terminal.advance_ticks(random.below(40));
            }
            terminal.compose(&screen, &mut kept);
            let mut fresh = Picture::new();
            terminal.compose(&screen, &mut fresh);
            assert!(seen(&kept) == seen(&fresh), "seed {seed:#x}, stream {stream}, step {step}, after {bytes}");
            assert!(kept.work() <= fresh.work(), "a partial repaint costs no more than a whole one");
            if kept.work() < fresh.work() {
                partial += 1;
            } else {
                whole_repaints += 1;
            }
        }
    }
    // The comparison means something only if partial repaints happened.
    assert!(partial > whole_repaints / 4, "{partial} partial repaints, {whole_repaints} whole");
}

#[test]
fn a_picture_repainted_where_it_changed_is_the_picture_painted_whole() {
    compare(0x2545_F491_4F6C_DD1D, 12, 60);
}

#[test]
#[ignore = "the long run of the full gate: scripts/check.sh full"]
fn a_picture_repainted_where_it_changed_is_the_picture_painted_whole_at_length() {
    compare(0x9E37_79B9_7F4A_7C15, 400, 120);
}

fn pixel(picture: &Picture, x: u32, y: u32) -> u8 {
    picture.canvas().pixel(x, y).unwrap()
}

#[test]
fn moving_one_small_drawing_repaints_only_what_it_covered_and_covers() {
    // A background over the whole screen and a small square over it.
    let background = "W(I2)F(P[0,0]V[0,0][799,0][799,479][0,479])";
    let square = "W(I3)P[0,0]V[19,0][19,19][0,19][0,0]";
    let mut terminal = Terminal::new();
    let mut screen = Screen::new();
    let _ =
        terminal.feed_regis(&mut screen, format!("D(B1){background}D(E)D(B2){square}D(E)D(S2(X100,Y50),C)").as_bytes());
    let mut picture = Picture::new();
    terminal.compose(&screen, &mut picture);
    let whole = picture.work();
    assert_eq!((pixel(&picture, 100, 50), pixel(&picture, 105, 55), pixel(&picture, 400, 300)), (3, 2, 2));

    let _ = terminal.feed_regis(&mut screen, b"D(S2(X110),C)");
    terminal.compose(&screen, &mut picture);
    assert_eq!((pixel(&picture, 100, 50), pixel(&picture, 110, 50), pixel(&picture, 129, 69)), (2, 3, 3));
    assert_eq!(picture.changed_rows(), Some((50, 69)));
    // The background is replayed, cut to the square's old and new places,
    // and only that part of it is paid for.
    let moved = picture.work();
    assert!(moved < whole / 50, "{moved} against {whole}");

    // A drawing the change does not touch is not replayed at all.
    let _ = terminal.feed_regis(&mut screen, b"D(B3)W(I4)F(P[0,0]V[0,0][99,0][99,99][0,99])D(E)D(S3(X600,Y300),C)");
    terminal.compose(&screen, &mut picture);
    assert_eq!(pixel(&picture, 650, 350), 4);
    let _ = terminal.feed_regis(&mut screen, b"D(S2(X120),C)");
    terminal.compose(&screen, &mut picture);
    assert_eq!(picture.work(), moved + 64, "the visit to drawing 3 is paid for and nothing more of it");
    let mut fresh = Picture::new();
    terminal.compose(&screen, &mut fresh);
    assert!(fresh.work() > whole + 10_000, "painted whole, drawing 3 costs its area: {}", fresh.work());
    assert_eq!(picture.canvas(), fresh.canvas());

    // Nothing changed: nothing is repainted.
    terminal.compose(&screen, &mut picture);
    assert_eq!(picture.changed_rows(), None);
    assert_eq!(picture.work(), 3 * 64, "only the visits to the three drawings are paid for");
}

#[test]
fn a_change_to_the_base_is_repainted_under_the_drawings() {
    let mut terminal = Terminal::new();
    let mut screen = Screen::new();
    let _ = terminal.feed_regis(&mut screen, b"D(B1)W(I3)P[0,0]V[19,0]D(E)D(S1(X100,Y50),C)");
    let mut picture = Picture::new();
    terminal.compose(&screen, &mut picture);
    let _ = terminal.feed_regis(&mut screen, b"W(I9)P[90,50]V[200,50]P[300,400]V[310,400]");
    terminal.compose(&screen, &mut picture);
    assert_eq!((pixel(&picture, 95, 50), pixel(&picture, 110, 50), pixel(&picture, 150, 50)), (9, 3, 9));
    assert_eq!(pixel(&picture, 305, 400), 9);
    assert_eq!(picture.changed_rows(), Some((50, 400)));
}

/// Two changes far apart in one repaint, as a key pressed at one side of
/// the music player's picture and a meter lit at the other
/// (page-rules.md RULE-65, RULE-92), repaint those two places and replay
/// nothing of the drawings between them.
#[test]
fn two_changes_far_apart_repaint_two_places_and_nothing_between() {
    let mut terminal = Terminal::new();
    let mut screen = Screen::new();
    // Ten filled squares 40 pixels wide, 70 apart.
    let mut bytes = String::new();
    for number in 1..=10 {
        let x = 20 + 70 * (number - 1);
        let commit = if number == 10 { ",C" } else { "" };
        write!(bytes, "D(B{number})W(I3)F(P[0,0]V[0,0][39,0][39,39][0,39])D(E)D(S{number}(X{x},Y100){commit})")
            .unwrap();
    }
    let _ = terminal.feed_regis(&mut screen, bytes.as_bytes());
    let mut picture = Picture::new();
    terminal.compose(&screen, &mut picture);
    let whole = picture.work();
    // The first and the last move down a pixel.
    let _ = terminal.feed_regis(&mut screen, b"D(S1(X20,Y101))D(S10(X650,Y101),C)");
    terminal.compose(&screen, &mut picture);
    let moved = picture.work();
    assert!(moved * 4 < whole, "two squares' places, not the eight between: {moved} against {whole}");
    let mut fresh = Picture::new();
    terminal.compose(&screen, &mut fresh);
    assert_eq!(picture.canvas(), fresh.canvas());
    assert_eq!((pixel(&picture, 30, 100), pixel(&picture, 30, 120), pixel(&picture, 660, 120)), (0, 3, 3));
}
