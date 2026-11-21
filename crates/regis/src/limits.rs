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
/// The most rectangles a picture's repaint is kept as: each is put back
/// and repainted on its own, so changes far apart do not repaint what lies
/// between them; past this the two whose joining adds least are joined.
/// Eight holds a key and a meter and a few names changed in one frame
/// (the music player's page, page-rules.md RULE-92) with room to spare,
/// and bounds the steps replayed for more than one rectangle.
pub const MAX_DAMAGE_RECTANGLES: usize = 8;
/// How many steps ahead the repaint looks for a step that was added to or
/// taken from the list since the last picture, to match the rest; past
/// it, the two steps are both taken as changed. It bounds the comparison
/// at this many times [`MAX_DAMAGE_STEPS`], and a scene that shows or
/// hides a drawing of up to this many steps is matched exactly.
pub const DAMAGE_LOOKAHEAD_STEPS: usize = 64;

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
/// Bytes the records of the numbered drawings of one scene may hold
/// together, as [`DRAWING_RECORD_BYTES`] and [`DRAWING_EXTRAS_BYTES`]
/// charge them: the same 4 MiB the contents may hold, so that a stream of
/// drawings holds no more for their records than for their contents. A
/// drawing whose record would pass it is not made and fails its batch with
/// code 8 ([`CommitError::Drawings`](crate::CommitError::Drawings)). It
/// lets about 15,000 drawings exist at once (asserted below at 12,000 or
/// more on 64-bit targets), where a picture of the corpus uses hundreds;
/// without it 65,535 empty drawings, 447,643 bytes of stream, held 55 MB
/// (SOLVEDSECURITYISSUES 015).
pub const MAX_DRAWING_RECORD_BYTES: usize = 4 * 1024 * 1024;
/// The memory one numbered drawing's record holds in one scene, at most:
/// the record itself, its share of the nodes of the scene's map of
/// drawings and of its list of children, and the header of the
/// allocation that holds its contents when it has any (their bytes count
/// against [`MAX_DRAWING_CONTENT_BYTES`]). The committed and the staged
/// scene each hold their own.
pub const DRAWING_RECORD_BYTES: usize = held_allocation(size_of::<crate::scene::Drawing>())
    .saturating_add(ordered_map_entry_bytes(size_of::<u16>(), size_of::<usize>()))
    .saturating_add(ordered_map_entry_bytes(size_of::<(u16, u16)>(), 0))
    .saturating_add(held_allocation(CONTENTS_HEADER_BYTES));
/// The memory a drawing's frame list and label hold, kept apart from its
/// record and only once one of them is given.
pub const DRAWING_EXTRAS_BYTES: usize = held_allocation(size_of::<crate::scene::Extras>());
/// The most numbered drawings one scene can hold at once: as many records
/// as [`MAX_DRAWING_RECORD_BYTES`] pays for.
pub const MOST_DRAWINGS_HELD: usize = MAX_DRAWING_RECORD_BYTES / DRAWING_RECORD_BYTES;

/// What the allocator adds to one allocation, at most, beyond the bytes
/// asked for: glibc's malloc keeps an 8-byte header and rounds a chunk up
/// to 16 bytes (up to 23 more), and macOS's rounds to 16; 32 covers both.
pub const ALLOCATION_OVERHEAD_BYTES: usize = 32;
/// The counts in front of shared contents: the strong and the weak count.
const CONTENTS_HEADER_BYTES: usize = 2 * size_of::<usize>();
/// The entries one node of the standard library's ordered map and set
/// holds: its B-tree has B = 6, so a node holds up to 2B - 1.
const ORDERED_MAP_NODE_ENTRIES: usize = 11;
/// The entries every node of those trees but the root holds at least,
/// B - 1, which insertion and removal both keep.
const ORDERED_MAP_LEAST_ENTRIES: usize = 5;

/// The memory one allocation of `bytes` holds, at most.
pub(crate) const fn held_allocation(bytes: usize) -> usize {
    bytes.saturating_add(ALLOCATION_OVERHEAD_BYTES)
}

/// The memory a vector's allocation holds: none until it has room.
pub(crate) fn vector_bytes<T>(vector: &alloc::vec::Vec<T>) -> usize {
    match vector.capacity() {
        0 => 0,
        capacity => held_allocation(capacity.saturating_mul(size_of::<T>())),
    }
}

