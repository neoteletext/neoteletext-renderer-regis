//! Damage repaint: a picture repainted where it changed is the picture
//! painted whole (`docs/regis-v2.md`, "Commit and repaint"; REQ-6329).

mod common;

use regis::{Picture, Screen, Terminal, ViewerState};

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
    match random.below(16) {
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
        _ => "D(Z)".to_string(),
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
