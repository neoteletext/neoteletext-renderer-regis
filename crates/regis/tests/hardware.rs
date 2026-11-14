//! The renderer against a real VT340.
//!
//! The references are screen captures from VT340 hardware (hackerb9's
//! vt340test collection, CC0), reduced to one bit a pixel: lit or dark.
//! A test compares which pixels the renderer lights.

mod common;

/// The size of the references and of the screen.
const WIDTH: usize = 800;

/// The lit-or-dark picture in a binary PBM file: `P4`, the size, then rows
/// of bits, the left pixel in the high bit.
fn reference(name: &str) -> Vec<bool> {
    let bytes = common::read(name);
    let header_end = bytes.iter().enumerate().filter(|(_, byte)| **byte == b'\n').nth(1).unwrap().0 + 1;
    bytes[header_end..].iter().flat_map(|byte| (0..8).rev().map(move |bit| (byte >> bit) & 1 == 1)).collect()
}

/// How many pixels differ between the capture and the renderer's picture
/// of the same commands, over the rows the capture covers.
fn differences(reference_name: &str, input: &str) -> usize {
    let expected = reference(reference_name);
    let drawn = common::draw_stream(&common::read(input));
    let lit = drawn.screen.displayed().pixels().iter().map(|pixel| *pixel != 0);
    assert_eq!(expected.len() % WIDTH, 0);
    expected.iter().zip(lit).filter(|(want, have)| **want != *have).count()
}

#[test]
fn the_checkerboard_of_filled_squares_matches_the_vt340_exactly() {
    assert_eq!(differences("vt340-checkerboard.pbm", "registest-checkerboard.regis"), 0);
}

#[test]
fn the_signal_flag_of_filled_polygons_matches_the_vt340_exactly() {
    // The flag's polygons are built from position moves and `V[]` dots;
    // the capture's lower rows hold shell text and are left out.
    assert_eq!(differences("vt340-interco.pbm", "interco.regis"), 0);
}

#[test]
fn the_grid_of_dashed_lines_and_text_matches_the_vt340_exactly() {
    // The labels are drawn with the VT340's own character shapes, upright
    // and turned, and one is placed by a half-cell move.
    assert_eq!(differences("vt340-grid.pbm", "registest-grid.regis"), 0);
}

#[test]
fn the_shaded_circles_differ_only_along_their_edges() {
    // The VT340 rounds its circles slightly differently.
    const KNOWN_EDGE_DIFFERENCES: usize = 272;
    assert_eq!(differences("vt340-raf.pbm", "registest-raf.regis"), KNOWN_EDGE_DIFFERENCES);
}
