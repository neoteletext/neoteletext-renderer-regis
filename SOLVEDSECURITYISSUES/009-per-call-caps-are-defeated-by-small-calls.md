# 009 — The reply and delay caps were per call, and the sender chooses the calls

**Found:** 14 November 2026, by an independent review of `machine/outbox.rs`.
**Class:** amplification; stalling. Reopens issues 003 and 006.

The caps on reply bytes and delay ticks were counted for one feeding call
and reset at its end. A network sender controls how its bytes are divided:
fed two bytes a call, 20,000 bytes of input produced 1,280,000 bytes of
replies, and 8,000 bytes produced a thousand minutes of delay.

**Fix wanted:** budgets that refill with the bytes received, as the work
allowance does, not with the number of calls.

**Test:** `tests/security.rs`, `reply_and_delay_budgets_hold_however_the_input_is_divided`.

## Resolution (14 November 2026)

The reply and delay limits in `machine/outbox.rs` are now budgets: up to `MAX_REPLY_BYTES` (4,096) and `MAX_SAVED_WAIT_TICKS` (3,600) saved up, each refilled by one for each byte fed. Replies can never outweigh the input by more than 4,096 bytes, and delay by more than a minute plus one tick a byte, however the input is divided. The regression test failed on the old code (1,280,000 reply bytes from 20,000) and passes now. This supersedes the per-call caps of issues 003 and 006.
