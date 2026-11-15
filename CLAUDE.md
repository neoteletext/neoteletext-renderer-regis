# neoteletext-renderer-regis

DEC ReGIS (the VT340's Remote Graphics Instruction Set) for NeoTeletext. The repository holds three crates:
- `regis`, the renderer: a `no_std` library that takes bytes and writes colour-map addresses into the caller's screen;
- `regis-render`, which draws a file to an image;
- `regis-minify`, which rewrites ReGIS shorter and checks the result by rendering it.

It also holds the fonts and the tools that make them, the guide and reference, and the ReGIS v2 proposal (`docs/regis-v2.md`). ReGIS v2 is meant to be an alternative renderer and standard beside teletext in the NeoTeletext browser and server. They pin this repository by revision, as they pin the engine.

## How we work

@~/devel/neoteletext-common/DEVELOPMENT.md

That is the one document of how we work, for every NeoTeletext repository, with rdm as the record of all work (DEC-2162). This file says only what is particular to this repository; where the two differ, that document holds.

## Rules particular to this repository

- **Developed on chunko** (Ubuntu). The Mac keeps a checkout for macOS builds.
- **Reference files stay out of the repository.** The manuals and the VT340 character generator ROM are read from `~/devel/teletext-reference/ReGIS/`. The ROM is `vt340-firmware/vt340e49.bin`, or wherever `VT340_ROM` names it. `tools/rom_to_regisfont.py` checks its SHA-256 before reading it, and the full gate fails without it.
- **The manual is the source of truth** for behaviour: DEC's VT330/VT340 Programmer Reference Manual, Volume 2. Screen captures from a real VT340 are the evidence where the manual is silent. Code cites the chapter it follows.
- **The renderer library is sans-IO, `no_std` and integer-only.** It has no file, clock or floating point. Fonts are data passed in, and every bound on untrusted input is a named constant in `crates/regis/src/limits.rs`.
- **Security issues** are written up in `UNSOLVEDSECURITYISSUES/` while open and moved to `SOLVEDSECURITYISSUES/` when fixed. The full gate fails while any is open.
- **No library without the owner's approval by name** (DEVELOPMENT.md section 7). The workspace has no dependencies.

## Commands

- `cargo build --release`, then `target/release/regis-render [--font FILE] [--page 0|1] INPUT OUTPUT.png` and `target/release/regis-minify INPUT OUTPUT`.
- Gates: `scripts/check.sh commit | push | full`. The full gate runs the long random-input test in a debug build and checks both font files against their sources.
