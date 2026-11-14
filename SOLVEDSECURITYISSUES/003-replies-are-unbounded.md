# 003 — Report replies were bounded in number but not in size

**Found:** 14 November 2026, reviewing `machine/report.rs` and `machine/state.rs`.
**Class:** denial of service (memory), traffic amplification.

One feeding call may queue 1,024 events. With macrograph reports allowed,
`R(M(A))` (seven bytes) queues a reply holding a macrograph of up to 16 KB.
A 7 KB stream could so queue 16 MB of replies in one call, and a host that
forwards replies would send 2,000 times what it received.

**Where:** `Machine::reply`, `Machine::emit`.

**Fix wanted:** a cap on the reply bytes queued by one feeding call; replies
past it are dropped and `LimitReached` is reported.

**Test:** `tests/security.rs`, `reply_bytes_in_one_call_are_capped`.

## Resolution (14 November 2026)

Events now go through `machine/outbox.rs`. One feeding call queues at most `limits::MAX_REPLY_BYTES` (4,096) bytes of replies; a reply that would pass the cap is dropped and `Event::LimitReached` ends the call's events. Macrograph reports stay off unless the caller allows them. The regression test failed on the old code (16 MB of replies) and passes now.

**Later (issue 009).** The per-call cap described above could be defeated by dividing the input into small calls; it is now a budget that refills with the bytes fed.
