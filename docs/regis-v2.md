# ReGIS v2: proposal

Nov 14, 2026 · @Andrew Stuart

## Summary

ReGIS v2 is ReGIS v1, unchanged, plus one command that lets a drawing be numbered so the display can move, hide, replace, reorder and delete it. No drawing command, pixel, colour or text behaviour changes.

The changes, all additive:

1. One new command, `D`, to define, change and delete numbered drawings.
2. Seven attributes per numbered drawing: position, depth, visible, parent, source, detectable, clip rectangle.
3. The renderer keeps numbered drawings and repaints the screen itself when one changes.
4. Commit: staged changes appear together, or are rejected together.
5. A defined starting state for each numbered drawing, so nothing leaks between them.
6. Screen, load and report commands are ignored inside a numbered drawing.
7. A pick record names the drawing under each pixel, so a screen position can be resolved to a drawing.

This document covers the language and the renderer only. How the bytes are carried, how input reaches the application, and how a server keeps and resends a scene are outside its scope: the system that embeds the renderer defines them.

## Requirements

These are the constraints the design was held to.

- **Backward compatible.** The renderer must render existing ReGIS exactly as it does today.
- **No change to graphical capability.** v2 changes how the system works, not what can be drawn.
- **Easier applications.** The purpose is to make it easy to change what is on the display.
- **Minimal.** Good features stay, but there is one way to do each thing.
- **Carried over from the platform proposal:** no client-side animation, no raster images, no code on the client.

## Model

The screen is the v1 picture with numbered drawings laid over it.

- **Base.** Any command outside a numbered drawing executes immediately on the bitmap, as today. A stream with no `D` command takes the existing code path, so compatibility holds by construction.
- **Numbered drawings.** Each holds ordinary ReGIS commands, kept by the renderer and replayed when the screen needs repainting. They appear over the base in depth order.

A v2 application puts its background in a numbered drawing at the lowest depth, so the whole picture is described by its numbered drawings and the shared tables. The base exists for v1 streams.

There is one kind of numbered drawing. The things an application needs are uses of it, not separate types:

| Application need | How it is made |
| --- | --- |
| Reusable symbol (master) | A hidden drawing that others name as their source |
| Placed copy (instance) | A drawing whose source is another drawing |
| Group | A drawing with children and no contents |
| Panel or scrolling area | A drawing with a clip rectangle; scroll by moving a child group |
| Changing label | A drawing whose contents are a `T` command, redefined when the string changes |
| Trend line | A group of drawings, each holding about 32 points as a `V` command. Redefine only the last chunk when a point is added; to scroll, delete the oldest chunk and move the group. One long line would be resent whole for every point. |

## The D command