/// The memory an ordered map with keys of `key` bytes and values of
/// `value` bytes (a set has none) holds for each entry, at most, its root
/// aside: a node is at most a branch node (a parent pointer, two 16-bit
/// counts, its keys and values and one edge more than its entries), and
/// every node but the root holds at least [`ORDERED_MAP_LEAST_ENTRIES`].
pub(crate) const fn ordered_map_entry_bytes(key: usize, value: usize) -> usize {
    let pointer = size_of::<usize>();
    let entries = ORDERED_MAP_NODE_ENTRIES.saturating_mul(key.saturating_add(value));
    let edges = ORDERED_MAP_NODE_ENTRIES.saturating_add(1).saturating_mul(pointer);
    let node = pointer.saturating_add(size_of::<[u16; 2]>()).saturating_add(entries).saturating_add(edges);
    held_allocation(node.next_multiple_of(pointer)).div_ceil(ORDERED_MAP_LEAST_ENTRIES)
}

/// The memory a map's root node holds when it is the only node, at most;
/// [`ordered_map_entry_bytes`] counts every other node.
pub(crate) const fn ordered_map_root_bytes(key: usize, value: usize) -> usize {
    ordered_map_entry_bytes(key, value).saturating_mul(ORDERED_MAP_LEAST_ENTRIES)
}

/// The memory one interpreter's working state can hold besides its
/// structure and its macrographs, at most: a macrograph definition in
/// progress and the macrograph bodies grown by doubling (twice
/// [`MAX_MACROGRAPH_BYTES`] each), a polygon fill's vertices grown by
/// doubling, a quoted string and an extent kept by the lexer (each held
/// twice, as read and as a token), the events and replies of one feeding
/// call, and `SMALL_STACK_ENTRIES` entries of the small stacks, each
/// entry at most `SMALL_STACK_ENTRY_BYTES`.
pub const INTERPRETER_HEAP_BYTES: usize = (4 * MAX_MACROGRAPH_BYTES + MACROGRAPHS * ALLOCATION_OVERHEAD_BYTES)
    .saturating_add(held_allocation(2 * MAX_FILL_VERTICES * size_of::<crate::geometry::Point>()))
    .saturating_add(2 * held_allocation(2 * MAX_STRING_BYTES))
    .saturating_add(2 * held_allocation(2 * MAX_EXTENT_BYTES))
    .saturating_add(held_allocation(MAX_EVENTS * size_of::<crate::Event>()))
    .saturating_add(MAX_REPLY_BYTES.saturating_add(MAX_EVENTS * ALLOCATION_OVERHEAD_BYTES))
    .saturating_add(SMALL_STACK_ENTRIES * held_allocation(SMALL_STACK_ENTRY_BYTES));
/// Entries of the small stacks an interpreter keeps: saved positions, saved
/// text controls, open parentheses, macrograph calls and a curve's points
/// (each in its own vector).
const SMALL_STACK_ENTRIES: usize =
    MAX_POSITION_STACK + MAX_TEXT_STACK + MAX_NESTING + MAX_MACROGRAPH_DEPTH + 2 * CURVE_WINDOW + 16;
/// The largest entry of those stacks, asserted against each entry type
/// where the type is defined.
pub(crate) const SMALL_STACK_ENTRY_BYTES: usize = 64;

