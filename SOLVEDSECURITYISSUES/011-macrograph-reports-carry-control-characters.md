# 011 — A macrograph report, when allowed, carried control characters, and the tool printed replies raw

**Found:** 14 November 2026, by an independent review of `machine/report.rs` and `regis-render`.
**Class:** injection (answerback; terminal escape). Applies only when the caller allows macrograph reports.

`R(M(x))` copied the macrograph's bytes into the reply unchanged, so a
reply could hold carriage returns followed by chosen text, or escape
sequences. `regis-render` printed replies to the user's terminal unescaped.
The reply was also built (up to 16 KB copied) before the size cap refused it.

**Fix wanted:** the contents reported keep printable characters only; the
size is checked before anything is copied; the tool escapes what it prints.

**Test:** `tests/security.rs`, `a_macrograph_report_holds_printable_characters_only`.

## Resolution (14 November 2026)

`report_macrograph` first asks the reply budget whether the report fits and copies nothing if it does not; the contents sent keep only the characters 0x20 to 0x7E. `regis-render` prints replies with non-printable bytes escaped. The regression test failed on the old code and passes now.
