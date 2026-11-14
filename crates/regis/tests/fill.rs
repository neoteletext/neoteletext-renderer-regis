//! The polygon fill command (manual chapter 11).

mod common;

use common::draw;

#[test]
fn a_filled_square_covers_its_edges_and_inside_and_restores_the_cursor() {
    // Manual figure 11-1.
    let drawn = draw("P[50,200]F(V(B)[+100][,+100][-100](E))R(P)");
    assert_eq!(drawn.replies(), ["[50,200]\r"]);
    assert_eq!(drawn.inked(), 101 * 101);
    assert_eq!((drawn.at(50, 200), drawn.at(150, 300), drawn.at(100, 250)), (7, 7, 7));
    assert_eq!((drawn.at(49, 200), drawn.at(151, 300)), (0, 0));
}

#[test]
fn a_filled_diamond_and_circle_are_solid() {
    let drawn = draw("P[400,300]F(V(B)[300,200][200,300][300,400](E))");
    assert_eq!((drawn.at(300, 300), drawn.at(300, 201), drawn.at(205, 300)), (7, 7, 7));
    assert_eq!(drawn.at(210, 210), 0);
    // Manual figure 11-2.
    let drawn = draw("P[500,300]F(C[+100])");
    assert_eq!((drawn.at(500, 300), drawn.at(500, 201), drawn.at(599, 300)), (7, 7, 7));
    assert_eq!(drawn.at(580, 220), 0);
}

#[test]
fn a_filled_closed_curve_is_solid() {
    let drawn = draw("P[100,200]F(C(B)[+200][,+100][-200](E))");
    assert_eq!(drawn.at(200, 250), 7);
}

#[test]
fn position_moves_and_dots_make_vertices_as_on_a_vt340() {
    // The signal-flag idiom: move, mark a vertex with V[], repeat.
    let drawn = draw("P[10,10]F(V[]P[+20]V[]P[,+20]V[]P[-20]V[])");
    assert_eq!(drawn.inked(), 21 * 21);
}

#[test]
fn the_fill_uses_the_last_temporary_write_control() {
    // Manual figure 11-4.
    let drawn = draw("P[200,100]F(W(I(R)),V[+100][,+100][-100][,-100])P[0,0]V[5,0]");
    assert_eq!(drawn.at(250, 150), 2);
    // The temporary colour ends with the fill.
    assert_eq!(drawn.at(3, 0), 7);
}

#[test]
fn arcs_and_vectors_join_into_one_figure() {
    // Manual figure 11-4: a box with rounded corners.
    let drawn =
        draw("P[200,100]F(V[+100]C(C,A-90)[,+50]V[,+100]C(C,A-90)[-50]V[-100]C(C,A-90)[,-50]V[,-100]C(C,A-90)[+50])");
    assert_eq!((drawn.at(250, 200), drawn.at(160, 200), drawn.at(340, 200)), (7, 7, 7));
    // The corner outside the rounding is empty.
    assert_eq!(drawn.at(152, 102), 0);
}

#[test]
fn fewer_than_three_vertices_draw_nothing() {
    let drawn = draw("P[10,10]F(V[20,10])R(P)");
    assert_eq!(drawn.inked(), 0);
    assert_eq!(drawn.replies(), ["[10,10]\r"]);
}

#[test]
fn a_fill_in_complement_writes_each_pixel_once() {
    let drawn = draw("S(I5,E)W(C)P[10,10]F(V[30,10][30,30][10,30][10,10])");
    assert_eq!((drawn.at(10, 10), drawn.at(20, 20), drawn.at(30, 30)), (10, 10, 10));
}

#[test]
fn a_fill_pattern_runs_down_the_screen() {
    let drawn = draw("W(P2(M1))P[0,0]F(V[20,0][20,15][0,15][0,0])");
    let column: Vec<u8> = (0..16).map(|y| drawn.at(10, y)).collect();
    assert_eq!(column, [7, 7, 7, 7, 0, 0, 0, 0, 7, 7, 7, 7, 0, 0, 0, 0]);
}

#[test]
fn only_path_commands_are_allowed_inside_a_fill() {
    let drawn = draw("P[10,10]F(S(E)T'x'V[30,10][30,30][10,30])");
    assert_eq!(drawn.at(25, 15), 7);
}