/// The memory a [`Terminal`](crate::Terminal) or a
/// [`Validator`](crate::Validator) holds, at most, besides the font and
/// the provided sets its caller gives it (which
/// [`Terminal::held_bytes`](crate::Terminal::held_bytes) counts as given):
/// its own structure; the committed and the staged scene, each with
/// [`MAX_DRAWING_RECORD_BYTES`] of records and
/// [`MAX_DRAWING_CONTENT_BYTES`] of contents; a definition being received;
/// the receiver's per-drawing painting state; two interpreters (the
/// stream's and the one that replays drawings) and a third copy of the
/// macrographs; the roots of the scene's maps; and three copies of the
/// loadable character sets, once loaded. About 22 MB on a
/// 64-bit target. A server counts each reader at
/// [`Terminal::held_bytes`](crate::Terminal::held_bytes), or at this
/// figure without asking (SOLVEDSECURITYISSUES 015).
pub const MAX_HELD_BYTES: usize = size_of::<crate::Validator>()
    .saturating_add(2 * (MAX_DRAWING_RECORD_BYTES + MAX_DRAWING_CONTENT_BYTES))
    .saturating_add(held_allocation(MAX_DRAWING_CONTENT_BYTES))
    .saturating_add(crate::scene::most_painting_state_bytes())
    .saturating_add(2 * INTERPRETER_HEAP_BYTES + MAX_MACROGRAPH_BYTES + MACROGRAPHS * ALLOCATION_OVERHEAD_BYTES)
    .saturating_add(crate::scene::MAP_ROOTS_BYTES)
    .saturating_add(LOADABLE_COPIES * crate::machine::LOADABLE_SETS * crate::font::MOST_LOADABLE_HEAP_BYTES);
/// The copies of the loadable sets a terminal holds: the stream's
/// interpreter's, the replaying interpreter's and the committed tables'.
const LOADABLE_COPIES: usize = 3;

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

/// The layers of a picture, drawn in this order: 0 the background, 1 the
/// scene, 2 the sprites, 3 the overlay (animation.md, ANIM-12). A
/// top-level numbered drawing names its layer with the attribute `G`.
pub const LAYERS: u8 = 4;
/// The layer of a numbered drawing that names none (ANIM-10): the scene.
pub const DEFAULT_LAYER: u8 = 1;
/// The numbered drawings that may hold animation state at once
/// ([`Terminal::set_animation`](crate::Terminal::set_animation); ANIM-9,
/// ANIM-16).
pub const MAX_ANIMATED_DRAWINGS: usize = 64;
/// The drawings of one page a receiver cycles through their frames at once
/// (ANIM-14, ANIM-16). The renderer keeps no clock and cycles nothing: the
/// receiver that sets each drawing's frame holds a page to this.
pub const MAX_CYCLING_OBJECTS: usize = 16;
/// The entries of a drawing's frame list, `F(...)`, and so the frames an
/// animation state can name: 0 to one less than this (ANIM-10, ANIM-14).
pub const MAX_OBJECT_FRAMES: usize = 16;
/// The pixels a receiver repaints for animation in one tick: half a
/// picture of 800 by 480 (ANIM-16). A starting value, to be confirmed by
/// measurement. The receiver holds a page to it; damage that would pass
/// it waits for the next tick.
pub const MAX_ANIMATION_PIXELS_PER_TICK: u32 = 192_000;
/// The ticks of the presentation clock in a second (ANIM-1); also the
/// ticks of a v1 time delay (manual chapter 2, "Time Delay").
pub const TICKS_PER_SECOND: u64 = 60;
/// The work units a receiver spends repainting for animation in one tick:
/// a tick's share of [`REPAINT_WORK_PER_SECOND`], taken from that same
/// budget (ANIM-16).
pub const ANIMATION_WORK_PER_TICK: u64 = REPAINT_WORK_PER_SECOND / TICKS_PER_SECOND;
/// The work units a second that one unit of the paint rate attribute,
/// `T`, paints: `T1` is a pen moving 100 pixels a second (DEC-6382 item
/// 1; ANIM-15).
pub const PAINT_RATE_UNIT: u64 = 100;
/// The largest paint rate: a 16-bit signed value, as every attribute is
/// (DEC-6382 item 1; ANIM-10).
pub const MAX_PAINT_RATE: u16 = 32_767;
/// The numbered drawings a receiver may watch at once for their painting
/// finishing ([`Terminal::watch_painted`](crate::Terminal::watch_painted);
/// DEC-7031 items 2 and 4): as many as one page's rule table may name in
/// its "painted" triggers, the engine's `MAX_PAINT_WATCHED`, which a
/// caller linking both asserts equal. Reports are of watched drawings
/// alone, so their number in any one call is bounded by this.
pub const MAX_WATCHED_PAINTINGS: usize = 32;
/// Bytes of a stream a [`Replay`](crate::Replay) holds while it feeds
/// them at a pace (DEC-7031 items 1 to 3): 256 KiB, the figure the NAPLPS
/// renderer holds for a stream that is waiting, which a receiver linking
/// both asserts equal. A push that would hold more ends the pacing and
/// feeds everything at once, so a replay holds no more than this and
/// lasts no longer than this many bytes take at its pace: 524,288 ticks
/// at 30 bytes a second, the slowest line a page may state.
pub const MAX_REPLAY_HELD_BYTES: usize = 262_144;

