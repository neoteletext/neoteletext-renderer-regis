# 015 — Empty numbered drawings hold memory out of all proportion to their bytes, and nothing reports it

**Found:** 19 November 2026, by NeoTeletext's server while counting what its kept pictures cost (the server's TODO-6836 and DEC-6842; renderer-parity.md threat 1, PAR-15, PAR-17 and PAR-23). Recorded for this repository as TODO-6843.
**Class:** resource exhaustion (memory).

`docs/regis-v2.md` ("Lifetime and memory") says a renderer's memory is
bounded by the contents cap, the staged cap and a constant. That is true,
but the constant is large and a short stream chooses it. Each numbered
drawing is a record of about 220 bytes of attributes (its label of 64
bytes and its frame list of 16 numbers kept in place whether used or
not), kept in an ordered map in the staged scene and again in the
committed one, with an entry in each scene's list of children and an
allocation for its empty contents. Nothing counts the records against any
cap: the contents cap counts only the bytes of contents, and the
drawings cap is the number range, 65,535.

Measured on 19 November 2026 (release build, x86_64 Linux, the process's
resident size):

| Stream | Bytes | Held by one `regis::Validator` |
| --- | --- | --- |
| nothing | 0 | 15,540 |
| every macrograph filled, two commits | — | 83,476 |
| `D(S1,S2,...,S9999,C)` | 58,891 | 8,446,976 |
| `D(S1,S2,...,S65535,C)` | 447,643 | 55,148,544 |

About 841 bytes a drawing, or about 123 bytes held for each byte of the
largest stream. A drawing costs at least three bytes of stream (`S1,`),
so a stream can make a reader hold up to roughly 280 bytes for each of
its bytes. The browser holds the same terminal for each picture it shows,
so one page of under half a megabyte costs a receiver 55 MB too.

The library reports no figure for this. A server that keeps a
`regis::Validator` for every kept picture and for each connection's own
stream cannot count what each holds, so it counts a fixed 131,072 bytes
(`REGIS_READER_BYTES`) and is wrong by a factor of 400 for this stream.

**Fix wanted:** a named bound in `crates/regis/src/limits.rs`, with its
derivation, on what a stream can make the renderer hold, so that what it
holds is comparable to its bytes; and a figure the library reports, an
upper bound of what a terminal or a validator holds, that a server can
count.

**Test wanted:** the 65,535-drawing stream: the figure reported is within
the bound, the resident size it causes is within the figure, and on the
old code the stream held far more than the bound.

## Resolution (19 November 2026)

What the drawings' records hold is now a cap of its own, and the figure
is reported (DEC-6850, needing the owner's confirmation; TODO-6843).

- **A sixth cap**, `MAX_DRAWING_RECORD_BYTES`, 4 MiB, the contents cap's
  figure: each drawing is charged `DRAWING_RECORD_BYTES` (273 bytes on a
  64-bit target: the record, its share of the nodes of the scene's map of
  drawings and of its list of children, whose nodes the standard library
  keeps at least five entries full, and its contents' allocation header,
  each allocation with 32 bytes for the allocator), and
  `DRAWING_EXTRAS_BYTES` (144) more once it has a label or a frame list.
  About 15,000 drawings (`MOST_DRAWINGS_HELD`) can exist at once. A
  drawing past the cap is not made, as the operation arrives, and its
  batch fails with the new code 8, `CommitError::Drawings`; a drawing
  deleted gives its charge back.
- **A smaller record**: the label and the frame list are kept apart and
  made only when given; records are boxed in the map; every empty drawing
  shares its scene's one empty contents; a definition being received
  grows only up to the contents cap; and at a commit the old scene is
  dropped before the new copy is made, so no more than two are held.
- **The figure**: `Terminal::held_bytes()` and `Validator::held_bytes()`,
  an upper bound of what one holds between calls (both scenes' records
  and contents, shared contents once; a definition being received; the
  painting and animation state; both interpreters' working state and the
  committed copy of the macrographs; the font and provided sets given).
  A new validator reports 19,344 bytes. `MAX_HELD_BYTES`, 22,751,666 bytes
  on a 64-bit target, is the most either holds besides the font and the
  provided sets, derived from the limits.

The issue's stream now fails its commit at drawing 15,364; just before
the commit the validator's resident size had grown by 2,461,696 bytes,
against 4,213,487 reported and the bound of 22,751,666 (x86_64 Linux,
release build). On the old code the same point had grown it by
28,749,824 bytes, and 55,148,544 after the commit.

**Tests:** `crates/regis/tests/resident.rs`,
`the_issue_stream_of_65535_empty_drawings_holds_no_more_than_reported_and_bounded`
(Linux; fails on the old code, with the bound written in, at 28,749,824
bytes against 22,751,666); `crates/regis/tests/memory.rs`: the cap at its
value and one past it, records given back on deletion, labels and frame
lists charged, shared contents counted once, the heaviest state a stream
can make within `MAX_HELD_BYTES`, and a terminal's figure against a
validator's.

**Still to do:** NeoTeletext's server counts each reader at
`held_bytes()` instead of its fixed `REGIS_READER_BYTES` (its
TODO-6845). The cap's figure awaits the owner's confirmation.
