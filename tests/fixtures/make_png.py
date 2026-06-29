#!/usr/bin/env python3
"""Generate minimal PNG files for tests. CRCs are not validated by our schema,
so we use real CRC32 anyway to keep the files honest."""
import struct, zlib, sys

def chunk(typ: bytes, data: bytes) -> bytes:
    body = typ + data
    return struct.pack(">I", len(data)) + body + struct.pack(">I", zlib.crc32(body) & 0xffffffff)

def png(width: int, height: int, color_type: int = 2, extra_text: bytes | None = None) -> bytes:
    sig = bytes([0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a])
    ihdr = struct.pack(">IIBBBBB", width, height, 8, color_type, 0, 0, 0)
    out = sig + chunk(b"IHDR", ihdr)
    out += chunk(b"IDAT", b"\x78\x9c\x63\x00\x00\x00\x01\x00\x01")
    if extra_text is not None:
        out += chunk(b"tEXt", extra_text)
    out += chunk(b"IEND", b"")
    return out

if __name__ == "__main__":
    out = sys.argv[1]
    spec = sys.argv[2]
    if spec == "1x1":
        data = png(1, 1)
    elif spec == "2x1":
        data = png(2, 1)
    elif spec == "2x1+text":
        data = png(2, 1, extra_text=b"Author\x00kdiff")
    elif spec == "1x1-indexed":
        data = png(1, 1, color_type=3)
    else:
        raise SystemExit(f"unknown spec {spec}")
    with open(out, "wb") as f:
        f.write(data)
