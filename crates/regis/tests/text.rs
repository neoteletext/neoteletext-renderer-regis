//! The text command (manual chapter 7).

mod common;

use common::{Drawn, draw};

/// The lit pixels of a rectangle, as rows of `#` and `.`.
fn picture(drawn: &Drawn, x: u32, y: u32, width: u32, height: u32) -> Vec<String> {
    (y..y + height)
        .map(|row| (x..x + width).map(|column| if drawn.at(column, row) == 0 { '.' } else { '#' }).collect())
        .collect()
}

/// The bounding box of everything lit: left, top, right, bottom.
fn bounds(drawn: &Drawn) -> (u32, u32, u32, u32) {
    let lit: Vec<(u32, u32)> =
        (0..480).flat_map(|y| (0..800).map(move |x| (x, y))).filter(|(x, y)| drawn.at(*x, *y) != 0).collect();
    let xs = lit.iter().map(|point| point.0);
    let ys = lit.iter().map(|point| point.1);
    (xs.clone().min().unwrap(), ys.clone().min().unwrap(), xs.max().unwrap(), ys.max().unwrap())
}

#[test]
fn a_character_is_drawn_below_and_right_of_the_position_and_the_cursor_advances() {
    let drawn = draw("P[100,100]T'H'R(P)");
    assert_eq!(drawn.replies(), ["[109,100]\r"]);
    let (left, top, right, bottom) = bounds(&drawn);
    assert!(left >= 100 && right <= 107 && top >= 100 && bottom <= 119, "{:?}", (left, top, right, bottom));
}

#[test]
fn size_one_text_uses_the_twenty_row_shapes() {
    // The digit 0 as a VT340 draws it at size 1 (from a screen capture).
    let drawn = draw("P[0,0]T'0'");
    let expected = [
        "...###..", "..##.##.", ".##...##", ".##...##", ".##...##", ".##...##", ".##...##", ".##...##", ".##...##",
        "..##.##.", "...###..",
    ];
    assert_eq!(picture(&drawn, 0, 4, 8, 11), expected);
}

#[test]
fn standard_sizes_scale_the_cell_and_the_spacing() {
    let end = |commands: &str| draw(&format!("P[0,0]{commands}R(P)")).replies().remove(0);
    assert_eq!(end("T(S0)'ab'"), "[18,0]\r");
    assert_eq!(end("T(S2)'ab'"), "[36,0]\r");
    assert_eq!(end("T(S16)'a'"), "[144,0]\r");
    let drawn = draw("P[0,0]T(S4)'H'");
    let (_, _, right, bottom) = bounds(&drawn);
    assert!((24..32).contains(&right) && (30..60).contains(&bottom));
    // Size 17 does not exist (error 9).
    assert_eq!(draw("T(S17)R(E)").replies(), ["\"9,0\"\r"]);
}

#[test]
fn explicit_spacing_and_cell_sizes_are_honoured() {
    // Manual figure 7-2.
    let drawn = draw("P[300,100]T[35,25]'AB'R(P)");
    assert_eq!(drawn.replies(), ["[370,150]\r"]);
    let drawn = draw("P[0,0]T(S[40,40],U[16,20])'M'");
    let (_, _, right, bottom) = bounds(&drawn);
    assert!(right < 16 && bottom < 20);
}

#[test]
fn the_height_multiplier_changes_height_only() {
    let drawn = draw("P[0,0]T(S1,H6)'H'R(P)");
    assert_eq!(drawn.replies(), ["[9,0]\r"]);
    let (_, _, right, bottom) = bounds(&drawn);
    assert!(right < 8 && (30..60).contains(&bottom));
}

#[test]
fn a_tilted_string_runs_along_its_direction() {
    let end = |commands: &str| draw(&format!("P[400,240]{commands}R(P)")).replies().remove(0);
    assert_eq!(end("T(D90,S1)'ab'"), "[400,222]\r");
    assert_eq!(end("T(D180,S1)'ab'"), "[382,240]\r");
    assert_eq!(end("T(D270,S1)'ab'"), "[400,258]\r");
    // At 180 degrees a character lies above and left of the position.
    let drawn = draw("P[400,240]T(D180,S1)'H'");
    let (left, top, right, bottom) = bounds(&drawn);
    assert!(right <= 400 && bottom <= 240 && left >= 392 && top >= 220);
}

