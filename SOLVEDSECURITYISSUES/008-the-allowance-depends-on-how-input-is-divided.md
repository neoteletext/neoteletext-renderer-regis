# 008 — The work allowance was credited per call, so the picture depended on chunking

**Found:** 14 November 2026, by an independent review of `terminal.rs` and `work.rs`.
**Class:** correctness under attack; inconsistent limits.

`feed` credited the allowance for the whole slice before reading it, and the
allowance is capped at 16,000,000 units. A large stream fed in one call
therefore had 16,000,000 units in all, while the same stream fed in small
pieces earned as it went and drew everything. The same bytes gave different
pictures, and `regis-render` (one call for the whole file) could draw less
than a network receiver.

**Fix wanted:** the allowance is credited byte by byte as input is read, so
the result does not depend on how the input is divided.

**Test:** `tests/security.rs`, `the_picture_does_not_depend_on_chunking_even_at_the_work_limit`.

## Resolution (14 November 2026)

`Terminal::feed` and `feed_regis` now credit the allowance one byte at a time as they read (`Machine::earn`), so a stream earns the same however it is divided. The regression test (two hundred screen-sized fills fed whole, in pieces of 1,000 and of 4,096) failed on the old code and passes now.
