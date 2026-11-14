//! The curve command (manual chapter 6).

mod common;

use common::draw;

#[test]
fn a_circle_about_the_position_passes_through_the_point_and_leaves_the_cursor() {
    let drawn = draw("P[400,240]C[+100]R(P)");
    assert_eq!(drawn.replies(), ["[400,240]\r"]);
    assert_eq!((drawn.at(500, 240), drawn.at(300, 240), drawn.at(400, 140), drawn.at(400, 340)), (7, 7, 7, 7));
    assert_eq!((drawn.at(400, 240), drawn.at(495, 240)), (0, 0));
}

#[test]
fn a_circle_with_its_centre_given_passes_through_the_position() {
    // Manual figure 6-2.
    let drawn = draw("P[400,240]C(C)[+150]R(P)");
    assert_eq!(drawn.replies(), ["[400,240]\r"]);
    assert_eq!((drawn.at(400, 240), drawn.at(700, 240), drawn.at(550, 90)), (7, 7, 7));
}

#[test]
fn the_centre_option_lasts_for_the_command_only() {
    // Manual figure 6-4: C(C)[+50][+25] keeps the option; a new C drops it.
    let drawn = draw("P[500,100]C(C)[+50]C[+25]");
    assert_eq!((drawn.at(600, 100), drawn.at(525, 100)), (7, 7));
}

#[test]
fn a_positive_arc_runs_counterclockwise_and_a_negative_one_clockwise() {
    // Manual figure 6-5: arcs about the position, from the point given.
    let drawn = draw("P[200,200]C(A180)[+150]R(P)");
    assert_eq!(drawn.replies(), ["[200,200]\r"]);
    assert_eq!((drawn.at(200, 50), drawn.at(200, 350)), (7, 0));
    let drawn = draw("P[200,200]C(A-180)[+150]");
    assert_eq!((drawn.at(200, 50), drawn.at(200, 350)), (0, 7));
}

#[test]
fn an_arc_with_its_centre_given_moves_the_cursor_to_its_end() {
    // Manual figure 6-7: from [350,200] about [200,200], half way round.
    let drawn = draw("P[350,200]C(A180C)[-150]R(P)");
    assert_eq!(drawn.replies(), ["[50,200]\r"]);
    assert_eq!(drawn.at(200, 50), 7);
    // Arcs link end to end (manual figure 6-8): a quarter turn about
    // [150,100] ends at [150,150]; a quarter turn about [150,200] from
    // there ends at [100,200].
    let drawn = draw("P[100,100]C(A90C)[+50](A90C)[,+50]R(P)");
    assert_eq!(drawn.replies(), ["[100,200]\r"]);
}

#[test]
fn a_fractional_angle_is_drawn_to_the_closest_degree() {
    let end = |angle: &str| draw(&format!("P[300,200]C(A{angle}C)[-200]R(P)")).replies().remove(0);
    assert_eq!(end("27.5"), end("28"));
    assert_eq!(end("27.4"), end("27"));
}

#[test]
fn an_open_curve_passes_through_its_inner_points_only() {
    // Manual figure 6-11: without null arguments the curve runs from the
    // first point given to the next to last.
    let drawn = draw("P[75,125]C(S)[150,200][225,125][300,200][375,125](E)R(P)");
    assert_eq!(drawn.replies(), ["[375,125]\r"]);
    assert_eq!((drawn.at(150, 200), drawn.at(225, 125), drawn.at(300, 200)), (7, 7, 7));
    assert_eq!((drawn.at(75, 125), drawn.at(375, 125)), (0, 0));
    // Manual figure 6-12: null arguments at both ends draw it all.
    let drawn = draw("P[75,125]C(S)[][150,200][225,125][300,200][375,125][](E)");
    assert_eq!((drawn.at(75, 125), drawn.at(375, 125)), (7, 7));
}

#[test]
fn a_closed_curve_returns_to_its_start() {
    // Manual figure 6-10.
    let drawn = draw("P[230,240]C(B)[320,160][480,120][570,160][480,240][320,280](E)R(P)");
    assert_eq!(drawn.replies(), ["[230,240]\r"]);
    for (x, y) in [(230, 240), (320, 160), (480, 120), (570, 160), (480, 240), (320, 280)] {
        assert_eq!(drawn.at(x, y), 7, "the curve misses [{x},{y}]");
    }
}

#[test]
fn a_curve_is_drawn_as_its_points_arrive() {
    // Manual figure 11-5 leaves a curve unended; what was given is drawn.
    let drawn = draw("P[100,100]C(S)[][200,150][300,100][400,150]");
    assert_eq!((drawn.at(100, 100), drawn.at(200, 150), drawn.at(300, 100)), (7, 7, 7));
}

#[test]
fn a_circle_of_no_radius_is_one_pixel() {
    let drawn = draw("P[10,10]C[]");
    assert_eq!(drawn.inked(), 1);
}
