//! What the renderer tells its caller besides pixels.

use alloc::vec::Vec;

/// Something the caller acts on; returned by the feeding calls in order.
#[derive(Clone, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum Event {
    /// The stream asked for a pause of this many ticks, 60 to the second
    /// (manual chapter 2, "Time Delay"). The caller owns the clock.
    Wait(u32),
    /// Bytes the terminal would send to the host: the answer to a report
    /// command (manual chapter 10). Each ends with a carriage return.
    Reply(Vec<u8>),
    /// The stream asked for the position of the graphics input cursor
    /// (`R(P(I))`). The library has no locator; the caller may answer.
    LocatorRequest,
    /// A work or size limit was reached and some input was not drawn.
    LimitReached,
    /// A ReGIS v2 commit was applied: the numbered drawings changed
    /// together, and the picture should be composed again. Holds the count
    /// of commits applied so far, which wraps to 0 after 4,294,967,295.
    Committed(u32),
    /// A ReGIS v2 commit failed: nothing changed, and the operations staged
    /// for it were dropped (`docs/regis-v2.md`, "Failed commits").
    CommitFailed {
        /// Why.
        error: CommitError,
        /// The drawing concerned, or 0.
        drawing: u16,
    },
}

/// Why a ReGIS v2 commit failed (`docs/regis-v2.md`, "Failed commits").
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum CommitError {
    /// The drawings' contents together passed
    /// [`MAX_DRAWING_CONTENT_BYTES`](crate::limits::MAX_DRAWING_CONTENT_BYTES).
    Contents,
    /// The operations staged passed
    /// [`MAX_STAGED_BYTES`](crate::limits::MAX_STAGED_BYTES).
    Staged,
    /// Drawings nest deeper than
    /// [`MAX_DRAWING_DEPTH`](crate::limits::MAX_DRAWING_DEPTH).
    Depth,
    /// A drawing can be reached from itself through parent and source links.
    Cycle,
    /// A drawing names a parent that does not exist.
    Parent,
    /// Painting the visible drawings would cost more than
    /// [`MAX_REBUILD_WORK`](crate::limits::MAX_REBUILD_WORK).
    Work,
    /// A `D` operation was malformed.
    Malformed,
    /// The drawings' records together passed
    /// [`MAX_DRAWING_RECORD_BYTES`](crate::limits::MAX_DRAWING_RECORD_BYTES):
    /// too many drawings exist at once (SOLVEDSECURITYISSUES 015).
    Drawings,
}

impl CommitError {
    /// The failure's code, 1 to 8.
    #[must_use]
    pub const fn code(self) -> u8 {
        match self {
            Self::Contents => 1,
            Self::Staged => 2,
            Self::Depth => 3,
            Self::Cycle => 4,
            Self::Parent => 5,
            Self::Work => 6,
            Self::Malformed => 7,
            Self::Drawings => 8,
        }
    }
}