`D`, for drawing, is the only addition to the language. The letter is free as a command: v1 uses `S W P V C T L @ R F` and `;` ([manual, chapter 1](https://vt100.net/docs/vt3xx-gp/chapter1.html)). In v1 it appears only inside parentheses, as the text direction option in `T` and as the colour letter for dark ([chapter 2](https://vt100.net/docs/vt3xx-gp/chapter2.html)), so there is no clash.

| Operation | Effect |
| --- | --- |
| Define | Stores the ReGIS commands that follow, up to an end marker, as drawing *n*. If *n* exists, its contents are replaced and its attributes and children are kept. |
| Set | Changes one or more attributes of drawing *n*. If *n* does not exist, creates it empty. |
| Delete | Removes drawing *n* and its children. |
| Delete all | Removes every numbered drawing. |
| Commit | Applies all changes since the last commit at once. If any would break a limit, none is applied and the failure is reported to the caller (see Failed commits). |

Drawing numbers run from 1 to 65,535. Deleting a number that does not exist does nothing. Naming a parent that does not exist rejects the commit.

Examples; the syntax is defined in the Grammar section below:

```
D(B3) P[0,0]V[20,0][20,20][0,20][0,0] D(E)    define drawing 3
D(S3(V0))                                     hide it, so it serves as a master
D(S12(R3,X220,Y180))                          drawing 12: source 3, at 220,180
D(S12(X240))                                  move 12
D(B20) T'SCORE 150' D(E)                      replace the contents of 20
D(K12)                                        delete 12
D(C)                                          commit
```

## Attributes

Every numbered drawing has the same nine attributes. The last two, added on 16 November 2026, take effect only through the viewer state (below).

| Attribute | Meaning | Default |
| --- | --- | --- |
| Position | Where its origin sits, in whole pixels, relative to its parent | 0,0 |
| Depth | Drawing order among the children of the same parent; higher draws later, and equal depths draw in order of drawing number | 0 |
| Visible | Whether it and its children are shown | Shown |
| Parent | The drawing it belongs to | None (top level) |
| Source | Another drawing to draw in place of its own contents, which are kept but not drawn | None |
| Detectable | Whether pointer events report it | No |
| Clip rectangle | It and its children appear only inside this rectangle, given relative to its own origin | None |
| Hover look | Another drawing to draw in its place while the viewer's pointer is over it | None |
| Pressed look | Another drawing to draw in its place while a button or a contact is pressed on it | None |

- A child moves, hides and clips with its parent.
- A drawing has exactly one parent. A change of parent applies to the staged scene at once and shows at commit; from then on it moves, hides and clips with the new parent, and is no longer deleted with the old one.
- A drawing with no contents can still have visible children.
- **Source:** a drawing with a source draws that drawing's contents and children, so a master can itself be a group. The source's own position, depth, visibility, parent and detectable flag are not used: the drawing that names it decides where and whether it appears. The source's clip rectangle is used, because it is part of how the source looks.
- **Order within one drawing:** its own contents, or else its source's contents followed by the source's children; then its own children. Children draw in depth order. Clip rectangles along the chain intersect.
- **Coordinates:** a numbered drawing always uses default v1 addressing, one unit per pixel, whatever `S(A)` the base is using. Positions and clip rectangles are in pixels.
- **Screen position:** the sum of the positions up the parent chain, computed in 32 bits so it cannot overflow.
- **Range:** inside a drawing, coordinates may use the full 16-bit range. Only the screen edge and clip rectangles cut drawing off, so a scrolling list can be taller than the screen.
- Position is the only transform. There is no scaling, rotation or mirroring.

## Rules for contents

The screen is defined as the base with the numbered drawings executed over it in depth order, each from its own fresh drawing context. Incremental repaint must give exactly the pixels of that full rebuild. Four rules make this possible.

1. **Starting state.** Each time it is drawn, its contents start in a fresh drawing context (default position, write controls and text controls), with \[0,0\] at its own origin. Nothing it sets carries over to anything else. It uses the shared tables as rule 4 defines and does not clear the pixels beneath it. This is needed because in v1 a changed setting stays in force until changed again ([chapter 1](https://vt100.net/docs/vt3xx-gp/chapter1.html)).
2. **Writing lands on what is beneath.** Overlay, replace, complement, erase and plane select behave as v1 defines them ([chapter 3](https://vt100.net/docs/vt3xx-gp/chapter3.html)), applied to the pixels below the drawing in depth order.
3. **Terminal commands are ignored inside.** `S`, `L`, `R`, and macrograph definition and clearing have no effect inside a numbered drawing. Calling a macrograph is allowed, and these commands are ignored inside the macrograph too. So is any D operation: D is honoured only in the stream as received, never from a macrograph expansion, inside a drawing or outside one. The renderer ignores these commands rather than rejecting the drawing, because a macrograph can be redefined later to contain one.
4. **Shared tables stay global, as in v1.** The palette, the three loadable character sets ([chapter 8](https://vt100.net/docs/vt3xx-gp/chapter8.html)) and the 26 macrographs ([chapter 9](https://www.vt100.net/docs/vt3xx-gp/chapter9.html)) are set outside numbered drawings. Commands outside numbered drawings take effect immediately and are never staged, so a changed loaded set or macrograph applies to base drawing at once. Numbered drawings use the loaded sets and macrographs as they stood at the last successful commit: each commit takes a copy (about 19 KB in the current renderer: 16,384 bytes of macrographs and 2,880 of loaded sets), checks the caps against it with macrographs expanded, and repaints. A table change and the drawings that depend on it therefore appear together. The palette is one colour map for the whole screen, base included. Outside an open batch a palette change is immediate, as in v1. While a batch is open, from the first D operation after a commit, it is held and takes effect at that commit for the whole screen, so new drawings and their colours appear in one frame. If the commit fails, it is discarded with the batch.

Two consequences:

- Fill patterns are anchored to the drawing's origin, so a drawing looks the same wherever it is placed. Pieces that must join without dither seams, such as a background split across several drawings, should share one position and draw different regions. If their positions differ, they must differ by multiples of the tile size.
- Outside numbered drawings, `S(E)` and scroll act on the base only. Numbered drawings stay where they are.

* Numbered drawings lie over whichever v1 graphics page is displayed.

## Commit and repaint

`D` operations are staged; nothing changes on screen until commit, and then everything changes at once. Operations apply in the order received to a staged copy of the scene, which replaces the visible scene at commit. In D(S5(P0),K3,C), where 5 was a child of 3, drawing 5 survives.

The renderer rebuilds the whole picture each time its caller asks for one: the base, then every visible numbered drawing in depth order. The caller asks when a commit was applied or the base changed, and no more often than it shows frames, so commits that arrive faster than frames are merged and only the latest scene is painted. A full rebuild is the reference the equivalence test needs anyway, and it gives a measured baseline. Damage repaint comes second, is not built yet, and would repaint only what changed:

1. Works out the damaged area: the old and new bounds of every changed drawing.
2. Copies the base pixels into that area.
3. Redraws the numbered drawings that intersect it, in depth order, writing only pixels inside the damage.

- **Required property:** the result is identical, pixel for pixel, to rebuilding the whole screen from the base and all numbered drawings.
- **Drawing on the base** takes effect immediately, as in v1. If numbered drawings lie over the affected area, they are repainted on top. The renderer need only bring the screen up to date before it next presents a frame, so a long run of base commands does not cause a repaint for each one.
- **Further optimisation** (tiles, spatial indexes) comes only if measurement calls for it.

* **Validation:** at commit the renderer checks the whole batch against the limits. If any operation fails, none is applied, the screen is unchanged, and the failure is reported to the caller as described under Failed commits.
* **Reused drawings:** when a drawing changes, every drawing that uses it as a source, directly or indirectly, is damaged and repainted too.

- **Presentation:** the renderer builds the committed picture apart from the base, in a picture the caller owns, and gives it with its pick record, its commit count and the work it cost as one frame. A commit is atomic on the visible screen as well as in the scene. The renderer says when a frame is wanted: an event at each commit applied, and a mark on the screen when base drawing changed it. A picture also says which rows differ from the one before.
- **Staged batches** survive leaving and re-entering ReGIS mode. Only a definition left open is discarded.

## Viewer state

The program that shows the picture (the receiver) can change how drawings look according to what the viewer's pointer is doing, with no byte of ReGIS arriving and no message to the application. Added on 16 November 2026.

- **The state** is three drawing numbers the receiver sets, each 0 for none: the *hovered* drawing, the *pressed* drawing and the *hidden* drawing. It is the receiver's alone: nothing in the stream can read or set it, so an application learns where the pointer is only through input.
- **Hover look and pressed look:** while a drawing is the pressed drawing and its pressed look names a drawing that exists, that drawing is drawn in its place; otherwise, while it is the hovered drawing and its hover look names a drawing that exists, that one is. "In its place" means what it means for a source: at this drawing's position, under its clip rectangle, from a fresh context, followed by this drawing's own children. The pressed look wins. A look that names no existing drawing leaves the ordinary look.
- **The hidden drawing** is not drawn, with its children, as if its visible attribute were 0. A receiver blinks a cursor the application drew by hiding it for half of each period.
- **Only the drawing itself:** a drawing drawn as another's source or look does not use its own looks, since it is not the drawing under the pointer.
- **Picking:** the pixels a look writes belong to the drawing it stands for, as a source's do. A look that covers fewer pixels than the ordinary look can therefore put the pointer outside the drawing it was over; give a look at least the ordinary look's pixels for a steady hover.
- **References:** looks are links between drawings like sources: no drawing may reach itself through sources and looks in any mixture, and chains of them count toward the nesting cap of 16.
- **Work:** the cap on one repaint holds whatever the viewer state. A commit counts, for each drawing, the dearest of its ordinary look, its hover look and its pressed look.
- **A look is usually kept hidden** (`V0`), so that it is drawn only where it is named.

Example: `D(S10(I1,H11,A12),S11(V0),S12(V0))` makes drawing 10 detectable, shows drawing 11 in its place while the pointer is over it and drawing 12 while it is pressed.

## Picking

The renderer can say which numbered drawing lies under any screen position. How input reaches the application, and in what messages, is outside the language.

- **What v1 does:** a position is reported on request, or on a button press in multiple input mode, and always as coordinates only ([chapter 10](https://vt100.net/docs/vt3xx-gp/chapter10.html)). v2 leaves `R` on the base as v1 defines it; what is done with its replies is the caller's choice.
- **What a lookup gives:** for a screen position, the number of the drawing picked (0 for none) and the position within that drawing.
- **How the renderer knows:** during repaint it records, for each pixel, its pick target. This record is allocated only once a detectable drawing exists; at 16 bits per pixel it is 768 KB. It holds the drawing number directly, with 0 for none.
- **Reused drawings:** a reused subtree counts as part of the drawing that placed it, so the lookup names that drawing, and only if it or a drawing above it is detectable.

* **Picking rule:** for each drawing that writes a pixel, walk up from it through its parents to the first detectable drawing; that is its pick target. The pixel belongs to the pick target of the last drawing to write it, whether or not the colour changed. Drawings with no detectable drawing at or above them do not affect picking, so a click on them reaches whatever detectable drawing lies beneath. To stop that under a panel, mark the panel detectable. So a detectable group reports hits on its plain children, and a detectable child inside it reports itself.

- **Identity inside a reused drawing:** reuse gives appearance, not identity. Anything that must be clicked separately has to be its own numbered drawing. A panel of ten buttons placed twice should be two groups whose buttons are separate drawings, each reusing a button master. Reusing the whole panel would report only the panel and a position within it.

* **Text and thin lines:** picking follows drawn pixels, so a tap between the letters of an overlay-mode label misses. Give the detectable drawing or group a replace-mode background, so every pixel of its area is written. If that proves insufficient for touch, it is the case for the deferred hit rectangle.

- **Revision:** the renderer counts the commits it has applied; a failed commit is not counted. A caller that stamps input with this count lets the application tell which scene the user saw, and discard input aimed at a number that has since been deleted and reused. It is a 32-bit counter that starts at 0 and wraps to 0, so it is compared for equality only. Counting commits is enough, because numbered drawings change only at a commit.

Gestures, focus and routing are not part of the language.

## Grammar

This section fixes the syntax of `D`. It follows v1 conventions: options in parentheses, decimal numbers, commas between options. It was checked against the renderer's lexer (`crates/regis/src/lexer/scan.rs`), which implements the manual's chapter 1 rules on letters, control characters and resynchronisation.

- **Letters are case-insensitive**, as every v1 command and option letter is: `d(e)` is `D(E)`.
- **Spaces and control characters are ignored** inside a `D` operation, as everywhere outside a quoted string in v1.
- **A semicolon resynchronises** wherever it appears outside a quoted string, including inside parentheses and brackets, as in v1.

### D operations

| Form | Meaning |
| --- | --- |
| `D(B<n>)` | Begin the definition of drawing *n*. The contents follow. |
| `D(E)` | End the definition. |
| `D(S<n>(<attributes>))` | Set attributes of drawing *n*. |
| `D(K<n>)` | Delete drawing *n* and its children. |
| `D(Z)` | Delete all numbered drawings. |
| `D(C)` | Commit. |

- *n* is a decimal number from 1 to 65,535.
- `B` or `S` on an unused number creates that drawing with default attributes.
- `S`, `K`, `Z` and `C` may be combined in one command, separated by commas and carried out left to right: `D(S12(X240),S13(X260),C)`. `B` and `E` always stand alone.
- A malformed `D` operation makes the current batch fail at commit (code 7). Parsing resumes at the next semicolon, the v1 resynchronisation character.

### Definitions

- The contents are everything between `D(B<n>)` and `D(E)`, stored as received. Macrograph calls are stored, not expanded.
- `D(E)` is recognised only where the v1 lexer reads `D` as a command letter. A `D` inside a quoted string, inside parentheses or brackets (such as `T(D90)`), naming a macrograph (`@D`), or in the hexadecimal cell data of an `L` command (where `D` and `E` are digits) is part of the contents. The renderer therefore scans contents with the v1 lexer in its v1 modes, not with a plain text search.
- Any other `D` operation before `D(E)` makes the batch fail. Definitions do not nest.
- A definition still open when ReGIS mode is exited is discarded, and the batch fails.

* A macrograph definition inside the contents (`@:A … @;`) is skipped as a unit, because v1 stores every character of a macrograph body ([chapter 9](https://www.vt100.net/docs/vt3xx-gp/chapter9.html)). A `D(E)` inside it does not end the drawing.
* `D` operations are honoured only in the stream as received, never from a macrograph expansion.
* A malformed `D(B…)` still takes its contents up to `D(E)` and discards them, so they are never drawn on the base.

### Attributes

| Letter | Attribute | Value | Default |
| --- | --- | --- | --- |
| `X`, `Y` | Position | Signed integer | 0 |
| `L` | Depth (layer) | Signed integer | 0 |
| `V` | Visible | 0 or 1 | 1 |
| `P` | Parent | Drawing number, or 0 for none | 0 |
| `R` | Source (reuse) | Drawing number, or 0 for none | 0 |
| `I` | Detectable (input) | 0 or 1 | 0 |
| `H` | Hover look | Drawing number, or 0 for none | 0 |
| `A` | Pressed look (active) | Drawing number, or 0 for none | 0 |
| `C` | Clip rectangle | `C(X<int>,Y<int>,W<int>,H<int>)`, or `C0` for none | None |

- Attribute values are 16-bit signed integers, the size v1 uses for coordinates ([chapter 1](https://vt100.net/docs/vt3xx-gp/chapter1.html)).
- A sign is part of the value and never means a relative move. Bracketed coordinates are not used in `D`, because in v1 a signed number in brackets is relative.
- Attributes not named in an `S` are left unchanged.

### Failed commits

A failed commit cannot happen when the sender works correctly, because a sender can check every batch with the same rules, caps and code before sending it (the validator). A failure therefore means the two have diverged. The renderer discards the batch, leaves the screen as it was, and reports the failure code and *n* to its caller. What the caller does with it is outside the language.

| Code | Meaning | *n* |
| --- | --- | --- |
| 1 | Stored contents cap exceeded | The drawing being defined |
| 2 | Staged operations cap exceeded | 0 |
| 3 | Chain deeper than 16 | A drawing in the chain |
| 4 | Cycle through parent and source links | A drawing in the cycle |
| 5 | Parent does not exist | The drawing naming it |
| 6 | Rendering work cap exceeded | 0 |
| 7 | Malformed `D` operation | The drawing concerned, or 0 |

## Lifetime and memory

There is no garbage collection. Lifetimes are explicit and memory is bounded by fixed caps, the model v1 already uses for macrographs ([chapter 9](https://www.vt100.net/docs/vt3xx-gp/chapter9.html)).

**Lifetime**

- A numbered drawing lives from creation, by define or set, until delete, delete all or terminal reset. Hiding does not free it.
- Redefining or deleting releases the old contents when the commit succeeds, not when the operation is received.
- Deleting a parent deletes its current children. A drawing moved to another parent beforehand survives.

* A terminal reset clears everything: numbered drawings, the staged batch, the committed table copy and the commit count.

**References**

- A source that does not exist draws nothing and is not an error, as calling an empty macrograph is not an error in v1.
- Deleting a drawing that others use as a source is therefore allowed. They go blank, and reappear if that number is defined again. No reference counting is needed.
- No drawing may be reachable from itself by following parent and source links in any combination. For example, drawing 1 with child 2, where 2 uses 1 as its source, is rejected. Chains of parent and source links, counted together, are capped at 16 levels, the figure v1 uses for macrograph nesting. Cycles are checked at every commit, because a later definition can close one.

**Bounds**

- Five caps: number of drawings, total bytes of stored contents, bytes of staged operations awaiting commit, chain depth, and rendering work per repaint. A commit that would exceed a cap is rejected whole and the failure reported to the caller.
- No pixels are kept per drawing. The renderer holds contents as text plus fixed-size bitmaps (base pages, screen, pick record), the staged batch, an off-screen frame and the committed table copy. Its memory is bounded by the contents cap, the staged cap and a constant.
- An application that creates and never deletes reaches the cap and its commits are rejected. It cannot harm the renderer.

Starting values, to be confirmed by measurement. They are derived from the platform proposal's figures: paintings of 30 to 150 KB, and a full repaint of a 53 to 88 KB painting in 27 to 53 ms (roughly 0.5 to 0.6 ms per KB).

| Cap | Value | Reasoning |
| --- | --- | --- |
| Drawings | 65,535 | Fixed by the number range |
| Stored contents | 4 MB | About 25 times the largest measured painting |
| Staged operations | 8 MB | Sending a whole scene at once is one commit holding: the contents plus up to about 3.4 MB of wrappers and attributes (65,535 drawings at roughly 50 bytes each) |
| Chain depth | 16 | Parent and source links counted together: the nesting reached while drawing. Matches v1 macrograph nesting |
| Rendering work | 100,000,000 work units per full rebuild, counting 64 for each byte of contents replayed and 64 for each drawing painted beside the pixels visited | Bytes are a poor measure: a few bytes can fill a large circle or draw size-16 text, so the cap is in the renderer's own work units (pixels visited). Measured on the corpus (14 November 2026, M1 Mac mini): full paintings take 12 to 19 million units at the median and 67 million at most (a 274 KB painting); dashboard and game screens take 2 to 4 million. Rendering runs at about 0.7 million units per millisecond, so the cap covers every measured picture with half again in hand and bounds a full rebuild at about 140 ms on that machine. A drawing's work is metered independent of position and clipping, so it is a fixed number per definition. A reused drawing counts each time it is drawn, and macrograph calls count expanded |

Hidden drawings cost storage but no rendering work.

Each byte of contents replayed costs 64 units, what a byte out of a macrograph costs in v1, and each drawing painted costs 64 units whether or not it has contents. Without the second, empty drawings that reuse one another many times over would cost nothing to check and unbounded time to walk. Inside a drawing, shaded and filled areas are paid for whole, seen or not, which is what makes a drawing's work the same wherever it is placed.

Nothing limits how often a rebuild happens. A commit of a dozen bytes can cost a rebuild, so work is not bounded by the bytes received as it is in v1. The receiver reports the work each rebuild cost, and for now that is all (decided 15 November 2026).

**Checking before sending**

- A sender can check a batch with the validator, which is the renderer's own code and caps, so it rejects exactly what the renderer would. Rendering work is computed by the same code, once per definition, and summed over visible uses; a separate calculation would drift and cause spurious failures.

## Left out

Each of these was considered and excluded, either because an existing mechanism already does the job or because it changes graphical capability.

| Left out | Reason |
| --- | --- |
| Separate instance, group, viewport, text and series types | One kind of drawing covers all of them (see Model) |
| Content offset for scrolling | Moving a child group does the same |
| Colour remap per drawing | Redefining, or pointing at a differently coloured source, gives the same result. Its meaning under complement and plane select would also need defining. Cost of leaving it out: more bytes for recoloured symbols. |
| Character sets and macrographs local to a drawing | Every dithered drawing would carry its own tiles. Global tables, as in v1, are shared. |
| Hit rectangle | Deferred until a touch application shows that picking by drawn pixels is not enough |
| Scaling, rotation, mirroring | Changes graphical capability; undefined for line width, patterns and text cells |
| Masks | Changes graphical capability beyond the clip rectangle |
| Binary encoding | A second encoding of the same thing |
| New primitives, pattern libraries, new coordinate system | v1 already has arcs, interpolated curves ([chapter 6](https://vt100.net/docs/vt3xx-gp/chapter6.html)) and display addressing ([chapter 2](https://vt100.net/docs/vt3xx-gp/chapter2.html)) |
| Client-side animation, bindings, parameterised drawings | Logic on the client |
| Keeping the base picture in the scene | Erase does not reset the colour map or cursor, so rebuilding the base would need the full history of base commands. v2 applications keep their background in a numbered drawing. |

The clip rectangle is the one item kept that v1 cannot do. It adds nothing that can be drawn; it limits where existing drawing appears, and panels and scrolling lists are not possible without it.

## Tests

Four tests gate the work, and none is traded for performance.

1. **Compatibility.** Every existing renderer test produces the same pixels as before.
2. **Repaint equivalence.** For random sequences of `D` operations and base drawing, the screen after incremental repaint equals a full rebuild from the base and all numbered drawings, in pixels and in the pick record. The sequences must mix parent changes, source redefinitions, complement writing, clipping and macrograph changes.
3. **Isolation.** A numbered drawing produces the same pixels on a blank base whatever other drawings were defined or repainted before it.
4. **Failed commit.** For each failure code, a commit that fails leaves the pixels, the pick record and the committed table copy exactly as they were, and reports the matching failure code.

## Open decisions

- [x] Clip rectangle kept, colour remap left out. Confirmed 14 November 2026.
- [x] Cap values: starting figures set in Lifetime and memory, to be confirmed by measurement.
- [x] Transport, input messages and server behaviour are out of scope: this document covers the language and the renderer only. A failed commit changes nothing and is reported to the caller with its code. Decided 15 November 2026.
- [x] Rendering work budget: 100,000,000 work units per full rebuild, from corpus measurements (see Lifetime and memory).
- [x] Grammar checked against the renderer's lexer: letters are case-insensitive, spaces and control characters are ignored, a semicolon resynchronises outside strings, and `D(E)` is found by the v1 lexer, which also keeps it out of `L` hexadecimal data.
- [x] Grammar fixed; no longer a draft.

- [x] The picture is rebuilt when the caller asks for one, not at each commit; the work each rebuild cost is reported, and nothing limits how often one happens. Decided 15 November 2026.

All decisions are made. Every figure marked as a starting value is confirmed or changed by measurement during implementation.

## What building changed

The renderer was built on 15 November 2026 (`crates/regis/src/scene/`, `machine/numbered.rs`). Where it departs from the text above as first written, the text has been changed to say what was built. The departures:

1. **Rebuild on demand.** The proposal rebuilt at every commit. The renderer rebuilds when its caller asks (`Terminal::compose`), so that commits faster than frames are merged.
2. **Two costs added to rendering work**: 64 units a byte of contents replayed and 64 a drawing painted. The proposal counted pixels visited only.
3. **A drawing pays for shaded and filled areas whole.** The proposal said a drawing's work is independent of position and clipping; v1 pays only for the part on the page, so inside a drawing the whole area is paid for.
4. **Nesting is counted as painting nests.** A drawing nests its source and its children; the cap of 16 and the search for rings follow those links. A drawing whose parent names it as a source is not a ring, because painting it ends: the ring is a drawing that reaches itself by being painted.
5. **`D` inside a polygon fill is not recognised**, as no command but `V`, `C`, `P` and `W` is; nor is `D(E)` inside any parenthesis when a definition is being received.
6. **A background colour set by `S(I)` does not reach a drawing**, since `S` has no effect inside one and each starts from a fresh context: replace and erase writing inside a drawing write colour 0.
7. **Not built:** damage repaint; every rebuild is a full one. The equivalence test therefore has nothing to compare yet.
8. **The staged cap counts** the bytes of `D` commands and of definitions received since the last commit.
9. **A validator keeps no pixels**: what the stream draws outside numbered drawings is read and not drawn.

10. **Viewer state, added 16 November 2026** (DEC-6366, REQ-6367): the hover look and pressed look attributes, `H` and `A`, and the hovered, pressed and hidden drawings a receiver sets (`Terminal::set_viewer_state`). The proposal had no receiver-drawn response to the pointer.

The measured cost to v1 of the drawing window every pixel now passes through: 1,600 renders of the seven example pictures took 6.24 s before and 6.70 s after, 7 % more, with the same pixels.

## Sources

DEC, VT330/VT340 Programmer Reference Manual, Volume 2: Graphics Programming ([contents](https://vt100.net/docs/vt3xx-gp/contents.html)). Chapters 1 to 3, 5 to 12 and 15 were read through page summaries, not the raw text. The lexical rules the grammar relies on were checked against the renderer's lexer, which was written from the manual and is tested against screen captures from a real VT340.

- [Chapter 1, Introduction to ReGIS](https://vt100.net/docs/vt3xx-gp/chapter1.html): command letters, persistent state, defaults
- [Chapter 2, Screen Control](https://vt100.net/docs/vt3xx-gp/chapter2.html): erase, scroll, display addressing, pages
- [Chapter 3, Write Control](https://vt100.net/docs/vt3xx-gp/chapter3.html): writing modes, plane select, patterns
- [Chapter 6, Curve](https://vt100.net/docs/vt3xx-gp/chapter6.html): arcs and interpolated curves
- [Chapter 8, Load](https://vt100.net/docs/vt3xx-gp/chapter8.html): three loadable sets of up to 96 characters
- [Chapter 9, Macrographs](https://www.vt100.net/docs/vt3xx-gp/chapter9.html): 26 macrographs, nesting to 16 levels, empty call is no error
- [Chapter 10, Report](https://vt100.net/docs/vt3xx-gp/chapter10.html): input modes and position reports
