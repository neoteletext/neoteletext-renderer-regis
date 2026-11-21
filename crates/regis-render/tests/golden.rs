//! Golden image sequences of animation (neoteletext-common
//! `docs/animation.md`, ANIM-18: "the golden sequences named in ...
//! ANIM-10, ANIM-14, ANIM-15"; ANIM-12: "golden images of the same
//! objects in each layer and order"; REQ-6530).
//!
//! Each sequence is a fixture of `tests/fixtures/` played by
//! `regis-render --frames`; `tests/golden/sequences.txt` holds a
//! checksum of each image, a PPM file, so of its size, its pixels and
//! their colours. Beside the checksums, each test asserts what its
//! requirement says of the sequence, so that a regenerated file cannot
//! quietly accept a wrong one.
//!
//! The file carries a format version (DEVELOPMENT.md section 8). A
//! deliberate change to what is painted raises the version and
//! regenerates the file in the same change, with the reason in the
//! commit: run with `REGIS_UPDATE_GOLDEN=1`, then check that only the
//! expected lines changed.

use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::path::{Path, PathBuf};
use std::process::Command;

/// Raise when what `regis-render` paints for a fixture changes on purpose.
const FORMAT_VERSION: u32 = 1;

/// The first line of the golden file.
fn heading() -> String {
    format!("format {FORMAT_VERSION}\n")
}

fn fixture(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures").join(name)
}

fn golden_path() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/golden/sequences.txt")
}

/// The 64-bit FNV-1a hash: the offset basis, then for each byte an
/// exclusive-or and a multiplication by the FNV prime (Fowler, Noll and
/// Vo; the constants are the algorithm's).
fn fnv(bytes: &[u8]) -> u64 {
    const OFFSET_BASIS: u64 = 0xCBF2_9CE4_8422_2325;
    const PRIME: u64 = 0x0000_0100_0000_01B3;
    bytes.iter().fold(OFFSET_BASIS, |hash, &byte| (hash ^ u64::from(byte)).wrapping_mul(PRIME))
}

/// One golden sequence: a fixture, its script if it has one, and the
/// ticks played.
struct Sequence {
    name: &'static str,
    picture: &'static str,
    script: Option<&'static str>,
    frames: u32,
    step: u32,
    /// The options given beside `--frames` and `--frame-step`.
    options: &'static [&'static str],
}

const ATTRIBUTES: Sequence = Sequence {
    name: "attributes",
    picture: "attributes.regis",
    script: Some("attributes.script"),
    frames: 18,
    step: 2,
    options: &[],
};
const LAYERS: Sequence = Sequence {
    name: "layers",
    picture: "layers.regis",
    script: Some("layers.script"),
    frames: 15,
    step: 1,
    options: &[],
};
const CYCLE: Sequence =
    Sequence { name: "cycle", picture: "cycle.regis", script: Some("cycle.script"), frames: 14, step: 1, options: &[] };
const CYCLE_ONCE: Sequence = Sequence {
    name: "cycle-once",
    picture: "cycle.regis",
    script: Some("cycle-once.script"),
    frames: 9,
    step: 1,
    options: &[],
};
const PAINT: Sequence =
    Sequence { name: "paint", picture: "paint.regis", script: None, frames: 30, step: 2, options: &[] };
/// Drawings with no `T`, with `T0` and with `T4`, and a child waiting for
/// its parent, under a default paint rate of 20 (REQ-7025).
const DEFAULT_RATE: Sequence = Sequence {
    name: "default-rate",
    picture: "default-rate.regis",
    script: None,
    frames: 36,
    step: 2,
    options: &["--default-rate", "20"],
};
/// A ReGIS v1 picture fed at 2 bytes a tick (REQ-7024).
const PACE: Sequence =
    Sequence { name: "pace", picture: "pace.regis", script: None, frames: 40, step: 4, options: &["--pace", "2/1"] };

