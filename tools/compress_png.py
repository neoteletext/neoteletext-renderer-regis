"""Recompress a PNG's pixel data, in place.

regis-render stores pixel data uncompressed so that it needs no compression
library. Pictures kept in the repository are recompressed with this script.

Usage: python3 -I compress_png.py picture.png [more.png ...]
"""

import struct
import sys
import zlib

SIGNATURE = b"\x89PNG\r\n\x1a\n"
MAX_BYTES = 64 * 1024 * 1024


def chunks(data):
    position = len(SIGNATURE)
    while position < len(data):
        (length,) = struct.unpack(">I", data[position:position + 4])
        kind = data[position + 4:position + 8]
        yield kind, data[position + 8:position + 8 + length]
        position += 12 + length


def chunk(kind, body):
    return struct.pack(">I", len(body)) + kind + body + struct.pack(">I", zlib.crc32(kind + body))


def recompress(path):
    with open(path, "rb") as source:
        data = source.read(MAX_BYTES + 1)
    if len(data) > MAX_BYTES or not data.startswith(SIGNATURE):
        raise SystemExit(f"{path}: not a PNG file this script accepts")
    parts = list(chunks(data))
    pixels = zlib.decompress(b"".join(body for kind, body in parts if kind == b"IDAT"))
    out = SIGNATURE
    written = False
    for kind, body in parts:
        if kind == b"IDAT":
            if not written:
                out += chunk(b"IDAT", zlib.compress(pixels, 9))
                written = True
        else:
            out += chunk(kind, body)
    with open(path, "wb") as target:
        target.write(out)


for name in sys.argv[1:]:
    recompress(name)
