#!/bin/sh
# Renders every example here into docs/images. Run from anywhere.
set -eu
cd "$(dirname "$0")/../.."
cargo build --release --locked -p regis-render
for example in docs/examples/*.regis; do
    name=$(basename "$example" .regis)
    target/release/regis-render "$example" "docs/images/$name.png"
    python3 -I tools/compress_png.py "docs/images/$name.png"
done