/// Bytes of a numbered drawing's label, the attribute `U`: the text a
/// screen reader speaks for it (DEC-6638 item 6, row 6.1). The same bound
/// as a sound's label in the engine's OBJECT record (REQ-37) and an object's
/// label in the NAPLPS renderer.
pub const MAX_LABEL_BYTES: usize = 64;
/// Bytes of a picture's title, `D(T'...')`: the name a page list shows for
/// the picture when the program's index gives none (DEC-6638 item 3, row
/// 2.8). One short line, as a label is.
pub const MAX_TITLE_BYTES: usize = 64;

/// The alphabet of the first of NeoTeletext's provided character sets:
/// `T(A100)` selects set 0 and `T(A154)` set 54 (`docs/regis-v2.md`,
/// "Provided character sets"; renderer-parity.md PAR-12). The VT340's own
/// alphabets are 0 to 3 (manual chapter 7, "Select Character Set"), so the
/// numbers between stay unused.
pub const FIRST_PROVIDED_ALPHABET: u8 = 100;
/// The provided character sets (DEC-6669 item 1): the engine's
/// `FONT_SET_COUNT`, which each caller linking both asserts equal.
pub const PROVIDED_SETS: u8 = 55;
/// The positions of a provided set: 2/0 to 7/15, as every teletext set has.
pub const PROVIDED_SET_POSITIONS: usize = 96;
/// The widest glyph of a provided set, in pixels: one `u16` row (the
/// engine's glyphs are 12 wide).
pub const MAX_PROVIDED_GLYPH_WIDTH: u8 = 16;
/// The tallest glyph of a provided set, in pixels (the engine's are 20).
pub const MAX_PROVIDED_GLYPH_HEIGHT: u8 = 32;

// A glyph row is one u16, its most significant used bit the left pixel.
const _: () = assert!(MAX_PROVIDED_GLYPH_WIDTH as u32 == u16::BITS);
// 2/0 to 7/15.
const _: () = assert!(PROVIDED_SET_POSITIONS == 0x80 - 0x20);
// The last provided alphabet, 154, fits the u8 the alphabets are named in.
const _: () = assert!(FIRST_PROVIDED_ALPHABET as u32 + PROVIDED_SETS as u32 - 1 == 154);

// Half a picture, as ANIM-16 derives the figure.
const _: () =
    assert!(MAX_ANIMATION_PIXELS_PER_TICK == crate::surface::SCREEN_WIDTH * crate::surface::SCREEN_HEIGHT / 2);
// ANIM-16 states the figure: a sixtieth of the repaint budget.
const _: () = assert!(ANIMATION_WORK_PER_TICK == 3_333_333);

/// Events one feeding call returns; later ones are dropped.
pub const MAX_EVENTS: usize = 1024;
/// Bytes of font text [`crate::Font::parse`] accepts.
pub const MAX_FONT_BYTES: usize = 1024 * 1024;
/// Parameter digits kept from a device control string's introducer.
pub const MAX_DCS_PARAMETER: u32 = 9999;
/// Bytes of report replies that can be saved up; one more is earned for
/// each byte fed, so replies never outweigh the input by more than this.
pub const MAX_REPLY_BYTES: usize = 4096;
// The cap on records lets a scene hold many more drawings than a picture
// of the corpus uses.
#[cfg(target_pointer_width = "64")]
const _: () = assert!(MOST_DRAWINGS_HELD >= 12_000);
// The two scene caps are the same figure, as their documents say.
const _: () = assert!(MAX_DRAWING_RECORD_BYTES == MAX_DRAWING_CONTENT_BYTES);

/// Ticks of delay that can be saved up (one minute); one more is earned
/// for each byte fed.
pub const MAX_SAVED_WAIT_TICKS: u32 = 3600;
/// Work units an arc pays for each degree it turns through.
pub const ARC_STEP_COST: u64 = 4;
