//! `regis-minify`: a ReGIS file in, a shorter one out that draws the same.
//!
//! ```text
//! regis-minify [--no-verify] INPUT OUTPUT
//! ```
//!
//! Both files are drawn with the renderer and compared pixel for pixel.
//! If the shortest rewriting draws anything differently the careful one
//! is tried, and if that differs too the input is copied unchanged.
#![deny(
    clippy::indexing_slicing,
    clippy::string_slice,
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::arithmetic_side_effects,
    clippy::unreachable,
    clippy::todo,
    clippy::unimplemented
)]

use std::fs;
use std::io::Read;
use std::process::ExitCode;

use regis::{Event, Picture, Screen, Terminal};
use regis_minify::{Level, minify};

const USAGE: &str = "usage: regis-minify [--no-verify] INPUT OUTPUT";
/// The largest input file read.
const MAX_INPUT_BYTES: u64 = 16 * 1024 * 1024;
const ESCAPE: u8 = 0x1B;
const PAGES: usize = 2;

/// Everything a stream does that a viewer could tell: both pages, the
/// colours, and what it asked of the terminal.
#[derive(PartialEq)]
struct Drawn {
    pages: Vec<Vec<u8>>,
    colours: Vec<[u8; 3]>,
    events: Vec<Event>,
    /// The picture with its numbered drawings, each painted to its end
    /// whatever its paint rate, and the pick record: what ReGIS v2 shows.
    composed: Vec<u8>,
    picks: Vec<u16>,
}

fn draw(input: &[u8]) -> Drawn {
    let mut terminal = Terminal::new();
    let mut screen = Screen::new();
    let framed = input.windows(2).any(|pair| pair == [ESCAPE, b'P']);
    let mut events = if framed { terminal.feed(&mut screen, input) } else { terminal.feed_regis(&mut screen, input) };
    events.extend(terminal.finish(&mut screen));
    let pages = (0..PAGES).filter_map(|number| screen.page(number).map(|page| page.pixels().to_vec())).collect();
    let mut picture = Picture::new();
    let _ = terminal.advance_ticks(u32::MAX);
    terminal.compose(&screen, &mut picture);
    Drawn {
        pages,
        colours: terminal.palette().colours().to_vec(),
        events,
        composed: picture.canvas().pixels().to_vec(),
        picks: picture.pick_record().to_vec(),
    }
}

fn read(path: &str) -> Result<Vec<u8>, String> {
    let file = fs::File::open(path).map_err(|error| format!("{path}: {error}"))?;
    let mut bytes = Vec::new();
    file.take(MAX_INPUT_BYTES.saturating_add(1)).read_to_end(&mut bytes).map_err(|error| format!("{path}: {error}"))?;
    if u64::try_from(bytes.len()).unwrap_or(u64::MAX) > MAX_INPUT_BYTES {
        return Err(format!("{path}: larger than {MAX_INPUT_BYTES} bytes"));
    }
    Ok(bytes)
}

/// The shortest rewriting that draws the same, and what it is called.
fn shortest(input: &[u8], verify: bool) -> (Vec<u8>, &'static str) {
    if !verify {
        return (minify(input, Level::Exact), "exact rewriting, not checked");
    }
    let original = draw(input);
    for (level, name) in [(Level::Joined, "joined"), (Level::Exact, "exact")] {
        let candidate = minify(input, level);
        if draw(&candidate) == original {
            return (candidate, name);
        }
    }
    (input.to_vec(), "unchanged: no rewriting drew the same")
}

fn run(arguments: &[String]) -> Result<(), String> {
    let (verify, files) = match arguments.split_first() {
        Some((first, rest)) if first == "--no-verify" => (false, rest),
        _ => (true, arguments),
    };
    let [input_path, output_path] = files else { return Err(USAGE.to_owned()) };
    let input = read(input_path)?;
    let (output, how) = shortest(&input, verify);
    fs::write(output_path, &output).map_err(|error| format!("{output_path}: {error}"))?;
    println!("{} -> {} bytes ({how})", input.len(), output.len());
    Ok(())
}

fn main() -> ExitCode {
    let arguments: Vec<String> = std::env::args().skip(1).collect();
    match run(&arguments) {
        Ok(()) => ExitCode::SUCCESS,
        Err(message) => {
            eprintln!("regis-minify: {message}");
            ExitCode::FAILURE
        }
    }
}
