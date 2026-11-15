//! The pixel memory the renderer draws into: canvases and the screen.

mod canvas;
mod screen;

pub use canvas::Canvas;
pub(crate) use canvas::Window;
pub use screen::Screen;

/// The VT340's screen width in pixels (manual chapter 1, "VT300 Screen").
pub const SCREEN_WIDTH: u32 = 800;
/// The VT340's screen height in pixels.
pub const SCREEN_HEIGHT: u32 = 480;
