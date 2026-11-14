# 010 — The bytes 0x90 and 0x9C were obeyed inside UTF-8 text

**Found:** 14 November 2026, by an independent review of `terminal/stream.rs`.
**Class:** injection; state confusion.

The stream reader always treated the byte 0x90 as the 8-bit device control
string introducer and 0x9C as the string terminator. Both bytes occur
inside ordinary UTF-8 text: the Cyrillic letter А is `D0 90`. Text
containing `Аp` therefore entered ReGIS with no escape character, and
anyone able to place text in a stream could draw on or erase the screen.
The reverse also held: a string holding М (`D0 9C`) ended at that byte.

**Fix wanted:** the 8-bit controls are recognised only if the caller asks
for them; by default only `ESC P` and `ESC \` are.

**Test:** `tests/security.rs`, `utf8_text_cannot_enter_or_leave_regis`.

## Resolution (14 November 2026)

The stream reader obeys 0x90 and 0x9C only after `Terminal::recognise_eight_bit_controls(true)`; by default they are ordinary bytes, so UTF-8 text can neither start nor end ReGIS. The regression test failed on the old code and passes now.