#[test]
fn a_second_tilt_turns_the_characters_but_not_the_string() {
    // Manual: T(D<string>,S<n>,D<character>).
    let drawn = draw("P[400,240]T(D0,S1,D90)'H'R(P)");
    assert_eq!(drawn.replies(), ["[409,240]\r"]);
    let (_, top, _, bottom) = bounds(&drawn);
    assert!(bottom <= 240 && top >= 232);
}

#[test]
fn control_characters_move_the_cursor() {
    let end = |text: &str| draw(&format!("P[100,100]T'{text}'R(P)")).replies().remove(0);
    assert_eq!(end("ab\r"), "[100,100]\r");
    assert_eq!(end("ab\n"), "[118,120]\r");
    assert_eq!(end("ab\r\nc"), "[109,120]\r");
    assert_eq!(end("ab\x08"), "[109,100]\r");
    assert_eq!(end("a\t"), "[118,100]\r");
}

#[test]
fn pixel_vectors_shift_by_half_a_cell_for_sub_and_superscripts() {
    // Manual figure 7-12: 'H' 7 '2' 1 'O'.
    let drawn = draw("P[300,200]T(S2)'H'7R(P)T'2'1R(P)");
    assert_eq!(drawn.replies(), ["[327,215]\r", "[354,200]\r"]);
}

#[test]
fn temporary_text_controls_are_restored() {
    // Manual figure 7-11.
    let drawn = draw("P[100,100]T(S1)T(B)(S5,D180)'A'(E)P[100,100]T'A'R(P)");
    assert_eq!(drawn.replies(), ["[109,100]\r"]);
}

#[test]
fn replace_writing_fills_the_cell_and_overlay_does_not() {
    let drawn = draw("S(I1,E)P[0,0]T(W(R))' '");
    assert_eq!((drawn.at(0, 0), drawn.at(8, 19), drawn.at(9, 0)), (1, 1, 1));
    let drawn = draw("S(I1,E)S(I3)P[0,0]T(W(R))' '");
    assert_eq!((drawn.at(0, 0), drawn.at(8, 19), drawn.at(9, 0)), (3, 3, 1));
    let drawn = draw("S(I1,E)S(I3)P[0,0]T' '");
    assert_eq!(drawn.at(0, 0), 1);
}

#[test]
fn italics_lean_the_top_of_a_character() {
    let left_edge = |commands: &str| bounds(&draw(&format!("P[100,100]T(S4{commands})'I'"))).0;
    assert!(left_edge(",I45") < left_edge(""));
    assert!(left_edge(",I-45") >= left_edge(""));
}

#[test]
fn quotes_inside_strings_and_latin_1_characters_are_drawn() {
    assert_eq!(draw("P[0,0]T'don''t'R(P)").replies(), ["[45,0]\r"]);
    let mut terminal = common::terminal();
    let mut screen = regis::Screen::new();
    let _ = terminal.feed_regis(&mut screen, b"P[0,0]T'\xE9'");
    let _ = terminal.finish(&mut screen);
    let lit = screen.displayed().pixels().iter().filter(|pixel| **pixel != 0).count();
    assert!((10..80).contains(&lit), "é drew {lit} pixels");
}

#[test]
fn without_a_font_every_character_is_the_error_block() {
    let mut terminal = regis::Terminal::new();
    let mut screen = regis::Screen::new();
    let _ = terminal.feed_regis(&mut screen, b"P[0,0]T'A'");
    let _ = terminal.finish(&mut screen);
    let lit = screen.displayed().pixels().iter().filter(|pixel| **pixel != 0).count();
    assert_eq!(lit, 8 * 20);
}

#[test]
fn spacing_is_measured_in_the_streams_coordinates() {
    // Coordinates at half scale: a spacing of 100 units is 50 pixels, and
    // the position reported comes back in units.
    let drawn = draw("S(A[0,0][1598,958])P[0,0]T[100,0]'A'R(P)");
    assert_eq!(drawn.replies(), ["[100,0]\r"]);
}
