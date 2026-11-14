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
}
