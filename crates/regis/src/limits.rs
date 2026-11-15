//! Every bound on untrusted input, in one place.
//!
//! ReGIS arrives from a network. Each constant here caps memory or work that
//! the input could otherwise choose.

/// Coordinates are 16-bit signed integers (manual chapter 1, "Numeric Values").
pub const MAX_COORDINATE: i32 = 32_767;
/// The lowest coordinate, for the same reason.
pub const MIN_COORDINATE: i32 = -32_768;
/// Pixel positions are clamped to this distance from the origin after
/// display addressing scales them, so pixel arithmetic cannot overflow.
pub const MAX_PIXEL_COORDINATE: i32 = 65_535;

/// Parentheses nested deeper than this are skipped, not interpreted.
pub const MAX_NESTING: usize = 32;
/// Bytes kept from one bracketed extent `[x,y]`.
pub const MAX_EXTENT_BYTES: usize = 64;
/// Bytes kept from one quoted string.
pub const MAX_STRING_BYTES: usize = 1024;
/// Saved positions (manual chapter 4: "up to 16 positions in a stack").
pub const MAX_POSITION_STACK: usize = 16;
/// Saved sets of text controls for `T(B)`.
pub const MAX_TEXT_STACK: usize = 16;

/// Macrographs are named by the 26 letters (manual chapter 9).
pub const MACROGRAPHS: usize = 26;
/// Macrographs nest "up to 16 levels deep" (manual chapter 9).
pub const MAX_MACROGRAPH_DEPTH: usize = 16;
/// Bytes of macrograph definitions held in all (the VT300 stores "at least
/// 10,000").
pub const MAX_MACROGRAPH_BYTES: usize = 16_384;

/// Vertices one polygon fill keeps. The VT300 keeps 256; a circle drawn in
/// one-degree steps needs 360, so this renderer keeps more.
pub const MAX_FILL_VERTICES: usize = 4096;
/// Points one interpolated curve keeps (it only ever needs the last four,
/// plus the first three to close).
pub const CURVE_WINDOW: usize = 4;
/// Straight pieces drawn for one span of an interpolated curve.
pub const MAX_CURVE_PIECES: i32 = 64;

/// The pattern multiplier runs from 1 to 16 (manual chapter 3).
pub const MAX_PATTERN_MULTIPLIER: u32 = 16;
/// A character cell side in pixels: the height multiplier's largest value,
/// 256, times the ten rows of a stored cell (manual chapter 7).
pub const MAX_CELL_SIDE: i32 = 2560;
/// A time delay's largest value in ticks (manual chapter 2).
pub const MAX_WAIT_TICKS: i32 = 32_767;

/// Work units (pixels visited) granted for each input byte.
pub const WORK_PER_BYTE: u64 = 256;
/// Work units that can be saved up; also the allowance at start.
pub const WORK_CAP: u64 = 16_000_000;

/// The steps of painting (one drawing's contents replayed at one place) a
/// picture keeps so that the next repaint can be partial; a scene of more
/// is painted whole each time. At about 64 bytes a step, kept twice while
/// two lists are compared, this is 2 MB.
pub const MAX_DAMAGE_STEPS: usize = 16_384;

/// The work units of repainting numbered drawings a receiver may spend on
/// one picture each second ([`RepaintBudget`](crate::RepaintBudget)):
/// twice [`MAX_REBUILD_WORK`], so a picture at the cap of one repaint is
/// repainted twice a second, about 28 % of one Apple M1 processor at the
/// 140 milliseconds such a repaint was measured to take.
pub const REPAINT_WORK_PER_SECOND: u64 = 2 * MAX_REBUILD_WORK;
/// The most repaint allowance a receiver holds for one picture: one
/// second's worth, so a picture left alone can be repainted at once
/// several times over, and no more is ever saved up.
pub const REPAINT_WORK_HELD: u64 = REPAINT_WORK_PER_SECOND;

/// Numbered drawings of ReGIS v2 are named 1 to this (`docs/regis-v2.md`,
/// "The D command"); it is also the most that can exist.
pub const MAX_DRAWING_NUMBER: u16 = 65_535;
/// Bytes of contents all numbered drawings may hold together: about 25
/// times the largest painting measured (`docs/regis-v2.md`, "Bounds").
pub const MAX_DRAWING_CONTENT_BYTES: usize = 4 * 1024 * 1024;
/// Bytes of `D` operations and definitions that may wait for one commit: a
/// whole scene sent at once is its contents and about 3.4 MB of wrappers.
pub const MAX_STAGED_BYTES: usize = 8 * 1024 * 1024;
/// Drawings nested through parent and source links while painting; the
/// figure the manual gives for macrograph nesting (chapter 9).
pub const MAX_DRAWING_DEPTH: usize = 16;
/// Work units one repaint of every visible numbered drawing may cost. The
/// corpus's largest painting takes 67 million (14 November 2026).
pub const MAX_REBUILD_WORK: u64 = 100_000_000;
/// Work units each byte of a numbered drawing's contents costs each time
/// the drawing is painted: what a byte executed out of a macrograph costs.
pub const DRAWING_BYTE_COST: u64 = 64;
/// Work units each numbered drawing costs each time it is painted, with or
/// without contents, so that empty drawings reused many times over are
/// paid for.
pub const DRAWING_VISIT_COST: u64 = 64;

/// Events one feeding call returns; later ones are dropped.
pub const MAX_EVENTS: usize = 1024;
/// Bytes of font text [`crate::Font::parse`] accepts.
pub const MAX_FONT_BYTES: usize = 1024 * 1024;
/// Parameter digits kept from a device control string's introducer.
pub const MAX_DCS_PARAMETER: u32 = 9999;
/// Bytes of report replies that can be saved up; one more is earned for
/// each byte fed, so replies never outweigh the input by more than this.
pub const MAX_REPLY_BYTES: usize = 4096;
/// Ticks of delay that can be saved up (one minute); one more is earned
/// for each byte fed.
pub const MAX_SAVED_WAIT_TICKS: u32 = 3600;
/// Work units an arc pays for each degree it turns through.
pub const ARC_STEP_COST: u64 = 4;
