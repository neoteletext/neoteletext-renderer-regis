//! `regis-render`: a ReGIS file in, a picture out.
//!
//! ```text
//! regis-render [--font FILE] [--page N] [--macrograph-reports] INPUT OUTPUT.png|OUTPUT.ppm
//! regis-render --frames TICKS [--frame-step K] [--script FILE|-] INPUT OUTPUT.png|OUTPUT.ppm
//! ```
//!
//! With `--frames` the picture's animation is played for that many ticks
//! of a sixtieth of a second and an image written every `K` ticks and at
//! the last, each named with its tick in five digits before the
//! extension (neoteletext-common `docs/animation.md`, ANIM-18). The
//! script is a text file of timed operations on numbered drawings, one a
//! line: `<tick> <operation> <drawing> ...`.
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
mod play;
mod png;
mod ppm;

use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};
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

    if let Some(frames) = arguments.frames {
        return play_frames(arguments, &mut terminal, &screen, frames);
    }
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
    write_image(Path::new(&arguments.output), page, &terminal)
}

/// Writes one page as the kind of image the path's extension names.
fn write_image(path: &Path, page: &regis::Canvas, terminal: &Terminal) -> Result<(), String> {
    let picture = picture::Picture { pixels: page.pixels(), width: page.width(), height: page.height() };
    let colours = terminal.palette().colours();
    let is_ppm = path.extension().is_some_and(|extension| extension.eq_ignore_ascii_case("ppm"));
    let bytes = if is_ppm { ppm::encode(&picture, &colours) } else { png::encode(&picture, &colours) };
    fs::write(path, bytes).map_err(|error| format!("{}: {error}", path.display()))
}

/// The output path with `-NNNNN`, the tick in five digits, before its
/// extension (ANIM-18).
fn frame_path(output: &str, tick: u32) -> PathBuf {
    let path = Path::new(output);
    let stem = path.file_stem().unwrap_or_default().to_string_lossy();
    let name = match path.extension() {
        Some(extension) => format!("{stem}-{tick:05}.{}", extension.to_string_lossy()),
        None => format!("{stem}-{tick:05}"),
    };
    path.with_file_name(name)
}

/// Reads the script: a file, or standard input for `-`.
fn read_script(path: &str) -> Result<Vec<play::Timed>, String> {
    let bytes = if path == "-" {
        let mut bytes = Vec::new();
        let input = std::io::stdin().lock().take(MAX_INPUT_BYTES.saturating_add(1)).read_to_end(&mut bytes);
        input.map_err(|error| format!("standard input: {error}"))?;
        if u64::try_from(bytes.len()).unwrap_or(u64::MAX) > MAX_INPUT_BYTES {
            return Err(format!("standard input: larger than {MAX_INPUT_BYTES} bytes"));
        }
        bytes
    } else {
        read_bounded(path)?
    };
    let text = String::from_utf8(bytes).map_err(|_| format!("{path}: not UTF-8 text"))?;
    play::parse(&text).map_err(|error| format!("{path}: {error}"))
}

/// Plays the picture for `frames` ticks from tick 0: at each tick the
/// script's operations due are carried out and every cycling drawing
/// given its frame, an image is written if the tick is one to write, and
/// then the tick passes for the paint rate (ANIM-18). Nothing in it
/// depends on a clock or on the machine, so a run's images are the same
/// every time.
fn play_frames(arguments: &Arguments, terminal: &mut Terminal, screen: &Screen, frames: u32) -> Result<(), String> {
    let script = match &arguments.script {
        Some(path) => read_script(path)?,
        None => Vec::new(),
    };
    let mut due = script.into_iter().peekable();
    let mut player = play::Player::default();
    let mut composed = regis::Picture::new();
    let mut written = 0_u32;
    for tick in 0..=frames {
        while let Some(timed) = due.next_if(|timed| timed.tick <= tick) {
            player.apply(terminal, tick, timed);
        }
        player.tick(terminal, tick);
        if tick.checked_rem(arguments.frame_step) == Some(0) || tick == frames {
            terminal.compose(screen, &mut composed);
            write_image(&frame_path(&arguments.output, tick), composed.canvas(), terminal)?;
            written = written.saturating_add(1);
        }
        if tick != frames {
            let _ = terminal.advance_ticks(1);
        }
    }
    eprintln!("{written} images of {frames} ticks; {} operations did nothing", player.ignored);
    Ok(())
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