/// Plays a sequence into a new folder and returns its images by tick.
fn play(sequence: &Sequence, folder: &str) -> BTreeMap<u32, Vec<u8>> {
    let folder = Path::new(env!("CARGO_TARGET_TMPDIR")).join(folder);
    let _ = std::fs::remove_dir_all(&folder);
    std::fs::create_dir_all(&folder).unwrap();
    let mut command = Command::new(env!("CARGO_BIN_EXE_regis-render"));
    command.args(["--frames", &sequence.frames.to_string(), "--frame-step", &sequence.step.to_string()]);
    if let Some(script) = sequence.script {
        command.arg("--script").arg(fixture(script));
    }
    command.args(sequence.options);
    let output = command.arg(fixture(sequence.picture)).arg(folder.join("image.ppm")).output().unwrap();
    assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
    let report = String::from_utf8_lossy(&output.stderr);
    assert!(report.contains("0 operations did nothing"), "{}: {report}", sequence.name);
    assert!(!report.contains("warning"), "{}: {report}", sequence.name);
    let tick_of = |name: &str| -> u32 {
        name.strip_prefix("image-").and_then(|rest| rest.strip_suffix(".ppm")).unwrap().parse().unwrap()
    };
    std::fs::read_dir(&folder)
        .unwrap()
        .map(|entry| entry.unwrap())
        .map(|entry| (tick_of(entry.file_name().to_str().unwrap()), std::fs::read(entry.path()).unwrap()))
        .collect()
}

/// The lines of the golden file for a sequence: `<name> <tick> <checksum>`.
fn lines(sequence: &Sequence, images: &BTreeMap<u32, Vec<u8>>) -> String {
    let mut out = String::new();
    for (tick, image) in images {
        let _ = writeln!(out, "{} {tick:05} {:016x}", sequence.name, fnv(image));
    }
    out
}

/// Plays a sequence and holds its images to the golden file; with
/// `REGIS_UPDATE_GOLDEN` set, writes its lines there instead.
fn held_to_golden(sequence: &Sequence) -> BTreeMap<u32, Vec<u8>> {
    let images = play(sequence, &format!("golden-{}", sequence.name));
    let actual = lines(sequence, &images);
    let prefix = format!("{} ", sequence.name);
    let path = golden_path();
    let file = std::fs::read_to_string(&path).unwrap_or_default();
    if std::env::var_os("REGIS_UPDATE_GOLDEN").is_some() {
        // The other sequences' lines are kept: each test owns its own,
        // and the tests may run at once, so the file is rewritten under
        // a lock.
        let lock = std::fs::File::create(path.with_extension("lock")).unwrap();
        lock.lock().unwrap();
        let file = std::fs::read_to_string(&path).unwrap_or_default();
        let kept = file.lines().filter(|line| !line.starts_with(&prefix) && !line.starts_with("format "));
        let mut all: Vec<String> = kept.chain(actual.lines()).map(String::from).collect();
        all.sort();
        std::fs::write(&path, format!("{}{}\n", heading(), all.join("\n"))).unwrap();
        lock.unlock().unwrap();
        let _ = std::fs::remove_file(path.with_extension("lock"));
        return images;
    }
    assert!(
        file.starts_with(&heading()),
        "tests/golden/sequences.txt is not of format {FORMAT_VERSION}; generate with REGIS_UPDATE_GOLDEN=1"
    );
    let mut expected = String::new();
    for line in file.lines().filter(|line| line.starts_with(&prefix)) {
        let _ = writeln!(expected, "{line}");
    }
    assert_eq!(actual, expected, "{} changed: if deliberate, raise FORMAT_VERSION and regenerate", sequence.name);
    images
}

