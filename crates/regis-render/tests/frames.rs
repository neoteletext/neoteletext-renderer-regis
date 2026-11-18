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
