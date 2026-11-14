# 006 — A stream can ask for hours of delay

**Found:** 14 November 2026, reviewing `machine/screen.rs`.
**Class:** denial of service (stalling the viewer).

`S(T n)` asks for a pause of up to 32,767 ticks, about nine minutes, and a
stream may hold many. The library only reports the request, but a caller
that sleeps for every `Event::Wait` in full can be held for hours by a few
hundred bytes.

**Where:** `Machine::screen_number`, option `T`.

**Fix wanted:** the library bounds the total delay one feeding call can ask
for, and the documentation tells callers to treat a wait as advice.

**Test:** `tests/security.rs`, `the_delay_one_call_can_ask_for_is_capped`.

## Resolution (14 November 2026)

One feeding call can now ask for at most `limits::MAX_WAIT_TICKS_PER_CALL` (3,600 ticks, one minute) of delay in all; the excess is dropped and `Event::LimitReached` is reported. The guide tells callers that a wait is advice and that they own the clock. The regression test failed on the old code (273 minutes requested) and passes now.

**Later (issue 009).** The per-call cap described above could be defeated by dividing the input into small calls; it is now a budget (`limits::MAX_SAVED_WAIT_TICKS`) that refills with the bytes fed.