/// The pixel of a binary PPM image of the screen's size, as red, green
/// and blue.
fn pixel(image: &[u8], x: usize, y: usize) -> [u8; 3] {
    const WIDTH: usize = 800;
    const HEIGHT: usize = 480;
    const SAMPLES: usize = 3;
    let pixels = &image[image.len() - WIDTH * HEIGHT * SAMPLES..];
    let at = (y * WIDTH + x) * SAMPLES;
    [pixels[at], pixels[at + 1], pixels[at + 2]]
}

/// ANIM-10's examples: a frame list, the three looks and a paint rate.
#[test]
fn the_attributes_frame_list_looks_and_paint_rate_are_the_golden_images() {
    let images = held_to_golden(&ATTRIBUTES);
    assert_eq!(images.keys().copied().collect::<Vec<_>>(), [0, 2, 4, 6, 8, 10, 12, 14, 16, 18]);
    // Drawing 40 at (100, 100) shows the entry of its frame list its
    // frame names; a frame beyond the list shows the last entry, and
    // frame 0 the first again.
    let sprite = |tick: u32| pixel(&images[&tick], 105, 105);
    let frames = [sprite(0), sprite(2), sprite(4), sprite(6)];
    for (index, frame) in frames.iter().enumerate() {
        assert!(!frames[..index].contains(frame), "frame {index} looks like an earlier one");
    }
    assert_eq!((sprite(8), sprite(10)), (frames[3], frames[0]));
    // Drawing 10 at (300, 100): its own look, then hover, pressed and
    // alternative forced, then its own again.
    let look = |tick: u32| [pixel(&images[&tick], 302, 101), pixel(&images[&tick], 310, 104)];
    let looks = [look(8), look(10), look(12), look(14)];
    for (index, one) in looks.iter().enumerate() {
        assert!(!looks[..index].contains(one), "look {index} looks like an earlier one");
    }
    assert_eq!(look(16), looks[0]);
    // Drawing 7's outline is painted gradually: its first corner is
    // there before its last side.
    let outline = |tick: u32| [pixel(&images[&tick], 110, 250), pixel(&images[&tick], 100, 280)];
    let background = pixel(&images[&0], 0, 0);
    assert_eq!(outline(0), [background, background]);
    assert!(outline(2)[0] != background && outline(2)[1] == background);
    assert_ne!(outline(18)[1], background);
}

/// ANIM-12: the same objects in each layer and order.
#[test]
fn the_same_drawings_in_each_layer_and_order_are_the_golden_images() {
    let images = held_to_golden(&LAYERS);
    assert_eq!(images.len(), 16);
    // Drawings 20, 21 and 22 all cover (290, 250), and only they and
    // the background drawing do; which is on top is seen there.
    let colour = |tick: u32, x: usize, y: usize| pixel(&images[&tick], x, y);
    let top = |tick: u32| colour(tick, 290, 250);
    let alone = [colour(0, 205, 185), colour(0, 245, 325), colour(0, 395, 355)];
    let [twenty, twenty_one, twenty_two] = alone;
    assert!(twenty != twenty_one && twenty_one != twenty_two && twenty != twenty_two);
    // By tick: equal orders by number (0); the order within the layer (1,
    // 2); a higher layer whatever the order (3, 4); the order, then the
    // number, within layer 3 (5, 6); each sent to the background layer
    // while drawing 22 stays above (7, 8) and then joins them (9, 10);
    // and each brought back through the layers (11 to 15).
    let expected = [
        twenty_two, twenty, twenty, twenty_one, twenty, twenty_two, twenty_two, twenty_two, twenty_two, twenty_two,
        twenty_one, twenty, twenty_one, twenty_two, twenty_one, twenty_one,
    ];
    let seen: Vec<[u8; 3]> = (0..16).map(top).collect();
    assert_eq!(seen, expected);
    // No drawing in layer 0 goes beneath the background drawing 1, whose
    // number is lower: at tick 9 all three are in layer 0 and all show.
    assert_eq!([colour(9, 205, 185), colour(9, 245, 325), colour(9, 395, 355)], alone);
    // At tick 10 drawing 22 has a lower order than drawing 1 and is
    // beneath it.
    assert_eq!(colour(10, 395, 355), colour(10, 440, 380));
    assert_ne!(colour(10, 395, 355), twenty_two);
    // Drawing 23 follows its parent 21 through the layers: its centre,
    // (300, 270), shows only while 21 is over both others.
    let child = colour(3, 300, 270);
    assert!(!alone.contains(&child));
    let over: Vec<u32> = (0..16).filter(|tick| colour(*tick, 300, 270) == child).collect();
    assert_eq!(over, [3, 10, 12, 14, 15]);
}

