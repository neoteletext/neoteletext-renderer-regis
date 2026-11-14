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
