# 001 — Screen erase costs 384,000 bytes of work for four bytes of input

**Found:** 14 November 2026, reviewing `machine/screen.rs`.
**Class:** denial of service (CPU amplification).

`S(E)` clears a whole page: 384,000 bytes written. The erase did not draw on
the work allowance, so a stream of nothing but `S(E)` made the renderer
write about 96,000 bytes for every byte received. Four megabytes of input
would cost roughly 380 GB of memory writes.

**Where:** `Machine::erase`.

**Fix wanted:** erasing pays for the page's area like any other drawing; an
erase that cannot be afforded is skipped and reported as `LimitReached`.

**Test:** `tests/security.rs`, `a_stream_of_erases_is_bounded_by_the_work_allowance`.

## Resolution (14 November 2026)

`Machine::erase` and `Machine::scroll` now spend the page's area (384,000 units) from the work allowance before touching the page; an erase that cannot be afforded is skipped and the caller receives `Event::LimitReached`. The regression test failed on the old code (the megabyte of erases took far over its time limit and reported no limit) and passes now.
