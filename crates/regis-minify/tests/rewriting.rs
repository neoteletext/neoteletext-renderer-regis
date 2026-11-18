//! What the rewriter writes, and that it draws the same.

use std::path::PathBuf;

use regis::{Picture, Screen, Terminal};
use regis_minify::{Level, minify};

fn short(input: &str) -> String {
    String::from_utf8(minify(input.as_bytes(), Level::Joined)).unwrap()
}

fn exact(input: &str) -> String {
    String::from_utf8(minify(input.as_bytes(), Level::Exact)).unwrap()
}

/// Both pages and the colours a stream leaves behind.
fn drawn(input: &[u8]) -> (Vec<Vec<u8>>, Vec<[u8; 3]>) {
    let mut terminal = Terminal::new();
    let mut screen = Screen::new();
    let framed = input.windows(2).any(|pair| pair == [0x1B, b'P']);
    let _ = if framed { terminal.feed(&mut screen, input) } else { terminal.feed_regis(&mut screen, input) };
    let _ = terminal.finish(&mut screen);
    let pages = (0..2).filter_map(|number| screen.page(number).map(|page| page.pixels().to_vec())).collect();
    (pages, terminal.palette().colours().to_vec())
}

#[test]
fn blanks_and_newlines_between_commands_go() {
    assert_eq!(short("P[10,20]\n V[30,40]\r\n"), "P[10,20]V[30,40]");
}

#[test]
fn each_coordinate_takes_its_shortest_form() {
    assert_eq!(short("P[100,100]V[102,100][102,103][400,103]"), "P[100,100]V[+2][,+3][400]");
    assert_eq!(short("P[100,100]V[+0,+5]"), "P[100,100]V[,+5]");
}

#[test]
fn a_position_that_is_not_known_keeps_relative_moves_relative() {
    assert_eq!(short("V[+5,+0][+3,-2]"), "V[+5][+3,-2]");
    assert_eq!(short("T'a'V[+5,20]"), "T'a'V[+5,20]");
}

#[test]
fn a_setting_already_in_force_is_left_out() {
    assert_eq!(short("W(I3)P[1,1]V[2,2]W(I3)V[3,3]"), "W(I3)P[1,1]V[2,2][3,3]");
    assert_eq!(short("W(I3P1)V[]W(I4P1)V[]"), "W(I3P1)V[]W(I4)V[]");
}

#[test]
fn a_setting_replaced_before_anything_is_drawn_is_left_out() {
    assert_eq!(short("W(I3)W(I4)P[1,1]W(P2)V[2,2]"), "W(I4)P[1,1]W(P2)V[2,2]");
    assert_eq!(short("W(I3)P[1,1]W(I4,P2)V[2,2]"), "P[1,1]W(I4P2)V[2,2]");
}

#[test]
fn shading_is_never_left_out_because_it_reads_the_position() {
    assert_eq!(short("W(S1)P[1,1]W(S1)V[2,2]"), "W(S1)P[1,1]W(S1)V[2,2]");
}

#[test]
fn moves_that_follow_moves_become_one() {
    assert_eq!(short("P[5,5]P[10,10]V[]"), "P[10,10]V[]");
    assert_eq!(short("P[+5]P[+5,+2]V[]"), "P[+5][+5,+2]V[]");
}

#[test]
fn a_move_to_where_the_position_already_is_goes_only_when_joining() {
    assert_eq!(short("P[1,1]V[5,5]P[5,5]V[9,9]"), "P[1,1]V[5,5][9,9]");
    assert_eq!(exact("P[1,1]V[5,5]P[5,5]V[9,9]"), "P[1,1]V[5,5]P[]V[9,9]");
}

#[test]
fn text_strings_macrographs_and_load_commands_are_copied_as_written() {
    assert_eq!(short("T(S2) 'a  b' P[1, 2]"), "T(S2)'a  b'P[1,2]");
    assert_eq!(short("@:A V[ +1 ] @;P[1,1]@A V[2,2]"), "@:A V[ +1 ] @;P[1,1]@AV[2,2]");
    assert_eq!(short("L(A1)\"A\"AA, 55;P[1,1]"), "L(A1)\"A\"AA, 55;P[1,1]");
}

#[test]
fn nothing_is_assumed_after_a_macrograph_runs() {
    assert_eq!(short("W(I3)P[1,1]@AW(I3)V[1,1]"), "W(I3)P[1,1]@AW(I3)V[1,1]");
}

#[test]
fn a_number_that_is_not_a_whole_coordinate_is_left_alone() {
    assert_eq!(short("P[1.5,2]V[+3]"), "P[1.5,2]V[+3]");
}

