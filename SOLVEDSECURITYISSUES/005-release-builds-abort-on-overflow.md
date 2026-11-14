# 005 — The release profile turned an arithmetic slip into a process abort

**Found:** 14 November 2026, reviewing `Cargo.toml`.
**Class:** denial of service (availability).

The workspace's release profile set `overflow-checks = true` together with
`panic = "abort"`. The geometry modules allow unchecked arithmetic on the
strength of written bounds. If one of those bounds were wrong, hostile
input would abort the whole process that embeds the renderer, rather than
draw one wrong pixel. Safe Rust cannot corrupt memory on wrap-around, so
aborting buys nothing in a release build.

**Fix wanted:** overflow checks stay on in debug builds and tests (where
the random-input tests run, so a wrong bound is found) and are off in
release builds.

**Test:** the gate runs the random-input tests in a debug build; see
`scripts/check.sh`.

## Resolution (14 November 2026)

`overflow-checks` was removed from the release profile; debug builds and tests keep the default checks, and `scripts/check.sh full` runs the random-input tests in a debug build so that a wrong bound panics there.
