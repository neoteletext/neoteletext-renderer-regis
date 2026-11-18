//! REQ-6764 in `regis-render`: `--teletext` draws every code of every one of
//! NeoTeletext's provided alphabets, 100 to 154, as the engine's Noto
//! Teletext glyph of the character the engine's code tables give it, and
//! joins a Persian word as the engine's font draws it (renderer-parity.md
//! PAR-12, PAR-13). Each expectation is read from the engine's checkout
//! here, apart from the program.
#![allow(
    clippy::arithmetic_side_effects,
    clippy::indexing_slicing,
    reason = "a test: fixed inputs within range, and failure is a panic"
)]

use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
use std::process::Command;

use regis::limits::{FIRST_PROVIDED_ALPHABET, PROVIDED_SETS};

/// The picture: 800 by 480, three bytes a pixel after its header.
const WIDTH: usize = 800;
const HEADER: &[u8] = b"P6\n800 480\n255\n";
/// The cell the test draws in: a unit cell of 24 by 20, twice the engine's
/// 12-pixel glyph across, so each glyph pixel is two picture pixels wide
/// and nothing of it is left out.
const CELL_WIDTH: usize = 24;
const CELL_HEIGHT: usize = 20;
const GLYPH_WIDTH: usize = 12;
const PER_LINE: usize = 32;
const LINE_STEP: usize = 40;
const CELL: &str = "T(S[24,20],U[24,20])";

const BDF_FILES: [&str; 4] =
    ["noto-teletext.bdf", "noto-teletext-composed.bdf", "noto-teletext-arabic.bdf", "noto-teletext-japanese.bdf"];

/// The engine's checkout: `NEOTELETEXT_ENGINE`, or the sibling of this
/// repository's checkout.
fn engine() -> PathBuf {
    std::env::var_os("NEOTELETEXT_ENGINE").map_or_else(
        || PathBuf::from(std::env::var_os("HOME").expect("HOME")).join("devel/neoteletext-renderer-teletext"),
        PathBuf::from,
    )
}

fn engine_file(path: &str) -> String {
    let path = engine().join(path);
    fs::read_to_string(&path)
        .unwrap_or_else(|error| panic!("{}: {error}: this test needs the engine's checkout", path.display()))
}

/// The engine's `FONT_SETS` names, in its order.
fn font_sets() -> Vec<String> {
    let source = engine_file("crates/neoteletext-core/src/glyphs/conformance.rs");
    let block = source.split("pub const FONT_SETS").nth(1).unwrap().split("];").next().unwrap();
    block.split("(\"").skip(1).map(|rest| rest.split('"').next().unwrap().to_owned()).collect()
}

/// The engine's list's name for a set.
fn list_name(name: &str) -> String {
    let spaced = name.replace('-', " ");
    for (prefix, script) in [("G0 Cyrillic ", "Cyrillic"), ("G0 Arabic ", "Arabic")] {
        if let Some(rest) = spaced.strip_prefix(prefix) {
            return match rest {
                "1" | "2" | "3" => format!("G0 {script}{rest}"),
                "Plain" => "G0 ArabicPlain".to_owned(),
                _ => format!("G0 {rest}"),
            };
        }
    }
    spaced
}

/// The engine's glyph of each character: Unicode scalar to rows.
fn noto_glyphs() -> HashMap<u32, Vec<u16>> {
    let mut glyphs = HashMap::new();
    for file in BDF_FILES {
        let text = engine_file(&format!("third_party/noto/{file}"));
        for character in text.split("STARTCHAR").skip(1) {
            let encoding = character.lines().find_map(|line| line.strip_prefix("ENCODING ")).unwrap().parse().unwrap();
            let bitmap = character.split("BITMAP\n").nth(1).unwrap();
            let rows = bitmap
                .lines()
                .take_while(|line| *line != "ENDCHAR")
                .map(|row| u16::from_str_radix(row, 16).unwrap() >> 4)
                .collect();
            glyphs.insert(encoding, rows);
        }
    }
    glyphs
}

/// The character at `position` (`C/R`) of the set named `list_name`.
fn listed_character(list: &str, list_name: &str, position: &str) -> Option<u32> {
    let wanted = format!("{list_name} {position}");
    let line = list.lines().find(|line| {
        line.starts_with("U+")
            && line.split("; ").any(|usage| usage == wanted || usage.ends_with(&format!(" {wanted}")))
    })?;
    u32::from_str_radix(&line[2..line.find(' ').unwrap()], 16).ok()
}

/// Draws `commands` with `--teletext` and returns the picture's pixels.
fn render(name: &str, commands: &[u8]) -> Vec<u8> {
    let base = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join(format!("teletext-{name}"));
    fs::create_dir_all(&base).unwrap();
    let (input, output) = (base.join("page.regis"), base.join("page.ppm"));
    fs::write(&input, commands).unwrap();
    let run = Command::new(env!("CARGO_BIN_EXE_regis-render"))
        .arg("--teletext")
        .arg(engine())
        .arg(&input)
        .arg(&output)
        .output()
        .unwrap();
    assert!(run.status.success(), "{}", String::from_utf8_lossy(&run.stderr));
    let picture = fs::read(output).unwrap();
    assert!(picture.starts_with(HEADER));
    picture[HEADER.len()..].to_vec()
}

