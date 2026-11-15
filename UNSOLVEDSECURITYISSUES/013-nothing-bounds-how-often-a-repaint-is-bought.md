# 013 — Nothing bounds how often a repaint of the numbered drawings is bought

**Found:** 15 November 2026, while writing the ReGIS v2 requirements (TODO-6333).
**Class:** resource exhaustion (processor time).

In ReGIS v1 every pixel drawn is paid for from an allowance that grows
with the bytes fed (`WORK_PER_BYTE`, 256 units a byte), so the work a
stream can cost is in proportion to its length (DEC-6267).

ReGIS v2 breaks that proportion. A commit that only moves one drawing,
`D(S1(X1),C)`, is about a dozen bytes and earns about 3,000 units. The
picture it changes is repainted whole when the caller next asks for one
(`Terminal::compose`), and a repaint may cost up to `MAX_REBUILD_WORK`,
100,000,000 units, about 140 milliseconds on the machine it was measured
on. The cap bounds one repaint; nothing bounds how many are bought. A
stream of small commits can therefore keep a receiver's decoder repainting
for as long as it is sent, for a few bytes a repaint.

What limits it today, outside the library:
- the caller repaints when it wants a frame, not at each commit, so commits
  that arrive faster than frames are shown are merged into one repaint;
- NeoTeletext's server checks a program's commits with the same code before
  sending them, so a repaint over the cap never reaches a receiver from it;
- the browser's decoder is a confined process, and one channel's alone.

None of these bounds the work by the bytes received.

**The owner's decision so far (15 November 2026, DEC-6335 item 4):** the
work each repaint cost is reported to the caller and shown on the browser's
display, "nothing more for now". No rule was chosen. The options laid out
in TODO-6333: charge repaints to the allowance earned per byte (rejected
by the engineer: a dashboard costs 2 to 4 million units to repaint, so a
12-byte move would show nothing until about 12,000 more bytes arrived); a
limit by time in the caller; a cap on commits; or accepting the cost.

**Fix wanted:** a rule, chosen by the owner, that bounds repaint work over
time or by input, recorded as a decision, stated in REQ-6326, and tested.

**Test wanted:** a stream of minimal commits cannot buy more repaint work
than the rule allows, as `a_short_stream_cannot_buy_unbounded_drawing` in
`crates/regis/tests/robustness.rs` shows for v1.
