//! Text: the controls the `T` command sets, and drawing a character.

mod controls;
mod draw;

pub(crate) use controls::TextControls;
pub(crate) use draw::draw_character;
