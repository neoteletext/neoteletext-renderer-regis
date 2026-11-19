//! Where the parser stands: the command in force and the parentheses open.

use alloc::vec::Vec;

use super::frames::{Command, Context, Frame};
use crate::limits::MAX_NESTING;

/// The parser's position in the grammar.
#[derive(Debug, Default)]
pub(crate) struct Syntax {
    /// The command whose arguments are being read.
    pub(crate) command: Option<Command>,
    /// The parentheses open, outermost first.
    frames: Vec<Frame>,
    /// Parentheses open beyond the nesting limit: counted, not interpreted.
    overflow: usize,
}

// A parenthesis open is an entry of a small stack the memory bound counts.
const _: () = assert!(size_of::<Frame>() <= crate::limits::SMALL_STACK_ENTRY_BYTES);

impl Syntax {
    /// The memory the open parentheses hold.
    pub(crate) fn heap_bytes(&self) -> usize {
        crate::limits::vector_bytes(&self.frames)
    }

    /// The innermost open parenthesis's context.
    pub(crate) fn context(&self) -> Option<Context> {
        self.frames.last().map(|frame| frame.context)
    }

    /// At command level arguments belong to a command, not to an option:
    /// outside all parentheses, or directly inside a polygon fill.
    pub(crate) fn at_command_level(&self) -> bool {
        !self.skipping() && self.context().is_none_or(|context| context == Context::Fill)
    }

    /// Inside parentheses that are being skipped for being nested too deep.
    pub(crate) const fn skipping(&self) -> bool {
        self.overflow > 0
    }

    /// `(`: opens a context, or counts the parenthesis if too deep.
    pub(crate) fn open(&mut self, context: Context) {
        if self.skipping() || self.frames.len() >= MAX_NESTING {
            self.overflow = self.overflow.saturating_add(1);
        } else {
            self.frames.push(Frame { context, option: 0 });
        }
    }

    /// `)`: closes the innermost parenthesis; the frame closed, unless it
    /// was one being skipped.
    pub(crate) fn close(&mut self) -> Option<Frame> {
        if self.skipping() {
            self.overflow = self.overflow.saturating_sub(1);
            None
        } else {
            self.frames.pop()
        }
    }

    /// Closes everything, for a resynchronisation.
    pub(crate) fn reset(&mut self) {
        *self = Self::default();
    }

    /// The option letter that arguments now belong to.
    pub(crate) fn option_letter(&self) -> u8 {
        self.frames.last().map_or(0, |frame| frame.option)
    }

    pub(crate) fn set_option_letter(&mut self, letter: u8) {
        if let Some(frame) = self.frames.last_mut() {
            frame.option = letter;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::Syntax;
    use crate::limits::MAX_NESTING;
    use crate::machine::frames::Context;

    #[test]
    fn parentheses_past_the_limit_are_counted_and_unwound_in_order() {
        let mut syntax = Syntax::default();
        for _ in 0..MAX_NESTING + 5 {
            syntax.open(Context::Skip);
        }
        assert!(syntax.skipping());
        for _ in 0..5 {
            assert!(syntax.close().is_none());
        }
        assert!(!syntax.skipping());
        for _ in 0..MAX_NESTING {
            assert!(syntax.close().is_some());
        }
        assert!(syntax.close().is_none());
        assert!(syntax.at_command_level());
    }

    #[test]
    fn the_option_letter_belongs_to_the_innermost_parenthesis() {
        let mut syntax = Syntax::default();
        syntax.open(Context::Skip);
        syntax.set_option_letter(b'M');
        syntax.open(Context::Skip);
        assert_eq!(syntax.option_letter(), 0);
        syntax.close();
        assert_eq!(syntax.option_letter(), b'M');
    }
}