/// ANIM-14: an image sequence of a four-frame cycle, and of one played
/// once.
#[test]
fn a_four_frame_cycle_is_the_golden_sequence() {
    let images = held_to_golden(&CYCLE);
    // Frame 0 until the cycle starts at tick 1, then three ticks a frame.
    let frame_at = [0, 0, 0, 0, 1, 1, 1, 2, 2, 2, 3, 3, 3, 0, 0];
    for (tick, frame) in (0..).zip(frame_at) {
        let first = frame_at.iter().position(|other| *other == frame).unwrap();
        assert!(images[&tick] == images[&u32::try_from(first).unwrap()], "tick {tick} is not frame {frame}");
    }
    let distinct: std::collections::BTreeSet<&Vec<u8>> = images.values().collect();
    assert_eq!(distinct.len(), 4);

    let once = held_to_golden(&CYCLE_ONCE);
    let frame_at = [0, 0, 0, 1, 1, 2, 2, 3, 3, 3];
    for (tick, frame) in (0..).zip(frame_at) {
        // The frames are the cycle's: 0 at its tick 0, 1 at 4, 2 at 7, 3 at 10.
        let same = [0, 4, 7, 10][frame];
        assert!(once[&tick] == images[&same], "tick {tick} of the cycle played once is not frame {frame}");
    }
}

/// ANIM-15: an image sequence of a drawing with two children, painted at
/// their rates.
#[test]
fn a_drawing_with_two_children_painted_gradually_is_the_golden_sequence() {
    let images = held_to_golden(&PAINT);
    assert_eq!(images.len(), 16);
    let background = pixel(&images[&0], 0, 0);
    let painted = |tick: u32, x: usize, y: usize| pixel(&images[&tick], x, y) != background;
    // The parent's outline, from (100, 100): nothing at tick 0, its first
    // side before its last.
    assert!(!painted(0, 110, 100));
    let first_side = (0..=30).step_by(2).find(|tick| painted(*tick, 110, 100)).unwrap();
    let last_side = (0..=30).step_by(2).find(|tick| painted(*tick, 100, 150)).unwrap();
    assert!(first_side < last_side, "the outline's sides appeared at ticks {first_side} and {last_side}");
    // The children wait for the parent to finish, then paint together:
    // the line of drawing 8 grows from its left end, and the filled
    // square of drawing 9 appears whole when its cost is earned.
    let line_start = (0..=30).step_by(2).find(|tick| painted(*tick, 125, 130)).unwrap();
    let line_end = (0..=30).step_by(2).find(|tick| painted(*tick, 375, 130)).unwrap();
    assert!(last_side <= line_start && line_start < line_end, "{last_side}, {line_start}, {line_end}");
    let corners = |tick: u32| [painted(tick, 121, 151), painted(tick, 198, 188)];
    let square = (0..=30).step_by(2).find(|tick| corners(*tick)[0]).unwrap();
    assert!(last_side <= square);
    for tick in (0..=30).step_by(2) {
        assert_eq!(corners(tick), [tick >= square; 2], "the square is whole or absent at tick {tick}");
    }
    // The finished picture stays as it is.
    // Checksums, so that a failure does not print two images.
    assert_eq!(fnv(&images[&28]), fnv(&images[&30]));
}

