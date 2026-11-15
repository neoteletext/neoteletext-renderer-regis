# neoteletext-renderer-regis

A renderer for DEC ReGIS, the vector graphics language of the VT125, VT240,
VT330 and VT340 terminals, written from scratch in Rust. It is the third
NeoTeletext renderer, beside the World System Teletext engine and the
NAPLPS renderer.

Bytes go in; colour-map addresses come out on an 800 by 480 screen of
sixteen colours that the caller owns. The library (`crates/regis`) is
`no_std`, uses no other crate, does no input or output and no floating
point.

```sh
cargo run --release -p regis-render -- picture.regis picture.png
cargo run --release -p regis-minify -- picture.regis shorter.regis
```

`regis-minify` (`crates/regis-minify`) rewrites ReGIS shorter without
changing what it draws, and proves it by drawing both.

- `docs/guide.html`: how to use the library and the tool.
- `docs/reference.html`: every command, limit and difference from a VT340.
- `docs/integration-browser.html`, `docs/integration-server.html`: the plans
  for the NeoTeletext browser and server. Nothing in them is built.
- `scripts/check.sh commit|push|full`: the local gate.
- `SOLVEDSECURITYISSUES/`, `UNSOLVEDSECURITYISSUES/`: the security record.

Requirements and decisions are kept in rdm, not in files.
