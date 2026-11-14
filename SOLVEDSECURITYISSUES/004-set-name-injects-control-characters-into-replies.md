# 004 — A character-set name could carry control characters into a reply

**Found:** 14 November 2026, reviewing `machine/load.rs` and `machine/report.rs`.
**Class:** injection (terminal answerback).

`L(A1'name')` stores a name and `R(L)` sends it back to the host as
`A'name'` and a carriage return. Quoted strings keep carriage return, line
feed, backspace and tab, so a name could contain a carriage return followed
by text of the attacker's choosing. A host that treats replies as typed
input (as a real terminal's host does) would then read attacker-chosen
lines: the classic answerback injection.

**Where:** `Loadable::set_name`.

**Fix wanted:** a name keeps only printable characters, and never a quote
mark, so the reply is always one line of the fixed form `A'…'`.

**Test:** `tests/security.rs`, `a_set_name_cannot_put_control_characters_in_a_reply`.

## Resolution (14 November 2026)

`Loadable::set_name` keeps only the printable characters 0x20 to 0x7E and drops both quote marks, so `R(L)` always answers one line of the form `A'name'`. The regression test failed on the old code (the reply carried the carriage return and line feed) and passes now. The integration plans also require that replies go only to the program that sent the stream.
