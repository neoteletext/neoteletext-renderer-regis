//! The position and vector commands (manual chapters 4 and 5).

mod common;

use common::draw;

#[test]
fn a_vector_draws_from_the_position_to_the_point_in_the_foreground_colour() {
    let drawn = draw("P[100,100]V[200,100]");
    assert_eq!((drawn.at(100, 100), drawn.at(150, 100), drawn.at(200, 100)), (7, 7, 7));
    assert_eq!((drawn.at(99, 100), drawn.at(201, 100)), (0, 0));
    assert_eq!(drawn.inked(), 101);
}

#[test]
fn coordinates_may_be_absolute_relative_or_one_of_each() {
    // Manual figures 4-1 to 4-3.
    let position = |commands: &str| draw(&format!("{commands}R(P)")).replies().remove(0);
    assert_eq!(position("P[200,200][300][,400]"), "[300,400]\r");
    assert_eq!(position("P[100,100][+100,+50][+50,-100]"), "[250,50]\r");
    assert_eq!(position("P[100,300][300,-100][+100,50]"), "[400,50]\r");
    assert_eq!(position("P[6.25,10.4][.1E3,1000E-1]"), "[100,100]\r");
}

#[test]
fn pixel_vectors_move_in_eight_directions_by_the_multiplier() {
    // Manual figure 4-5.
    let drawn = draw("P[400,200]P(W(M100))77 6 444 11R(P)");
    assert_eq!(drawn.replies(), ["[500,300]\r"]);
    // Manual figure 1-4: the multiplier lasts when set by a write command.
    let drawn = draw("P[400,100]W(M100)V642446064600206R(P)");
    assert_eq!(drawn.replies(), ["[400,400]\r"]);
    assert_eq!(drawn.at(400, 150), 7);
}

#[test]
fn a_temporary_multiplier_ends_at_the_next_key_letter() {
    let drawn = draw("P[100,100]P(W(M50))0P0R(P)");
    assert_eq!(drawn.replies(), ["[151,100]\r"]);
}

#[test]
fn an_empty_bracket_draws_a_dot_and_does_not_move() {
    let drawn = draw("P[10,10]V[]R(P)");
    assert_eq!(drawn.inked(), 1);
    assert_eq!(drawn.at(10, 10), 7);
    assert_eq!(drawn.replies(), ["[10,10]\r"]);
}

#[test]
fn a_bounded_stack_closes_a_figure_and_an_unbounded_one_does_not() {
    // Manual figure 5-4 (A): a square drawn with (B) … (E).
    let drawn = draw("P[100,100]V(B)[+100][,+100][-100](E)R(P)");
    assert_eq!(drawn.replies(), ["[100,100]\r"]);
    assert_eq!((drawn.at(100, 150), drawn.at(200, 150), drawn.at(150, 200)), (7, 7, 7));
    // Each of the four corners is drawn once: 4 sides of 100 pixels.
    assert_eq!(drawn.inked(), 400);
    let drawn = draw("P[100,100]V(S)[+100][,+100][-100](E)R(P)");
    assert_eq!(drawn.replies(), ["[100,200]\r"]);
    assert_eq!(drawn.at(100, 150), 0);
}

#[test]
fn position_stacks_nest_and_report_overflow_and_underflow() {
    let drawn = draw("P[10,10](B)[20,20](B)[30,30](E)R(P)P(E)R(P)");
    assert_eq!(drawn.replies(), ["[20,20]\r", "[10,10]\r"]);
    // Manual table 10-1: error 8 for an (E) with nothing saved, 7 for a
    // seventeenth save.
    assert_eq!(draw("P(E)R(E)").replies(), ["\"8,69\"\r"]);
    let overflow = format!("P{}R(E)", "(B)".repeat(17));
    assert_eq!(draw(&overflow).replies(), ["\"7,66\"\r"]);
}

#[test]
fn the_pattern_restarts_at_each_key_letter_and_runs_on_across_vectors() {
    // Pattern 2 is four on, four off; at multiplier 1 a vector of 12
    // pixels shows on, off, on.
    let drawn = draw("W(P2(M1))P[0,0]V[11,0]");
    let row: Vec<u8> = (0..12).map(|x| drawn.at(x, 0)).collect();
    assert_eq!(row, [7, 7, 7, 7, 0, 0, 0, 0, 7, 7, 7, 7]);
    // Two vectors under one key letter share the pattern; a new letter
    // starts it again.
    let drawn = draw("W(P2(M1))P[0,0]V[5,0][11,0]P[0,2]V[5,2]V[11,2]");
    let row = |y| (0..12).map(|x| drawn.at(x, y)).collect::<Vec<u8>>();
    assert_eq!(row(0), [7, 7, 7, 7, 0, 0, 0, 0, 7, 7, 7, 7]);
    // The second command draws its own first pixel, at x = 5, with the
    // first bit of the pattern.
    assert_eq!(row(2), [7, 7, 7, 7, 0, 7, 7, 7, 7, 0, 0, 0]);
}

#[test]
fn the_second_page_is_drawn_on_and_shown_separately() {
    let drawn = draw("P(P1)V[10,0]");
    assert_eq!(drawn.inked(), 0);
    assert_eq!(drawn.screen.page(1).unwrap().pixel(5, 0), Some(7));
    let drawn = draw("P(P1)V[10,0]S(P1)");
    assert_eq!(drawn.screen.displayed_index(), 1);
    assert_eq!(drawn.inked(), 11);
}

#[test]
fn drawing_off_the_screen_is_clipped() {
    let drawn = draw("P[-50,10]V[850,10]P[790,470]V[900,580]");
    assert_eq!((drawn.at(0, 10), drawn.at(799, 10), drawn.at(799, 479)), (7, 7, 7));
}
