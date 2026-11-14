# ReGIS writing modes and the server proposal

ReGIS draws into a bitmap and keeps no record of what drew what, so nothing can be deleted once
drawn. Its **writing modes**, set with the write command `W`, are the language's own tools for
working within that limit. This note describes them, shows where each is used in the server
proposal (`server-proposal.html`), and records one combination the proposal should adopt.

## The modes

| Mode | Command | What it does | Use for removing things |
|---|---|---|---|
| Overlay | `W(V)` | Draws the foreground colour where the line pattern is on; leaves other pixels alone. The default. | None by itself |
| Replace | `W(R)` | Draws the foreground where the pattern is on and the background colour where it is off, so the whole shape area is overwritten | Text in replace mode clears its own character cell, so new text erases the old as it goes |
| Complement | `W(C)` | Inverts the colour bits of every pixel it covers | Draw the same thing again and it disappears exactly, restoring whatever was underneath |
| Erase | `W(E)` | Draws in the background colour, ignoring the pattern | Redraw the same shape in erase mode to rub it out, but only cleanly on a plain background |

Two related controls:

- **Negative pattern, `W(N1)`**, swaps which bits of the line pattern draw, so a dashed line can be
  filled in or knocked out.
- **Plane select, `W(F…)`**, limits drawing to chosen bit planes. With erase mode it clears one
  plane and leaves the others untouched.

DEC's VT330/VT340 graphics programming manual describes these in its chapter on write controls,
and presents complement mode as the way to draw and then remove cursors and temporary graphics
without damaging the picture underneath. The renderer implements all four modes and both controls.

### Complement: the true undo

```
W(C) P[100,100] V[300,200]     draw the line, inverting what it crosses
W(C) P[100,100] V[300,200]     the same again: the line is gone, the picture restored
```

It works over any picture, including detailed art, but only if nothing else was drawn over that
spot in between. The temporary shape's colours depend on what lies beneath, because it inverts
rather than paints. It suits cursors, hover rings, rubber-band lines and selections.

### Erase: simpler but blunter

```
W(I3) P[100,100] V[300,200]    draw a line in colour 3
W(E)  P[100,100] V[300,200]    draw it again in erase mode: it becomes background colour
```

Over a plain background this removes the line completely. Over anything else it leaves a
background-coloured scar, because erase mode does not know what was there before.

## Where each is used in the proposal

| Mode | Where in the proposal | Value |
|---|---|---|
| Complement `W(C)` | Transient elements (section 13.3), the removal toolkit (15), immediate click feedback (21.7), hovers, rubber bands and beams in the strategy game (24) | **High.** The cheapest exact undo, about 20 bytes each way, over any picture |
| Plane select `W(F…)` with erase | Overlay layers (15.1) and sprites in action games (25) | **Highest.** Genuinely transparent layers that can be cleared without touching the picture beneath |
| Replace `W(R)` | The compact paint encoder (17): one fill lays down both colours of a dither. The cell layer (16): text in replace mode rewrites its whole cell | **High.** What makes shaded pictures small and cell grids cheap |
| Erase `W(E)` | One row of the removal table (15) | **Low to medium.** Clean only on plain backgrounds; replay and the overlay plane cover the rest |
| Overlay `W(V)` | The default for everything else | Neutral |
| Negative pattern `W(N1)` | Not used | Little, beyond line styling |

## A combination to adopt: complement within one plane

The weakness of complement mode is that its colours are unpredictable: it inverts all four bits of
whatever is underneath, so a highlight looks different over every part of the picture.

Combining complement mode with the plane mask fixes this. Restrict complement drawing to the
overlay plane alone, and it flips only that one bit. Because the palette is arranged so that
"plane bit set" means "overlay colour":

- the transient always appears in the overlay colour, whatever lies beneath, instead of a scramble
  of inverted colours;
- drawing it again still removes it exactly, because flipping one bit twice restores it;
- it needs no erase command and no memory of the box it occupied, so a cursor or hover ring costs
  the same 20 bytes each way and now looks deliberate.

This also makes transients and overlay-plane sprites one mechanism, which simplifies the server.
Section 15 of the proposal should present it as the default way to draw anything temporary over a
rich picture.

## To check on the renderer before relying on them

- **The replace-mode cell rewrite.** Text written in replace mode must fully rewrite its cell at
  every text size the cell layer uses.
- **Complement restricted by the plane mask.** The renderer's ink code tests complement mode and
  plane-mask protection separately; the combination needs its own test before the design depends
  on it.
