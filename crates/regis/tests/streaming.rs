//! A stream may be cut anywhere: the picture must not depend on where.

mod common;

use regis::Screen;

const SAMPLES: [&str; 8] = [
    "registest-grid.regis",
    "registest-raf.regis",
    "registest-checkerboard.regis",
    "registest-bitplane.regis",
    "interco.regis",
    "hersheydemo.regis",
    "surface3d.regis",
    "galaxy.regis",
];

fn pixels_when_fed_in_pieces(stream: &[u8], piece: usize) -> Vec<u8> {
    let mut terminal = common::terminal();
    let mut screen = Screen::new();
    for chunk in stream.chunks(piece) {
        let _ = terminal.feed(&mut screen, chunk);
    }
    let _ = terminal.finish(&mut screen);
    screen.displayed().pixels().to_vec()
}

#[test]
fn feeding_byte_by_byte_draws_the_same_picture_as_feeding_all_at_once() {
    for name in SAMPLES {
        let stream = common::read(name);
        let whole = pixels_when_fed_in_pieces(&stream, stream.len());
        // An ordinary picture never comes near the work allowance.
        let drawn = common::draw_stream(&stream);
        assert!(!drawn.events.contains(&regis::Event::LimitReached), "{name} reached a limit");
        assert!(whole.iter().any(|pixel| *pixel != 0), "{name} drew nothing");
        for piece in [1, 7, 4096] {
            assert!(whole == pixels_when_fed_in_pieces(&stream, piece), "{name} differs in pieces of {piece}");
        }
    }
}

#[test]
fn rendering_is_deterministic() {
    let stream = common::read("galaxy.regis");
    let (first, second) = (pixels_when_fed_in_pieces(&stream, 100), pixels_when_fed_in_pieces(&stream, 100));
    assert_eq!(first.iter().zip(&second).position(|(a, b)| a != b), None);
}