/// Whether the cell at `(x, y)` shows `rows` doubled across.
fn shows(pixels: &[u8], x: usize, y: usize, rows: &[u16]) -> Result<(), String> {
    for (row, bits) in rows.iter().enumerate() {
        for column in 0..CELL_WIDTH {
            let want = bits >> (GLYPH_WIDTH - 1 - column / 2) & 1 != 0;
            let at = ((y + row) * WIDTH + x + column) * 3;
            if (pixels[at..at + 3] != [0, 0, 0]) != want {
                return Err(format!("row {row} column {column}"));
            }
        }
    }
    Ok(())
}

/// A quoted string of codes, a quote mark written twice.
fn quoted(codes: impl IntoIterator<Item = u8>) -> Vec<u8> {
    let mut out = vec![b'\''];
    for code in codes {
        out.push(code);
        if code == b'\'' {
            out.push(code);
        }
    }
    out.push(b'\'');
    out
}

/// Each code a string of its own, so that no letter joins another: the
/// strings' quote marks alternate, so none is taken for a doubled one.
fn apart(codes: &[u8]) -> Vec<u8> {
    let mut out = Vec::new();
    for (index, &code) in codes.iter().enumerate() {
        let quote = if index % 2 == 0 { b'\'' } else { b'"' };
        out.push(quote);
        out.push(code);
        if code == quote {
            out.push(code);
        }
        out.push(quote);
    }
    out
}

#[test]
#[ignore = "needs the engine's checkout at ~/devel/neoteletext-renderer-teletext (or NEOTELETEXT_ENGINE); scripts/check.sh full runs it"]
fn every_code_of_every_provided_alphabet_draws_the_engines_glyph_for_its_character() {
    let names = font_sets();
    assert_eq!(names.len(), usize::from(PROVIDED_SETS), "PROVIDED_SETS must be the engine's FONT_SET_COUNT");
    let list = engine_file("crates/neoteletext-core/src/glyphs/required.txt");
    let glyphs = noto_glyphs();
    let codes: Vec<u8> = (0x20..=0x7E).collect();
    for (number, name) in names.iter().enumerate() {
        let alphabet = usize::from(FIRST_PROVIDED_ALPHABET) + number;
        let mut commands = format!("S(E){CELL}T(A{alphabet})").into_bytes();
        for (line, chunk) in codes.chunks(PER_LINE).enumerate() {
            commands.extend(format!("P[0,{}]T", line * LINE_STEP).bytes());
            commands.extend(apart(chunk));
        }
        let pixels = render(&format!("set-{number}"), &commands);
        let list_name = list_name(name);
        for (index, &code) in codes.iter().enumerate() {
            let position = format!("{:X}/{:X}", code >> 4, code & 0xF);
            let blank = vec![0; CELL_HEIGHT];
            // A position the engine's list leaves empty is blank in the set.
            let rows = listed_character(&list, &list_name, &position).map_or(&blank, |character| &glyphs[&character]);
            let (x, y) = (index % PER_LINE * CELL_WIDTH, index / PER_LINE * LINE_STEP);
            shows(&pixels, x, y, rows).unwrap_or_else(|fault| panic!("{name} {position}: {fault}"));
        }
    }
}

#[test]
#[ignore = "needs the engine's checkout at ~/devel/neoteletext-renderer-teletext (or NEOTELETEXT_ENGINE); scripts/check.sh full runs it"]
fn a_persian_word_is_joined_with_the_engines_forms() {
    let list = engine_file("crates/neoteletext-core/src/glyphs/required.txt");
    let forms = engine_file("crates/neoteletext-core/src/glyphs/forms.txt");
    let glyphs = noto_glyphs();
    let form = |letter: u32, name: &str| -> u32 {
        let wanted = format!("U+{letter:04X} {name}");
        let line = forms.lines().find(|line| line.ends_with(&wanted)).unwrap_or_else(|| panic!("no {wanted}"));
        u32::from_str_radix(&line[2..line.find(' ').unwrap()], 16).unwrap()
    };
    let code = |letter: u32| -> u8 {
        (0x20..=0x7E_u8)
            .find(|code| {
                listed_character(&list, "G0 Persian", &format!("{:X}/{:X}", code >> 4, code & 0xF)) == Some(letter)
            })
            .unwrap()
    };
    // سلام in visual order, meem alef lam seen: the engine's joining test
    // `a_word_joins_right_to_left` gives isolated, final, medial, initial.
    let (meem, alef, lam, seen) = (0x0645, 0x0627, 0x0644, 0x0633);
    let alphabet = usize::from(FIRST_PROVIDED_ALPHABET) + 44;
    let mut commands = format!("S(E){CELL}T(A{alphabet})P[0,0]T").into_bytes();
    commands.extend(quoted([meem, alef, lam, seen].map(code)));
    let pixels = render("persian", &commands);
    let expected = [meem, form(alef, "final"), form(lam, "medial"), form(seen, "initial")];
    for (index, point) in expected.into_iter().enumerate() {
        shows(&pixels, index * CELL_WIDTH, 0, &glyphs[&point])
            .unwrap_or_else(|fault| panic!("letter {index} (U+{point:04X}): {fault}"));
    }
}
