# 012 — Running out of allowance inside a macrograph left the parser in the middle of a token

**Found:** 14 November 2026, by an independent review of `terminal/source.rs`.
**Class:** state confusion.

When the allowance ran out while a macrograph was being executed, the rest
of its body was dropped where it stood: possibly inside a quoted string, a
bracket or a parenthesis. The bytes that followed in the stream were then
read in that wrong context (as text, for instance) until a quote or `;`.
Two smaller leaks of the same kind: a tilt flag survived `;`, and an output
map location survived to a later `S(M(…))` that named none.

**Fix wanted:** dropping a macrograph's body also resynchronises the
parser; per-option scratch state is cleared when its option ends or on `;`.

**Test:** `tests/security.rs`, `an_abandoned_macrograph_does_not_swallow_what_follows`,
and `tests/screen.rs`, `an_output_map_location_does_not_outlive_its_option`.

## Resolution (14 November 2026)

When the allowance runs out during a macrograph the terminal now restarts the parser (`Terminal::restart`: lexer reset, macrograph state reset, resynchronise), so the bytes that follow are read as commands. A resynchronisation now also clears the tilt flag, a half-given addressing corner, the shading option and the output map location, and each `M` option names its location afresh. Both regression tests failed on the old code and pass now.
