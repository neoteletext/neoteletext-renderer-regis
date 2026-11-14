//! The load, macrograph and report commands (manual chapters 8 to 10).

mod common;

use common::draw;
use regis::Event;

#[test]
fn a_loaded_character_is_drawn_from_its_hex_rows() {
    // A cell whose top row is all on and second row half on.
    let drawn = draw("L(A1)'a'FF,F0;P[0,0]T(A1,S0)'a'");
    let row = |y| (0..8).map(|x| drawn.at(x, y)).collect::<Vec<u8>>();
    assert_eq!(row(0), [7; 8]);
    assert_eq!(row(1), [7, 7, 7, 7, 0, 0, 0, 0]);
    assert_eq!(row(2), [0; 8]);
}

#[test]
fn hex_digits_pair_up_and_a_single_digit_is_the_right_half() {
    // "If you use only one hex value … the first hex value is 0."
    let drawn = draw("L(A2)'b'F,81C3;P[0,0]T(A2,S0)'b'");
    let row = |y| (0..8).map(|x| u8::from(drawn.at(x, y) != 0)).collect::<Vec<u8>>();
    assert_eq!(row(0), [0, 0, 0, 0, 1, 1, 1, 1]);
    assert_eq!(row(1), [1, 0, 0, 0, 0, 0, 0, 1]);
    assert_eq!(row(2), [1, 1, 0, 0, 0, 0, 1, 1]);
}

#[test]
fn a_character_not_loaded_is_the_error_block_and_set_0_cannot_be_loaded() {
    let drawn = draw("P[0,0]T(A1,S0)'z'");
    assert_eq!(drawn.inked(), 80);
    let drawn = draw("L(A0)'A'FF;P[0,0]T(S0)'A'");
    assert!(drawn.inked() < 40);
}

#[test]
fn the_set_name_is_reported_and_cut_to_ten_characters() {
    assert_eq!(draw("L(A3'alpha')R(L)").replies(), ["A'alpha'\r"]);
    assert_eq!(draw("L(A1'abcdefghijklmnop')R(L)").replies(), ["A'abcdefghij'\r"]);
    // Manual table 10-1: error 4 for a set number out of range.
    assert_eq!(draw("L(A7)R(E)").replies(), ["\"4,0\"\r"]);
}

#[test]
fn a_macrograph_is_stored_then_drawn_wherever_it_is_invoked() {
    let drawn = draw("@:TV[+10]P[+5]@;P[0,0]@T@tR(P)");
    assert_eq!(drawn.replies(), ["[30,0]\r"]);
    assert_eq!(drawn.inked(), 22);
    // Defining draws nothing.
    assert_eq!(draw("@:TV[+10]@;").inked(), 0);
}

#[test]
fn a_macrograph_can_supply_part_of_a_command() {
    let drawn = draw("@:C[20,30@;P@C]R(P)");
    assert_eq!(drawn.replies(), ["[20,30]\r"]);
}

#[test]
fn macrographs_are_cleared_singly_or_all_at_once() {
    assert_eq!(draw("@:AP[5,5]@;@:A@;P[1,1]@AR(P)").replies(), ["[1,1]\r"]);
    assert_eq!(draw("@:AP[5,5]@;@:BP[6,6]@;@.P[1,1]@A@BR(P)").replies(), ["[1,1]\r"]);
}

#[test]
fn at_signs_and_semicolons_in_strings_are_text() {
    let drawn = draw("@:AP[5,5]@;P[0,0]T'@A;'R(P)");
    assert_eq!(drawn.replies(), ["[27,0]\r"]);
}

#[test]
fn macrograph_contents_are_reported_only_when_allowed() {
    let run = |allowed: bool| {
        let mut terminal = common::terminal();
        terminal.allow_macrograph_reports(allowed);
        let mut screen = regis::Screen::new();
        terminal.feed_regis(&mut screen, b"@:XP[1,2]@;R(M(x))")
    };
    assert_eq!(run(false), [Event::Reply(b"@=X@;\r".to_vec())]);
    assert_eq!(run(true), [Event::Reply(b"@=XP[1,2]@;\r".to_vec())]);
}

#[test]
fn macrograph_storage_is_reported_as_free_and_total() {
    assert_eq!(draw("R(M(=))").replies(), ["\"16384,16384\"\r"]);
    assert_eq!(draw("@:A12345@;R(M(=))").replies(), ["\"16379,16384\"\r"]);
}

#[test]
fn the_error_report_names_the_last_fault_and_a_semicolon_clears_it() {
    assert_eq!(draw("R(E)").replies(), ["\"0,0\"\r"]);
    assert_eq!(draw("P[1,2,3]R(E)").replies(), ["\"3,0\"\r"]);
    assert_eq!(draw("Z R(E)").replies(), ["\"1,90\"\r"]);
    assert_eq!(draw("Z;R(E)").replies(), ["\"0,0\"\r"]);
}

#[test]
fn several_reports_in_one_command_are_each_answered() {
    assert_eq!(draw("P[3,4]R(P,E)").replies(), ["[3,4]\r", "\"0,0\"\r"]);
}

#[test]
fn input_modes_answer_with_a_carriage_return_and_the_locator_is_the_callers() {
    assert_eq!(draw("R(I0)").events, [Event::Reply(b"\r".to_vec())]);
    assert_eq!(draw("R(I)").events, [Event::Reply(b"\r".to_vec())]);
    assert_eq!(draw("R(P(I))").events, [Event::LocatorRequest]);
}

#[test]
fn the_shipped_greek_set_loads_and_draws() {
    let set = std::fs::read_to_string(common::data("../../../../fonts/loadable/greek.regis")).unwrap();
    let drawn = draw(&format!("{set}T(A1)R(L)P[100,100]T(S1)'A'"));
    assert_eq!(drawn.replies(), ["A'greek'\r"]);
    // Alpha's second row is one pixel, 0x08, doubled in height at size 1.
    let row = |y: u32| (100..108).map(|x| drawn.at(x, y)).collect::<Vec<u8>>();
    assert_eq!(row(100), [0; 8]);
    assert_eq!(row(102), [0, 0, 0, 0, 7, 0, 0, 0]);
}
