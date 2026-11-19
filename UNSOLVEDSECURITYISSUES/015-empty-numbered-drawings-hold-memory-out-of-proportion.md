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
