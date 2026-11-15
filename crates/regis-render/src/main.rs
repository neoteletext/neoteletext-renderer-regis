//! `regis-render`: a ReGIS file in, a picture out.
//!
//! ```text
//! regis-render [--font FILE] [--page N] [--macrograph-reports] INPUT OUTPUT.png|OUTPUT.ppm
//! ```
//!
//! The input is a terminal data stream holding device control strings
//! (`ESC P p … ESC \`), or bare ReGIS commands. Replies the stream asks for
//! are written to standard error.
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

mod arguments;
mod checksum;
mod picture;
mod png;
mod ppm;

use std::fs;
use std::io::Read;
use std::path::Path;
use std::process::ExitCode;

use arguments::Arguments;
use regis::{Event, Font, Screen, Terminal};

/// The largest input file read: far above any real ReGIS picture.
const MAX_INPUT_BYTES: u64 = 16 * 1024 * 1024;

const ESCAPE: u8 = 0x1B;
const DCS: u8 = 0x90;

fn main() -> ExitCode {
    let arguments = match Arguments::parse(std::env::args().skip(1)) {
        Ok(arguments) => arguments,
        Err(message) => {
            eprintln!("regis-render: {message}\n{}", arguments::USAGE);
            return ExitCode::from(2);
        }
    };
    match run(&arguments) {
        Ok(()) => ExitCode::SUCCESS,
        Err(message) => {
            eprintln!("regis-render: {message}");
            ExitCode::FAILURE
        }
    }
}

/// Reads a file, refusing one over the limit.
/// The VT340's characters, used when no `--font` is given.
const VT340_FONT: &str = include_str!("../../../fonts/vt340.regisfont");

fn read_bounded(path: &str) -> Result<Vec<u8>, String> {
    let file = fs::File::open(path).map_err(|error| format!("{path}: {error}"))?;
    let mut bytes = Vec::new();
    file.take(MAX_INPUT_BYTES.saturating_add(1)).read_to_end(&mut bytes).map_err(|error| format!("{path}: {error}"))?;
    if u64::try_from(bytes.len()).unwrap_or(u64::MAX) > MAX_INPUT_BYTES {
        return Err(format!("{path}: larger than {MAX_INPUT_BYTES} bytes"));
    }
    Ok(bytes)
}

fn run(arguments: &Arguments) -> Result<(), String> {
    let mut terminal = Terminal::new();
    let font = match &arguments.font {
        Some(path) => {
            let text = String::from_utf8(read_bounded(path)?).map_err(|_| format!("{path}: not UTF-8 text"))?;
            Font::parse(&text).map_err(|error| format!("{path}: {error}"))?
        }
        None => Font::parse(VT340_FONT).map_err(|error| format!("built-in font: {error}"))?,
    };
    terminal.set_font(font);
    terminal.allow_macrograph_reports(arguments.macrograph_reports);

    let input = read_bounded(&arguments.input)?;
    let mut screen = Screen::new();
    // A stream with a device control string is fed as a terminal would
    // receive it; anything else is taken to be bare ReGIS commands.
    let framed = input.windows(2).any(|pair| pair == [ESCAPE, b'P']) || input.contains(&DCS);
    let mut events = if framed { terminal.feed(&mut screen, &input) } else { terminal.feed_regis(&mut screen, &input) };
    events.extend(terminal.finish(&mut screen));
    report(&events);

    // The numbered drawings of ReGIS v2 are painted over the page on show.
    let mut composed = regis::Picture::new();
    let page = match arguments.page {
        Some(number) => screen.page(number).ok_or_else(|| format!("no page {number}"))?,
        None if terminal.has_drawings() => {
            terminal.compose(&screen, &mut composed);
            eprintln!("numbered drawings: {} commits, {} work units", composed.commits(), composed.work());
            composed.canvas()
        }
        None => screen.displayed(),
    };
    let picture = picture::Picture { pixels: page.pixels(), width: page.width(), height: page.height() };
    let colours = terminal.palette().colours();
    let is_ppm =
        Path::new(&arguments.output).extension().is_some_and(|extension| extension.eq_ignore_ascii_case("ppm"));
    let bytes = if is_ppm { ppm::encode(&picture, &colours) } else { png::encode(&picture, &colours) };
    fs::write(&arguments.output, bytes).map_err(|error| format!("{}: {error}", arguments.output))
}

/// Tells the user what the stream asked of the terminal.
fn report(events: &[Event]) {
    for event in events {
        match event {
            // A reply holds bytes the stream chose; they are escaped so
            // they cannot act on the user's terminal.
            Event::Reply(bytes) => eprintln!("reply: {}", bytes.trim_ascii_end().escape_ascii()),
            Event::LimitReached => eprintln!("warning: a work limit was reached; the picture is incomplete"),
            Event::CommitFailed { error, drawing } => {
                eprintln!("warning: a commit failed with code {} ({error:?}), drawing {drawing}", error.code());
            }
            // Waits and locator requests mean nothing to a still picture.
            _ => {}
        }
    }
}
