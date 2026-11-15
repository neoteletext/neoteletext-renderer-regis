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

## Resolution (16 November 2026)

Repaint work is bounded by time, in the receiver (DEC-6366 item 2, the engineer's design on the owner's order of that day to build a limit on how often a picture is repainted; TODO-6333's option 2).

The library supplies the meter, `RepaintBudget` (REQ-6368): it is told how much time has passed, earns `REPAINT_WORK_PER_SECOND` (200,000,000 units) a second, holds at most `REPAINT_WORK_HELD` (200,000,000), and says whether a repaint may start and how long to wait. A repaint that starts is paid for in full and what it overdraws is earned back first. Over any `t` seconds one picture's repaints cost at most `REPAINT_WORK_HELD` + `t` × `REPAINT_WORK_PER_SECOND` + `MAX_REBUILD_WORK`.

The receiver keeps it: NeoTeletext's decoder has one budget for each sub-page's picture and composes only while the budget allows; a repaint held back is made once, of the latest scene (the browser's REQ-6380, commit `f2ffaa3`). Damage repaint (REQ-6329) also means a small change now costs its area and not the whole picture.

A rule tied to the bytes received was not chosen, because ReGIS v2 exists to cause repaints with small messages, and since the viewer state a repaint can be caused by no bytes at all.

**Tests:** `crates/regis/tests/budget.rs`, `minimal_commits_cannot_buy_more_repaint_work_than_the_budget_allows` (with the meter's check removed, 4 repaints cost 311,230,800 against a bound of 300,000,000); the browser's `decoder/src/regis.rs`, `repaints_are_held_to_the_budget_and_the_last_scene_is_shown` (with the check removed, 200 repaints cost 13,670,052,960 against a bound of 340,000,000).

**Still to do:** the two constants are starting values, to be confirmed by measuring real pages; the decision awaits the owner's confirmation.
