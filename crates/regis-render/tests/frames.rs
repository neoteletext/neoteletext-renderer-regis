//! `regis-render --frames`: a picture's animation played into a sequence
//! of images, the same on every run (neoteletext-common
//! `docs/animation.md`, ANIM-18; REQ-6530).

use std::path::{Path, PathBuf};
use std::process::Command;

fn example(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../docs/examples").join(name)
}

/// Plays the example into a new folder and returns its images by name.
fn play(folder: &str, options: &[&str]) -> Vec<(String, Vec<u8>)> {
    let folder = Path::new(env!("CARGO_TARGET_TMPDIR")).join(folder);
    let _ = std::fs::remove_dir_all(&folder);
    std::fs::create_dir_all(&folder).unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_regis-render"))
        .args(options)
        .arg(example("animation.regis"))
        .arg(folder.join("frame.ppm"))
        .output()
        .unwrap();
    assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
    let mut images: Vec<(String, Vec<u8>)> = std::fs::read_dir(&folder)
        .unwrap()
        .map(|entry| entry.unwrap())
        .map(|entry| (entry.file_name().into_string().unwrap(), std::fs::read(entry.path()).unwrap()))
        .collect();
    images.sort();
    images
}

#[test]
fn a_fixture_played_twice_gives_the_same_images() {
    let script = example("animation.script");
    let options = ["--frames", "60", "--frame-step", "7", "--script", script.to_str().unwrap()];
    let first = play("frames-first", &options);
    let second = play("frames-second", &options);
    // Every seventh tick, and always the last.
    let names: Vec<&str> = first.iter().map(|(name, _)| name.as_str()).collect();
    let expected: Vec<String> =
        [0, 7, 14, 21, 28, 35, 42, 49, 56, 60].iter().map(|tick| format!("frame-{tick:05}.ppm")).collect();
    assert_eq!(names, expected);
    assert!(first == second, "two runs of one fixture differ");
    // The animation shows: the images are not all one picture.
    let different = first.windows(2).filter(|pair| pair[0].1 != pair[1].1).count();
    assert!(different >= 8, "only {different} images differ from the one before");
}

#[test]
fn without_a_script_the_paint_rate_alone_is_played() {
    let images = play("frames-paint", &["--frames", "30", "--frame-step", "10"]);
    assert_eq!(images.len(), 4);
    assert!(images[0].1 != images[1].1 && images[1].1 != images[2].1, "the panel is painted gradually");
}

fn fixture(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures").join(name)
}

/// Runs the program on a fixture with `options` into a new folder, and
/// returns its exit code and what it wrote to standard error.
fn run_on(fixture_name: &str, folder: &str, options: &[&str]) -> (Option<i32>, String) {
    let folder = Path::new(env!("CARGO_TARGET_TMPDIR")).join(folder);
    let _ = std::fs::remove_dir_all(&folder);
    std::fs::create_dir_all(&folder).unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_regis-render"))
        .args(options)
        .arg(fixture(fixture_name))
        .arg(folder.join("frame.ppm"))
        .output()
        .unwrap();
    (output.status.code(), String::from_utf8_lossy(&output.stderr).into_owned())
}

/// DEC-7031 item 2: `--default-rate` takes 0 to 32,767 and `--pace`
/// BYTES/TICKS of 1 or more each, both only with `--frames`; anything
/// else is refused with the usage, exit code 2.
#[test]
fn the_default_rate_and_the_pace_are_read_and_mistakes_refused() {
    for good in [
        &["--frames", "2", "--default-rate", "0"][..],
        &["--frames", "2", "--default-rate", "32767"],
        &["--frames", "2", "--pace", "1/1"],
        &["--frames", "2", "--pace", "1200/600", "--default-rate", "20"],
    ] {
        let (code, report) = run_on("default-rate.regis", "options-good", good);
        assert_eq!(code, Some(0), "{good:?}: {report}");
    }
    for (bad, message) in [
        (&["--default-rate", "5"][..], "--default-rate and --pace need --frames"),
        (&["--pace", "2/3"], "--default-rate and --pace need --frames"),
        (&["--frames", "2", "--default-rate", "32768"], "--default-rate: 32768 is not 0 to 32767"),
        (&["--frames", "2", "--default-rate", "-1"], "--default-rate: -1 is not 0 to 32767"),
        (&["--frames", "2", "--default-rate", "fast"], "--default-rate: fast is not 0 to 32767"),
        (&["--frames", "2", "--pace", "0/3"], "--pace: 0/3 is not BYTES/TICKS, each 1 or more"),
        (&["--frames", "2", "--pace", "2/0"], "--pace: 2/0 is not BYTES/TICKS, each 1 or more"),
        (&["--frames", "2", "--pace", "2"], "--pace: 2 is not BYTES/TICKS, each 1 or more"),
        (&["--frames", "2", "--pace", "2/3/4"], "--pace: 2/3/4 is not BYTES/TICKS, each 1 or more"),
        (&["--frames", "2", "--pace", "a/b"], "--pace: a/b is not BYTES/TICKS, each 1 or more"),
        (&["--frames", "2", "--pace", "-2/3"], "--pace: -2/3 is not BYTES/TICKS, each 1 or more"),
        (&["--frames", "2", "--pace", "4294967296/1"], "--pace: 4294967296/1 is not BYTES/TICKS, each 1 or more"),
    ] {
        let (code, report) = run_on("default-rate.regis", "options-bad", bad);
        assert_eq!(code, Some(2), "{bad:?}: {report}");
        assert!(report.contains(message), "{bad:?}: {report}");
        assert!(report.contains("usage: regis-render"), "{bad:?}: {report}");
    }
    // An option given its value last on the line has none.
    for (bad, message) in
        [("--default-rate", "--default-rate needs a paint rate"), ("--pace", "--pace needs BYTES/TICKS")]
    {
        let output = Command::new(env!("CARGO_BIN_EXE_regis-render"))
            .args(["--frames", "2", "in", "out.ppm", bad])
            .output()
            .unwrap();
        assert_eq!(output.status.code(), Some(2));
        assert!(String::from_utf8_lossy(&output.stderr).contains(message), "{bad}");
    }
}

/// DEC-7031 item 2: a pace that has not delivered the input by the last
/// tick says how many bytes it left unread; one that has says nothing.
#[test]
fn a_pace_with_too_few_frames_prints_the_bytes_left_unread() {
    let length = std::fs::read(fixture("pace.regis")).unwrap().len();
    let (code, report) = run_on("pace.regis", "pace-unread", &["--frames", "10", "--pace", "1/1"]);
    assert_eq!(code, Some(0), "{report}");
    let expected = format!("the pace left {} bytes of the input unread at the last tick", length - 10);
    assert!(report.contains(&expected), "{report}");
    let (code, report) = run_on("pace.regis", "pace-read", &["--frames", "40", "--pace", "2/1"]);
    assert_eq!(code, Some(0), "{report}");
    assert!(!report.contains("unread"), "{report}");
}
