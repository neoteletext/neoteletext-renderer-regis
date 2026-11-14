//! The write control command (manual chapter 3).

mod common;

use common::draw;

#[test]
fn the_four_writing_styles_treat_pattern_bits_as_the_manual_says() {
    // A dashed line (four on, four off) over a line of colour 3.
    let over = |style: &str| {
        let drawn = draw(&format!("W(I3)P[0,0]V[7,0]W(I5,P2(M1),{style})P[0,0]V[7,0]"));
        (0..8).map(|x| drawn.at(x, 0)).collect::<Vec<u8>>()
    };
    assert_eq!(over("V"), [5, 5, 5, 5, 3, 3, 3, 3]);
    assert_eq!(over("R"), [5, 5, 5, 5, 0, 0, 0, 0]);
    assert_eq!(over("C"), [12, 12, 12, 12, 3, 3, 3, 3]);
    assert_eq!(over("E"), [0; 8]);
    assert_eq!(over("N1,E"), [5; 8]);
    assert_eq!(over("N1,V"), [3, 3, 3, 3, 5, 5, 5, 5]);
}

#[test]
fn the_plane_mask_limits_which_bits_change() {
    // Manual table 3-5: I5 complemented under W(F1) is I4.
    let drawn = draw("W(I5)P[0,0]V[]W(F1,C)V[]");
    assert_eq!(drawn.at(0, 0), 4);
    let drawn = draw("W(I15,F8)P[0,0]V[]W(F4)V[]");
    assert_eq!(drawn.at(0, 0), 12);
    let drawn = draw("W(F0)P[0,0]V[]");
    assert_eq!(drawn.at(0, 0), 0);
}

#[test]
fn drawing_a_shape_twice_in_complement_restores_the_screen() {
    let drawn = draw("S(I1,E)W(C)P[100,100]V(B)[+50][,+50][-50](E)V(B)[+50][,+50][-50](E)C[+30]C[+30]");
    assert!(drawn.screen.displayed().pixels().iter().all(|pixel| *pixel == 1));
}

#[test]
fn shading_fills_from_each_point_to_the_reference_line() {
    // The default reference line is the row of the position when shading
    // is switched on; the line itself is included.
    let drawn = draw("P[10,20]W(S1)V[19,10]");
    assert_eq!((drawn.at(10, 20), drawn.at(19, 10), drawn.at(19, 20), drawn.at(19, 15)), (7, 7, 7, 7));
    assert_eq!((drawn.at(11, 10), drawn.at(19, 21), drawn.at(19, 9)), (0, 0, 0));
    // A reference line given by its row, then by its column.
    let drawn = draw("P[10,20]W(S[,30])V[12,20]");
    assert_eq!(drawn.inked(), 33);
    let drawn = draw("P[10,20]W(S(X)[14])V[10,22]");
    assert_eq!(drawn.inked(), 15);
    assert_eq!(drawn.at(14, 21), 7);
}

#[test]
fn a_shaded_circle_is_solid_and_shading_off_stops_it() {
    // Manual figure 3-12.
    let drawn = draw("P[150,200]W(S1)C[+100]W(S0)P[400,200]C[+10]");
    assert_eq!((drawn.at(150, 200), drawn.at(150, 120), drawn.at(220, 250)), (7, 7, 7));
    assert_eq!(drawn.at(400, 200), 0);
    assert_eq!(drawn.at(410, 200), 7);
}

#[test]
fn a_shading_pattern_runs_down_the_screen_whichever_way_the_line_lies() {
    // Manual figures 3-16 and 3-21: the dashed pattern makes bars across.
    let bars = |shade: &str| {
        let drawn = draw(&format!("W(P2(M1))P[400,200]C(W({shade}))[+60]"));
        (196..204).map(|y| drawn.at(400, y)).collect::<Vec<u8>>()
    };
    let horizontal = bars("S1");
    assert!(horizontal.contains(&7) && horizontal.contains(&0));
    assert_eq!(horizontal, bars("S(X)[400]"));
}

#[test]
fn a_shading_character_tiles_the_shaded_area() {
    // Manual figure 3-22.
    let drawn = draw("P[400,200]T(S1)W(S'X')C[+60]W(S0)");
    let window: Vec<u8> = (380..420).map(|x| drawn.at(x, 205)).collect();
    assert!(window.contains(&7) && window.contains(&0));
    // The tiles repeat every eight columns.
    assert_eq!(window[..32], window[8..]);
}

#[test]
fn temporary_write_controls_last_for_one_command_only() {
    let drawn = draw("P[0,0]V(W(I2))[5,0]V[10,0]");
    assert_eq!((drawn.at(3, 0), drawn.at(8, 0)), (2, 7));
    // A semicolon ends them too.
    let drawn = draw("P[0,0]V(W(I2));V[5,0]");
    assert_eq!(drawn.at(3, 0), 7);
}

#[test]
fn foreground_numbers_out_of_range_are_ignored() {
    let drawn = draw("W(I16)W(I-1)P[0,0]V[]");
    assert_eq!(drawn.at(0, 0), 7);
}
