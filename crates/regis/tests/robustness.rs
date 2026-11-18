//! Hostile and random input: the renderer must never panic, and its work
//! must stay in proportion to the input.

mod common;

use std::fmt::Write as _;

use regis::{Event, Screen};

/// A small deterministic generator (xorshift64*), so failures reproduce.
struct Random(u64);

impl Random {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 >> 12;
        self.0 ^= self.0 << 25;
        self.0 ^= self.0 >> 27;
        self.0.wrapping_mul(0x2545_F491_4F6C_DD1D)
    }

    fn below(&mut self, limit: usize) -> usize {
        usize::try_from(self.next() % u64::try_from(limit).unwrap()).unwrap()
    }
}

/// Bytes weighted toward what ReGIS gives meaning to.
fn random_page(random: &mut Random, length: usize) -> Vec<u8> {
    const LIKELY: &[u8] = b"SWPVCTLRF()[],;@:.'\"0123456789+-ABEHIMNXDUabcdef \n\x1b\\P p";
    let mut page = Vec::with_capacity(length);
    while page.len() < length {
        if random.below(64) == 0 {
            // An alphabet selection or load around the provided alphabets
            // 100 to 154, past them, or among the VT340's own (PAR-12).
            let alphabet = random.below(160);
            let command = if random.below(4) == 0 { "L" } else { "T" };
            page.extend(format!("{command}(A{alphabet})").bytes());
        } else if random.below(8) == 0 {
            page.push(u8::try_from(random.below(256)).unwrap());
        } else {
            page.push(LIKELY[random.below(LIKELY.len())]);
        }
    }
    page
}

/// A terminal with the shipped font and two provided sets, one of them
/// joined, so that random alphabet selections draw through both paths.
fn terminal_with_provided_sets() -> regis::Terminal {
    use regis::{JoinedForm, JoiningType, ProvidedSet};
    let mut terminal = common::terminal();
    let mut plain = ProvidedSet::new(12, 20).unwrap();
    let mut joined = ProvidedSet::new(16, 32).unwrap();
    for code in 0x20..=0x7F_u8 {
        plain.set_glyph(code, &[0x0F0F; 20]).unwrap();
        joined.set_glyph(code, &[0xF00F; 32]).unwrap();
        let kind =
            [JoiningType::None, JoiningType::Right, JoiningType::Dual, JoiningType::Transparent][usize::from(code % 4)];
        joined.set_joining(code, kind).unwrap();
        joined.set_form(code, JoinedForm::Medial, &[0xFFFF; 32]).unwrap();
    }
    terminal.set_provided_set(0, plain).unwrap();
    terminal.set_provided_set(44, joined).unwrap();
    terminal
}

fn feed_random_pages(pages: usize, seed: u64) {
    let mut random = Random(seed);
    for _ in 0..pages {
        let length = random.below(3000);
        let page = random_page(&mut random, length);
        let mut terminal = terminal_with_provided_sets();
        terminal.allow_macrograph_reports(true);
        let mut screen = Screen::new();
        for chunk in page.chunks(37) {
            let _ = terminal.feed_regis(&mut screen, chunk);
            let _ = terminal.feed(&mut screen, chunk);
        }
        let _ = terminal.finish(&mut screen);
        assert!(screen.displayed().pixels().iter().all(|pixel| *pixel < 16));
    }
}

#[test]
fn random_input_never_panics_and_never_writes_a_pixel_out_of_range() {
    feed_random_pages(300, 0x9E37_79B9_7F4A_7C15);
}

#[test]
#[ignore = "slow: thousands of random pages, run by the full gate in a debug build so overflow would panic"]
fn many_random_pages_never_panic() {
    feed_random_pages(2000, 0x0123_4567_89AB_CDEF);
}

/// Draws commands and says whether a limit was reported.
fn limited(commands: &str) -> bool {
    common::draw(commands).events.contains(&Event::LimitReached)
}

#[test]
fn a_short_stream_cannot_buy_unbounded_drawing() {
    // Each shaded vector here covers most of the screen; four bytes repeat.
    let hostile = format!("W(S1[,0])P[0,479]W(M800)V{}", "0404".repeat(20_000));
    let started = std::time::Instant::now();
    assert!(limited(&hostile));
    assert!(started.elapsed().as_secs() < 20, "the work allowance did not bound the drawing");
}

#[test]
fn enormous_text_cells_and_coordinates_are_bounded() {
    let hostile = format!("T(S[99999,99999]U[99999,99999]){}", "'MMMMMMMM'".repeat(2000));
    assert!(limited(&hostile));
    // Far-away coordinates are clamped, not wrapped.
    let drawn = common::draw("P[1E30,-1E30]V[-99999999,99999999]R(P)");
    assert_eq!(drawn.replies(), ["[-1,32767]\r"]);
    let drawn = common::draw("P[-1E30,1E30]V[-99999999,+99999999]R(P)");
    assert_eq!(drawn.replies(), ["[-32768,32767]\r"]);
}

#[test]
fn macrographs_that_call_each_other_stop() {
    let drawn = common::draw("@:AV[+1]@B@;@:BV[,+1]@A@;P[10,10]@A R(P)");
    assert_eq!(drawn.replies(), ["[11,11]\r"]);
    // A chain of doubling macrographs is cut off by the work allowance.
    let mut bomb = String::from("@:AV[+1]V[-1]@;");
    for (outer, inner) in ('B'..='P').zip('A'..='O') {
        write!(bomb, "@:{outer}@{inner}@{inner}@{inner}@{inner}@;").unwrap();
    }
    bomb.push_str(&"@P".repeat(50));
    assert!(limited(&bomb));
}

#[test]
fn deep_nesting_and_unterminated_tokens_are_survived() {
    let deep = "V".to_owned() + &"(".repeat(10_000) + &")".repeat(10_000) + "P[5,5]V[6,5]";
    assert_eq!(common::draw(&deep).at(6, 5), 7);
    for unterminated in ["P[100", "T'abc", "W(I(H120", "@:A P[1,1]", "L\"A\"FF", "P[1,2]V[3,4]W(P"] {
        common::draw(unterminated);
    }
}

#[test]
fn a_macrograph_definition_larger_than_the_store_is_dropped() {
    let huge = format!("@:A{}@;@:BP[7,7]@;@A@B R(P)", "V[+1]".repeat(10_000));
    assert_eq!(common::draw(&huge).replies(), ["[7,7]\r"]);
}