#[test]
fn only_regis_inside_a_data_stream_is_rewritten() {
    assert_eq!(short("a b\x1bP0pP[1, 2]\x1b\\c d"), "a b\x1bP0pP[1,2]\x1b\\c d");
}

#[test]
fn every_sample_draws_the_same_after_rewriting() {
    let data = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../regis/tests/data");
    let mut checked = 0;
    for entry in std::fs::read_dir(data).unwrap() {
        let path = entry.unwrap().path();
        if path.extension().is_none_or(|extension| extension != "regis") {
            continue;
        }
        let input = std::fs::read(&path).unwrap();
        let original = drawn(&input);
        let rewritten = minify(&input, Level::Exact);
        assert!(rewritten.len() <= input.len(), "{} grew", path.display());
        assert!(drawn(&rewritten) == original, "{} draws differently", path.display());
        checked += 1;
    }
    assert!(checked >= 5, "the sample files were not found");
}

#[test]
fn rewriting_what_is_already_short_changes_nothing() {
    let once = minify(b"W(I3)P[100,100]V[102,100][102,103]T'x'P[5,5]", Level::Joined);
    assert_eq!(minify(&once, Level::Joined), once);
}

/// The picture with its numbered drawings, each painted to its end.
fn composed(input: &str) -> (Vec<u8>, Vec<u16>) {
    let mut terminal = Terminal::new();
    let mut screen = Screen::new();
    let _ = terminal.feed_regis(&mut screen, input.as_bytes());
    let _ = terminal.finish(&mut screen);
    let _ = terminal.advance_ticks(u32::MAX);
    let mut picture = Picture::new();
    terminal.compose(&screen, &mut picture);
    (picture.canvas().pixels().to_vec(), picture.pick_record().to_vec())
}

#[test]
fn drawing_commands_pass_through_with_every_attribute() {
    // The attributes of animation among them (animation.md, ANIM-10).
    let commands = "D(S40(G2,X100,Y-240,I1,F(41,42,43,44),N45,T50,H1,A2,R3,L-4,V0,P9,C(X1,Y2,W3,H4)),S41(F0),K7,Z,C)";
    assert_eq!(short(commands), commands);
    assert_eq!(exact(commands), commands);
}

#[test]
fn nothing_known_outside_a_numbered_drawing_holds_inside_it_or_after_it() {
    // A drawing's contents start from a fresh context: the colour and
    // the position set before it are not theirs.
    let input = "W(I3)P[5,5]D(B1)W(I3)P[5,5]V[50,5]D(E)D(S1(I1),C)P[5,5]V[9,9]";
    assert_eq!(short(input), "W(I3)P[5,5]D(B1)W(I3)P[5,5]V[50]D(E)D(S1(I1),C)P[5,5]V[9,9]");
    assert_eq!(composed(&short(input)), composed(input));
    assert_eq!(drawn(short(input).as_bytes()), drawn(input.as_bytes()));
}

#[test]
fn the_animation_example_is_composed_the_same() {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../docs/examples/animation.regis");
    let input = std::fs::read_to_string(path).unwrap();
    let rewritten = short(&input);
    assert!(rewritten.len() < input.len());
    assert!(rewritten.contains("D(S40(G2,X100,Y240,I1,F(41,42,43,44),N45))"));
    assert!(rewritten.contains("D(S7(G1,X250,Y40,T50))"));
    assert_eq!(composed(&rewritten), composed(&input));
}

/// The labels and the title a stream commits (DEC-6638 items 3 and 6).
fn spoken(input: &str) -> (Vec<(u16, String)>, Option<String>) {
    let mut terminal = Terminal::new();
    let mut screen = Screen::new();
    let _ = terminal.feed_regis(&mut screen, input.as_bytes());
    let _ = terminal.finish(&mut screen);
    (terminal.labels(), terminal.title())
}

#[test]
fn labels_and_titles_pass_through_as_written() {
    // A label and a title are quoted strings: blanks and doubled quotes
    // inside them are theirs, and are kept.
    let commands = "D(S40(I1,U'Next  page'),S41(U\"Say \"\"hi\"\"\"),T'Harbour  webcam',C)";
    assert_eq!(short(commands), commands);
    assert_eq!(exact(commands), commands);
    let input = "P[5,5]V[50,5] D(B3)P[0,0]V[19,0]D(E) D(S3(X10, U'Play'), T'Arcade', C)";
    let rewritten = short(input);
    assert!(rewritten.len() < input.len());
    assert_eq!(spoken(&rewritten), (vec![(3, "Play".to_owned())], Some("Arcade".to_owned())));
    assert_eq!(spoken(&rewritten), spoken(input));
    assert_eq!(composed(&rewritten), composed(input));
}
