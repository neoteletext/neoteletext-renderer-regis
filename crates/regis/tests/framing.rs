//! Device control strings and the whole-terminal behaviour (manual
//! chapter 1, "Entering and Exiting ReGIS").

mod common;

use common::draw_stream;

#[test]
fn only_what_is_inside_a_regis_control_string_draws() {
    let drawn = draw_stream(b"P[0,0]V[9,0]\x1bPpP[0,5]V[9,5]\x1b\\P[0,9]V[9,9]");
    assert_eq!(drawn.inked(), 10);
    assert_eq!(drawn.at(4, 5), 7);
}

#[test]
fn state_carries_over_from_one_string_to_the_next() {
    let drawn = draw_stream(b"\x1bPpW(I3)P[5,5]\x1b\\\x1bPpV[]\x1b\\");
    assert_eq!(drawn.at(5, 5), 3);
    // Mode 0 resumes even in the middle of a command.
    let drawn = draw_stream(b"\x1bPpP[5\x1b\\\x1bP0p,7]V[]\x1b\\");
    assert_eq!(drawn.at(5, 7), 7);
}

#[test]
fn mode_one_begins_at_a_new_command_but_keeps_the_picture_and_settings() {
    let drawn = draw_stream(b"\x1bPpW(I3)P[5,5]V[]P[9\x1b\\\x1bP1p,7]V[]\x1b\\");
    // The unfinished P[9 is dropped; ",7]" belongs to no command.
    assert_eq!(drawn.at(5, 5), 3);
    assert_eq!(drawn.inked(), 1);
    assert_eq!(drawn.terminal.position(), (5, 5));
}

#[test]
fn eight_bit_introducers_and_sixel_strings_are_handled() {
    let mut terminal = common::terminal();
    terminal.recognise_eight_bit_controls(true);
    let mut screen = regis::Screen::new();
    let _ = terminal.feed(&mut screen, b"\x90pP[1,1]V[]\x9c\x1bPq#0!100~\x1b\\\x1bPpP[3,3]V[]\x1b\\");
    assert_eq!(screen.displayed().pixels().iter().filter(|pixel| **pixel != 0).count(), 2);
}

#[test]
fn case_and_layout_do_not_matter() {
    let tidy = draw_stream(b"\x1bPpP[10,10]V[20,10]W(I2)V[20,20]\x1b\\");
    let messy = draw_stream(b"\x1bPp p [ 10 , 10 ]\r\n\tv[20,10]\n w ( i2 ) v [20,20]\x1b\\");
    let first_difference =
        tidy.screen.displayed().pixels().iter().zip(messy.screen.displayed().pixels()).position(|(a, b)| a != b);
    assert_eq!(first_difference, None);
}

#[test]
fn the_manuals_combined_example_draws_a_graph() {
    // Manual figure 12-1, abridged: axes, tick macrographs and labels.
    let example = "S(M0(L0)1(L33)2(L67)3(L100))(I3,E)W(I0,P1(M1),F3,V,S0)\
        @:TV[-10]P[+10,+50]@;@:MV[,+10]P[+100,-10]@;\
        P[100,100]V[,+300][+500]P[100,150]@T@T@T@T@T\
        P[60,140]T(A0,S1,I0)(D0,S1,D0)T'100'P[60,+50]T' 80'P[225,400]@M@M@M@M\
        P[350,450]T'QUARTER'";
    let drawn = common::draw(example);
    // The page is white (3) with black (0) ink.
    assert_eq!(drawn.at(0, 0), 3);
    assert_eq!((drawn.at(100, 250), drawn.at(300, 400), drawn.at(95, 150), drawn.at(225, 405)), (0, 0, 0, 0));
    assert_eq!(drawn.replies(), Vec::<String>::new());
}
