# 007 — Circles and arcs cost 360 rotations each and paid nothing

**Found:** 14 November 2026, by an independent review of `machine/curve.rs` and `raster/arc.rs`.
**Class:** denial of service (CPU amplification).

Every circle or arc turns its starting point through up to 360 one-degree
steps. With a radius of nothing (`W(M0)C0`) all 360 points fall on one
pixel, so nothing was drawn and nothing was paid, but the 360 rotations
were still computed. From a macrograph, where a byte costs 64 work units
against 1,024 earned, one megabyte of input bought 46 seconds of CPU.

**Fix wanted:** an arc pays for its steps before computing them.

**Test:** `tests/security.rs`, `arcs_of_no_radius_are_paid_for`.

## Resolution (14 November 2026)

`Machine::arc` now spends `limits::ARC_STEP_COST` (4 units) for each degree before computing any point; an arc that cannot be afforded is not computed. The regression test failed on the old code (43 seconds for a megabyte) and passes now. The work earned for each input byte was also lowered from 1,024 to 256 units, which no sample picture comes near (`tests/streaming.rs` asserts it).
