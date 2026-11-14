//! Helpers shared by the integration tests.
#![allow(dead_code, reason = "each test file uses its own part of these helpers")]

use std::path::PathBuf;

use regis::{Event, Font, Screen, Terminal};

/// The directory of test inputs and references.
pub fn data(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/data").join(name)
}

pub fn read(name: &str) -> Vec<u8> {
    std::fs::read(data(name)).unwrap()
}

/// The font shipped with the renderer.
pub fn font() -> Font {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../fonts/vt340.regisfont");
    Font::parse(&std::fs::read_to_string(path).unwrap()).unwrap()
}

/// A terminal with the shipped font.
pub fn terminal() -> Terminal {
    let mut terminal = Terminal::new();
    terminal.set_font(font());
    terminal
}

/// The result of drawing some ReGIS.
pub struct Drawn {
    pub screen: Screen,
    pub terminal: Terminal,
    pub events: Vec<Event>,
}

impl Drawn {
    /// The pixel at a position on the displayed page.
    pub fn at(&self, x: u32, y: u32) -> u8 {
        self.screen.displayed().pixel(x, y).unwrap()
    }

    /// How many pixels of the displayed page are not colour 0.
    pub fn inked(&self) -> usize {
        self.screen.displayed().pixels().iter().filter(|pixel| **pixel != 0).count()
    }

    /// The replies the stream asked for, as text.
    pub fn replies(&self) -> Vec<String> {
        let reply = |event: &Event| match event {
            Event::Reply(bytes) => Some(String::from_utf8_lossy(bytes).into_owned()),
            _ => None,
        };
        self.events.iter().filter_map(reply).collect()
    }
}

/// Draws bare ReGIS commands on a fresh screen.
pub fn draw(commands: &str) -> Drawn {
    let mut terminal = terminal();
    let mut screen = Screen::new();
    let mut events = terminal.feed_regis(&mut screen, commands.as_bytes());
    events.extend(terminal.finish(&mut screen));
    Drawn { screen, terminal, events }
}

/// Draws a terminal data stream (with device control strings).
pub fn draw_stream(stream: &[u8]) -> Drawn {
    let mut terminal = terminal();
    let mut screen = Screen::new();
    let mut events = terminal.feed(&mut screen, stream);
    events.extend(terminal.finish(&mut screen));
    Drawn { screen, terminal, events }
}
