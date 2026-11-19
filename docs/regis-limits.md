# Limits that apply to ReGIS pages

An inventory of every limit a page drawn in ReGIS meets in NeoTeletext, read from the code as it stood on 16 November 2026. It was made at the owner's request, to decide which limits should be part of the sandbox's constraints. Nothing here changes a limit.

## What this covers

- The ReGIS renderer, the server, the wire and the browser, for pages drawn in ReGIS.
- **Not covered:** the teletext-side limits (page and sub-page counts, links on a teletext page, cookies, the server's budgets for each connection). They were not re-read for this inventory.
- **Not covered:** the audio limits.

## The picture

| Limit | Value | Where |
|---|---|---|
| Size | 800 × 480 pixels | renderer |
| Colours | 16 from a colour map | renderer |
| Graphics pages | 2 | renderer |
| Coordinates | −32,768 to 32,767 | renderer |

## ReGIS v1 drawing

Constants in `crates/regis/src/limits.rs`.

| Limit | Value |
|---|---|
| Macrographs | 26, each up to 16,384 bytes, nested 16 deep |
| Quoted string | 1,024 bytes |
| Polygon fill | 4,096 corners |
| Parenthesis nesting | 32 |
| Position stack, text stack | 16 each |
| Character cell side | 2,560 pixels |
| Pattern multiplier | 16 |
| Curve pieces between points | 64 |
| Drawing allowance | 256 work units earned per byte received, 16,000,000 held |
| Font file | 1 MB |
| Reply to the host | 4,096 bytes |
| Events returned per feed | 1,024 |

## ReGIS v2 numbered drawings

Constants in `crates/regis/src/limits.rs`.

| Limit | Value |
|---|---|
| Drawing numbers | 1 to 65,535 |
| All drawings' contents together | 4 MB |
| All drawings' records together (`MAX_DRAWING_RECORD_BYTES`), each charged `DRAWING_RECORD_BYTES` (273 bytes on a 64-bit target) and 144 more once it has a label or a frame list (`DRAWING_EXTRAS_BYTES`) | 4 MB, about 15,000 drawings at once (`MOST_DRAWINGS_HELD`); past it the commit fails with code 8 |
| What a terminal or validator holds (`MAX_HELD_BYTES`), besides the font and provided sets its caller gives; each reports its own figure with `held_bytes()` | 22,751,666 bytes on a 64-bit target |
| Bytes waiting for one commit | 8 MB |
| Nesting through parents, sources, looks and frames | 16 |
| Work of one repaint | 100,000,000 units |
| Cost per drawing painted, and per byte replayed | 64 units each |
| Steps kept for partial repaint | 16,384 (more means a whole repaint every time) |
| Rectangles of one partial repaint | 8 (more are joined); a step added or taken away is matched 64 steps ahead |
| Repaint budget | 200,000,000 units a second, 200,000,000 held |

## ReGIS v2 animation

Added on 18 November 2026 with the renderer's animation (REQ-6530; neoteletext-common `docs/animation.md`, ANIM-16). Constants in `crates/regis/src/limits.rs`. The renderer keeps the first four; the receiver keeps the rest, which are about time.

| Limit | Value | What happens past it |
|---|---|---|
| Layers (`LAYERS`) | 4 | A `G` above 3 is malformed: the commit fails with code 7 |
| Entries in a frame list (`MAX_OBJECT_FRAMES`) | 16 | A seventeenth is malformed: the commit fails with code 7 |
| Paint rate (`MAX_PAINT_RATE`) | 32,767, each unit 100 work units a second (`PAINT_RATE_UNIT`) | A greater rate is malformed: the commit fails with code 7 |
| A drawing's label, `U` (`MAX_LABEL_BYTES`), and a picture's title, `D(T'...')` (`MAX_TITLE_BYTES`) | 64 bytes each, plain text | Longer, or holding a backspace, tab, line feed or carriage return: malformed, the commit fails with code 7 |
| Drawings holding animation state (`MAX_ANIMATED_DRAWINGS`) | 64 | The state is refused |
| Drawings cycling their frames (`MAX_CYCLING_OBJECTS`) | 16 | The cycle does nothing and is counted |
| Pixels repainted for animation in one tick (`MAX_ANIMATION_PIXELS_PER_TICK`) | 192,000, half a picture | The painting waits for the next tick |
| Work for animation in one tick (`ANIMATION_WORK_PER_TICK`) | 3,333,333 units, a sixtieth of the repaint budget's second | The painting waits for the next tick |
| Showing a drawing the stream hid | The scene must still cost no more than 100,000,000 units to repaint | The shown part of the state is refused |

## Server and wire

| Limit | Value |
|---|---|
| ReGIS in one program message | 1 MB |
| ReGIS in one record | 8,174 bytes |
| Drawing links per sub-page | 64 |
| Link target | 8,004 bytes |
| Cursor drawings per sub-page | 1 |

## Browser

| Limit | Value | What happens past it |
|---|---|---|
| ReGIS sub-pages kept by a channel's decoder | 4 | The one fed longest ago is forgotten |
| Clickable drawings per picture | 1,024 | All hover, looks, links and picking stop, silently |
| Pick record runs per picture | 12,000 | Same: all stop, silently |
| Pick records kept by the trusted core | 16 pages | The lowest page is forgotten |
| Drawing-link sets kept | 16 pages | The lowest page is forgotten |
| Hover and pressed looks | 1 of each per drawing | — |
| Cursor blink | 530 ms half-period, fixed | — |
| Link click dwell | 500 ms | The click is refused |

A run is one drawing's pixels along one row of the picture. Many small separate clickable drawings (a grid, a keyboard of small keys, a chart of thin marks) are what use runs up. The two silent bounds are recorded as rdm TODO-6386.

## Input

| Limit | Value |
|---|---|
| Pointer precision | One picture pixel when the picture is shown 800 wide or more; coarser when shown smaller |
| Hover events | Mouse or pen only, never a finger |

## Observations for the sandbox

- **Most of the v2 caps are far above what a page uses.** The largest demonstration page costs about 1.3 million work units against a cap of 100 million, and a few kilobytes against 4 MB. They are safety limits, not creative constraints.
- **The limits an author actually meets are the plain ones:** 16 colours, 800 × 480, 64 links, and the two silent pick-record bounds.
- **Constraints that force creativity would be new, much lower numbers,** for example a few hundred drawings, a small total byte size for a page, or a repaint budget low enough to feel. Those are design choices; the code would take them as changed constants.