/// REQ-7025: with a default paint rate, a drawing with no `T` paints at
/// it, one with `T0` at once and one with `T4` at its own rate, and a
/// child waits for its parent painting at the default.
#[test]
fn drawings_under_a_default_paint_rate_are_the_golden_sequence() {
    let images = held_to_golden(&DEFAULT_RATE);
    let ticks: Vec<u32> = (0..=36).step_by(2).collect();
    assert_eq!(images.keys().copied().collect::<Vec<_>>(), ticks);
    let background = pixel(&images[&0], 0, 0);
    let painted = |tick: u32, x: usize, y: usize| pixel(&images[&tick], x, y) != background;
    let first = |x: usize, y: usize| ticks.iter().copied().find(|tick| painted(*tick, x, y));
    // Drawing 4, with T0: whole before any tick.
    assert!(painted(0, 100, 400) && painted(0, 300, 400));
    // Drawing 1, with no T, at the default 20 (2,000 units a second): its
    // outline of about 800 pixels takes about 24 ticks, its first side
    // before its last, whose next-to-last pixel is (100, 101).
    assert!(!painted(0, 110, 100));
    let first_side = first(110, 100).unwrap();
    let last_side = first(100, 101).unwrap();
    assert!(first_side < last_side && (24..=26).contains(&last_side), "{first_side}, {last_side}");
    // Child 2, with T0, waits for its parent and then appears whole, in
    // the image of the parent's last pixels or the next (the outline
    // costs a few units more than the 800 pixels the next-to-last one
    // needs).
    let child = first(125, 130).unwrap();
    assert!(last_side <= child && child <= last_side + 2, "{last_side}, {child}");
    assert_eq!(first(375, 130), Some(child));
    // Drawing 3, with T4 (400 units a second), takes 30 ticks for its 201
    // pixels: it grows from its left end at its own rate, not the default.
    let line_start = first(102, 300).unwrap();
    let line_end = first(300, 300).unwrap();
    assert!(line_start <= 2 && (30..=32).contains(&line_end), "{line_start}, {line_end}");
    assert!(painted(16, 200, 300) && !painted(14, 200, 300), "half the line at about tick 15");
    // Finished, it stays as it is.
    assert_eq!(fnv(&images[&34]), fnv(&images[&36]));
}

/// REQ-7024: a ReGIS v1 picture fed at a pace, two bytes a tick from tick
/// 0: each image shows the bytes delivered by its tick, and the last is
/// the picture fed at once.
#[test]
fn a_picture_fed_at_a_pace_is_the_golden_sequence() {
    let images = held_to_golden(&PACE);
    let background = pixel(&images[&0], 0, 0);
    // Nothing is fed before the first tick: only the background.
    assert!((0..800).all(|x| pixel(&images[&0], x, 100) == background));
    // The first line's 25 bytes are delivered by tick 13: not at 12, by 16.
    assert_eq!(pixel(&images[&12], 400, 100), background);
    assert_ne!(pixel(&images[&16], 400, 100), background);
    // The second line's bytes end at byte 45, tick 23.
    assert_eq!(pixel(&images[&20], 400, 200), background);
    assert_ne!(pixel(&images[&24], 400, 200), background);
    let distinct: std::collections::BTreeSet<&Vec<u8>> = images.values().collect();
    assert!(distinct.len() >= 4, "only {} distinct images", distinct.len());
    // All 67 bytes are fed by tick 34: the last images are the picture
    // fed at once.
    let at_once = play(
        &Sequence { name: "pace-at-once", picture: "pace.regis", script: None, frames: 1, step: 1, options: &[] },
        "golden-pace-at-once",
    );
    assert_eq!(fnv(&images[&36]), fnv(&at_once[&0]));
    assert_eq!(fnv(&images[&40]), fnv(&at_once[&0]));
    assert_ne!(fnv(&images[&32]), fnv(&at_once[&0]));
}
