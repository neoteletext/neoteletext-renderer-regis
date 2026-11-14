//! The screen control command (manual chapter 2).

mod common;

use common::draw;
use regis::Event;

#[test]
fn erase_fills_the_page_with_the_background_and_keeps_the_position() {
    let drawn = draw("P[50,60]V[70,60]S(I3,E)R(P)");
    assert!(drawn.screen.displayed().pixels().iter().all(|pixel| *pixel == 3));
    assert_eq!(drawn.replies(), ["[70,60]\r"]);
}

#[test]
fn the_power_up_colour_map_is_the_manuals_table() {
    let colours = draw("").terminal.palette().colours();
    assert_eq!(colours[0], [0, 0, 0]);
    assert_eq!(colours[1], [51, 51, 204]);
    assert_eq!(colours[2], [204, 33, 33]);
    assert_eq!(colours[7], [135, 135, 135]);
    assert_eq!(colours[15], [204, 204, 204]);
}

#[test]
fn the_output_map_takes_hls_letters_and_lightness() {
    // Manual examples: S(M1(AH60L80S60)3(AH150L50S60)), S(M0(AR)2(AG)),
    // S(M1(L25)2(L99)3(L50)).
    let palette = |commands: &str| draw(commands).terminal.palette().colours();
    let colours = palette("S(M0(AR)2(AG))");
    assert_eq!((colours[0], colours[2]), ([255, 0, 0], [0, 255, 0]));
    let colours = palette("S(M1(L25)2(L99)3(L50))");
    assert_eq!((colours[1], colours[2], colours[3]), ([64, 64, 64], [252, 252, 252], [128, 128, 128]));
    let colours = palette("S(M5(H120L50S100))");
    assert_eq!(colours[5], [255, 0, 0]);
    // A location past 15 changes nothing.
    assert_eq!(palette("S(M16(AR))"), palette(""));
}

#[test]
fn background_and_foreground_may_be_named_by_the_closest_colour() {
    // Location 2 is the map's red; location 6 its yellow.
    let drawn = draw("S(I(R),E)");
    assert_eq!(drawn.at(0, 0), 2);
    let drawn = draw("W(I(H180L50S100))P[1,1]V[]");
    assert_eq!(drawn.at(1, 1), 6);
}

#[test]
fn display_addressing_scales_coordinates_evenly() {
    let drawn = draw("S(A[0,0][1598,958])P[800,400]V[]R(P)");
    assert_eq!(drawn.at(400, 200), 7);
    assert_eq!(drawn.replies(), ["[800,400]\r"]);
    // An upward y axis: [0,0] is the bottom-left corner.
    let drawn = draw("S(A[0,479][799,0])P[0,0]V[]");
    assert_eq!(drawn.at(0, 479), 7);
    // One corner alone is ignored.
    let drawn = draw("S(A[0,0])P[10,10]V[]");
    assert_eq!(drawn.at(10, 10), 7);
}

#[test]
fn scrolling_moves_the_picture_and_loses_what_leaves() {
    let drawn = draw("P[100,100]V[]S[10,20]");
    assert_eq!(drawn.at(90, 80), 7);
    assert_eq!(drawn.inked(), 1);
    let drawn = draw("P[5,5]V[]S[10,0]S[-10,0]");
    assert_eq!(drawn.inked(), 0);
    // A pixel vector scrolls by the multiplier.
    let drawn = draw("P[100,100]V[]S(W(M30))0");
    assert_eq!(drawn.at(70, 100), 7);
}

#[test]
fn a_time_delay_is_reported_to_the_caller_in_ticks() {
    assert_eq!(draw("S(T60)").events, [Event::Wait(60)]);
    // One call can ask for a minute at most.
    assert_eq!(draw("S(T99999)").events, [Event::Wait(3600), Event::LimitReached]);
}

#[test]
fn the_cursor_switch_is_remembered_and_hard_copy_is_ignored() {
    assert!(draw("").terminal.cursor_visible());
    assert!(!draw("S(C0)").terminal.cursor_visible());
    let drawn = draw("S(H(P[50,0])[0,0][100,100])S(C(H2))P[3,3]V[]");
    assert_eq!(drawn.inked(), 1);
}

#[test]
fn an_output_map_location_does_not_outlive_its_option() {
    // S(M(G)) names no location, so it must change nothing.
    let colours = draw("S(M5(R))S(M(G))").terminal.palette().colours();
    assert_eq!(colours[5], [255, 0, 0]);
}

#[test]
fn scrolling_is_measured_in_the_streams_coordinates() {
    // With the corner addressed as [100,100], scrolling to [100,100] moves
    // nothing, and a signed amount is measured from the corner.
    let drawn = draw("S(A[100,100][899,579])P[100,100]V[]S[100,100]");
    assert_eq!(drawn.at(0, 0), 7);
    let drawn = draw("S(A[100,100][899,579])P[110,120]V[]S[+10,+20]");
    assert_eq!(drawn.at(0, 0), 7);
}
