#!/usr/bin/env bash
# The repository's one check script: its own steps, on the gate machinery
# every neoteletext repository shares (scripts/gate.sh, copied from
# neoteletext-common; DEVELOPMENT.md section 9 there). Every step passes or
# fails the run: nothing is left out, and a missing tool or reference file
# is a failure.
#   scripts/check.sh commit   lockfile, formatting, the no-panic lists, Clippy
#   scripts/check.sh push     the tests: a push of livedevelopment
#   scripts/check.sh full     documentation, release tests, the long
#                             random-input run in a debug build, the long
#                             comparison of partial repaints, the animation
#                             fuzz target by property testing, the fonts
#                             against their sources, no unsolved security
#                             issue: a push to main
# This repository is developed on chunko, a Linux machine, so the full gate
# has no separate Linux stage.
set -euo pipefail
cd "$(dirname "$0")/.."

# Unused: the full gate has no linux_steps. gate.sh reads it only there.
linux_host="${TELETEXT_LINUX_HOST:-chunko}"
# What the gates read.
readonly CHECKED_PATHS=(crates scripts tools fonts UNSOLVEDSECURITYISSUES .githooks Cargo.toml Cargo.lock rust-toolchain.toml clippy.toml rustfmt.toml)

. scripts/gate.sh

# The VT340's character generator ROM, which fonts/vt340.regisfont is made
# from. Like every reference file it stays out of the repository, in
# ~/devel/teletext-reference (DEVELOPMENT.md section 1); the tool checks its
# SHA-256 before reading it.
readonly VT340_ROM="${VT340_ROM:-$HOME/devel/teletext-reference/ReGIS/vt340-firmware/vt340e49.bin}"

# The crates that read input from outside their process, each with the
# same no-panic lint list at its root (DEVELOPMENT.md section 6). Every
# crate root must be listed.
readonly NO_PANIC_ROOTS=(crates/regis/src/lib.rs crates/regis-fonts/src/lib.rs crates/regis-fuzz/src/lib.rs crates/regis-render/src/main.rs crates/regis-minify/src/lib.rs crates/regis-minify/src/main.rs)

check_no_panic_lists() {
    step "the no-panic lint list is the same in every crate root"
    local first="" root list
    for root in crates/*/src/lib.rs crates/*/src/main.rs crates/*/src/bin/*.rs; do
        [[ -f "$root" ]] || continue
        [[ " ${NO_PANIC_ROOTS[*]} " == *" $root "* ]] || fail "$root is a crate root missing from NO_PANIC_ROOTS"
    done
    for root in "${NO_PANIC_ROOTS[@]}"; do
        [[ -f "$root" ]] || fail "$root is listed in NO_PANIC_ROOTS but does not exist"
        list="$(sed -n '/^#!\[deny(/,/^)\]/p' "$root")"
        [[ -n "$list" ]] || fail "$root has no #![deny(...)] list"
        if [[ -z "$first" ]]; then
            first="$list"
        elif [[ "$list" != "$first" ]]; then
            fail "$root's #![deny(...)] list differs from ${NO_PANIC_ROOTS[0]}'s"
        fi
    done
}

check_no_unsolved_security_issues() {
    step "no unsolved security issue is open"
    local issue
    for issue in UNSOLVEDSECURITYISSUES/*.md; do
        [[ -e "$issue" && "$issue" != UNSOLVEDSECURITYISSUES/README.md ]] || continue
        fail "unsolved security issue: $issue"
    done
}

commit_steps() {
    step "lockfile up to date"
    cargo metadata --quiet --locked --format-version 1 >/dev/null
    step "formatting"
    cargo fmt --check
    check_no_panic_lists
    step "Clippy, warnings as errors"
    cargo clippy --quiet --workspace --all-targets --locked -- -D warnings
}

push_steps() {
    step "tests"
    cargo test --quiet --workspace --locked
}

full_steps() {
    step "documentation builds, warnings as errors"
    RUSTDOCFLAGS="-D warnings" cargo doc --quiet --workspace --no-deps --locked
    step "release tests"
    cargo test --quiet --workspace --release --locked
    step "random input in a debug build, where arithmetic overflow panics"
    cargo test --quiet -p regis --test robustness --locked -- --ignored
    step "partial repaints against whole ones, at length"
    cargo test --quiet -p regis --test damage --release --locked -- --ignored
    step "the fuzz target regis_animation on inputs made by property testing, at length"
    cargo test --quiet -p regis-fuzz --test regis_animation --release --locked -- --ignored
    step "the fonts are what their sources make"
    need_tool python3 "the font tools are Python (sudo apt-get install python3)"
    [[ -f "$VT340_ROM" ]] || fail "the VT340 ROM is not at $VT340_ROM; put vt340e49.bin there (SHA-256 eb0426b4...) or name it in VT340_ROM"
    python3 -I tools/bdf_to_regisfont.py fonts/vwsvt0_19.bdf | cmp - fonts/vwsvt0.regisfont ||
        fail "fonts/vwsvt0.regisfont is not what tools/bdf_to_regisfont.py makes from fonts/vwsvt0_19.bdf"
    python3 -I tools/rom_to_regisfont.py "$VT340_ROM" | cmp - fonts/vt340.regisfont ||
        fail "fonts/vt340.regisfont is not what tools/rom_to_regisfont.py makes from the VT340 ROM"
    check_no_unsolved_security_issues
}

gate_main "$@"
