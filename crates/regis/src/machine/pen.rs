//! How drawing looks: the write controls and what a command carries along.

use crate::geometry::Point;
use crate::write::WriteControls;

/// The state of the pen.
#[derive(Clone, Copy, Debug, Default)]
pub(crate) struct Pen {
    /// The background colour, `S(I n)`.
    pub(crate) background: u8,
    /// The lasting write controls, set by the `W` command.
    pub(crate) write: WriteControls,
    /// Write controls in force only until the next key letter.
    pub(crate) temporary: Option<WriteControls>,
    /// Points drawn since the key letter: the place in pattern memory.
    pub(crate) phase: u32,
    /// The pixel the last drawing of this command ended on, so that a
    /// vector continuing from it does not draw that pixel twice.
    pub(crate) chain: Option<Point>,
}

impl Pen {
    /// The write controls now in force.
    pub(crate) fn controls(&self) -> WriteControls {
        self.temporary.unwrap_or(self.write)
    }

    /// The write controls an option changes: the temporary ones (created
    /// from the lasting ones on first use) or the lasting ones.
    pub(crate) fn controls_mut(&mut self, temporary: bool) -> &mut WriteControls {
        if temporary { self.temporary.get_or_insert(self.write) } else { &mut self.write }
    }

    /// A key letter: the pattern starts again and temporary controls end
    /// (manual chapter 3, "Pattern Control"; chapter 5, "Temporary Write
    /// Control").
    pub(crate) fn begin_command(&mut self) {
        self.temporary = None;
        self.phase = 0;
        self.chain = None;
    }
}
