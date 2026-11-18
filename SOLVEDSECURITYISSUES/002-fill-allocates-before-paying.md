# 002 — A polygon fill allocated its mask before checking the work allowance

**Found:** 14 November 2026, reviewing `raster/fill.rs`.
**Class:** denial of service (memory and CPU amplification).

`fill_polygon` built a mask the size of the polygon's bounding box (up to
384,000 entries, allocated and zeroed) and only then asked the work
allowance whether the fill could be afforded. A 25-byte fill command
covering the screen therefore cost an allocation and zeroing of 384 KB even
after the allowance was spent.

**Where:** `Mask::covering`, called before `Painter::spend`.

**Fix wanted:** work out the mask's size, pay for it, and only then allocate.

**Test:** `tests/security.rs`, `fills_after_the_allowance_is_spent_do_no_work`
(times a stream of screen-sized fills).

## Resolution (14 November 2026)

`Mask::covering` now only measures the polygon; `fill_polygon` pays for the area and the edges first and allocates the mask after. The regression test compares screen-sized fills with tiny ones. It passed on the old code as well: on this machine a zeroed allocation is served lazily by the operating system, so the waste could not be measured. The fix is by inspection of the order of the calls; the test stays as a guard against the cost growing.

## The test's budget (18 November 2026)

The test held the stream to 15 seconds of wall-clock time in whatever build ran it. A debug build takes 14.3 to 15 seconds for it on a quiet machine, at the commit before animation (534ade4: 14.42 s of processor time at best in eight runs) and after it (0920a09: 14.25 s), nearly all of it the 1,296 million work units the stream's bytes pay for; an optimised build takes 0.48 s at both. So the bound failed under load and said nothing of the stream. It is now two tests:
- `fills_after_the_allowance_is_spent_do_no_work` holds the claim in work units, in every build: the stream spends no more than it earned (`WORK_CAP` and `WORK_PER_BYTE` for each byte), and less than one fill's cost is left unspent.
- `the_stream_of_fills_is_read_within_its_time` holds the time, 5 seconds, in an optimised build; in a debug build it is ignored and says why. The full gate runs the release tests.
