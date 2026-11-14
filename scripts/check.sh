#!/bin/sh
# The local gate: every check this repository has, in three sizes.
#
#   scripts/check.sh commit   formatting, the no-panic lists, Clippy
#   scripts/check.sh push     commit, then every test
#   scripts/check.sh full     push, then documentation, release tests, the
#                             long random-input run in a debug build, and
#                             the font files checked against their sources
#
# There is no CI: the git hooks in .githooks run this (pre-commit: commit;
# pre-push: push, or full for main). Never bypass them.
set -eu
cd "$(dirname "$0")/.."

# Crate roots that read outside input. Each must carry the same list of
# denied lints, so no crate quietly allows a panic.
NO_PANIC_ROOTS="crates/regis/src/lib.rs crates/regis-render/src/main.rs crates/regis-minify/src/lib.rs crates/regis-minify/src/main.rs"

step() { printf '\n== %s\n' "$1"; }

no_panic_list() { sed -n '/^#!\[deny($/,/^)\]$/p' "$1"; }

check_no_panic_lists() {
    reference=$(no_panic_list crates/regis/src/lib.rs)
    [ -n "$reference" ] || { echo "no deny list found in crates/regis/src/lib.rs"; exit 1; }
    for root in $NO_PANIC_ROOTS; do
        [ "$(no_panic_list "$root")" = "$reference" ] || { echo "$root: the no-panic deny list differs"; exit 1; }
    done
}

check_no_unsolved_security_issues() {
    for issue in UNSOLVEDSECURITYISSUES/*.md; do
        [ "$issue" = "UNSOLVEDSECURITYISSUES/README.md" ] || [ ! -e "$issue" ] || {
            echo "unsolved security issue: $issue"; exit 1;
        }
    done
}

gate_commit() {
    step "lockfile is current"; cargo metadata --locked --format-version 1 > /dev/null
    step "formatting"; cargo fmt --check
    step "no-panic lists"; check_no_panic_lists
    step "Clippy"; cargo clippy --workspace --all-targets --locked -- -D warnings
}

gate_push() {
    gate_commit
    step "tests"; cargo test --workspace --locked
}

gate_full() {
    gate_push
    step "documentation"; RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps --locked
    step "release tests"; cargo test --workspace --release --locked
    step "random input, debug build (overflow panics)"; cargo test -p regis --test robustness --locked -- --ignored
    step "font files match their sources"
    python3 -I tools/bdf_to_regisfont.py fonts/vwsvt0_19.bdf | cmp - fonts/vwsvt0.regisfont
    # The VT340 font comes from the terminal's character generator ROM,
    # which is not kept here; it is checked when VT340_ROM names the file.
    if [ -n "${VT340_ROM:-}" ]; then
        python3 -I tools/rom_to_regisfont.py "$VT340_ROM" | cmp - fonts/vt340.regisfont
    else
        echo "VT340_ROM not set: fonts/vt340.regisfont not checked against the ROM"
    fi
    step "no unsolved security issues"; check_no_unsolved_security_issues
}

case "${1:-}" in
    commit) gate_commit ;;
    push) gate_push ;;
    full) gate_full ;;
    *) echo "usage: scripts/check.sh commit|push|full"; exit 2 ;;
esac
printf '\ngate %s passed\n' "$1"
