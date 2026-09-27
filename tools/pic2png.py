"""Convert Reunion "SpidyGfx" .PIC images to PNG.

Format:
  0   8   magic b"SpidyGfx"
  8   2   width  (u16 LE)
  10  2   height (u16 LE)
  12  ..  PCX-style RLE: byte >= 0xC0 -> repeat next byte (b & 0x3F) times,
          otherwise a literal pixel
  -769 1  0x0C palette marker
  -768 768 palette, 256 x RGB, 8 bits per channel

Usage: pic2png.py <game_dir> <out_dir>
"""
import struct
import sys
from pathlib import Path

from PIL import Image

MAGIC = b"SpidyGfx"


def decode_pic(data: bytes) -> Image.Image:
    if data[:8] != MAGIC:
        raise ValueError("not a SpidyGfx file")
    w, h = struct.unpack_from("<HH", data, 8)
    if data[-769] != 0x0C:
        raise ValueError("palette marker missing")
    rle = data[12:-769]
    need = w * h
    out = bytearray()
    i = 0
    while i < len(rle) and len(out) < need:
        b = rle[i]
        i += 1
        if b >= 0xC0:
            out += bytes([rle[i]]) * (b & 0x3F)
            i += 1
        else:
            out.append(b)
    # A few files overrun by a run or two; clamp to the declared size.
    out = bytes(out[:need]).ljust(need, b"\0")
    img = Image.frombytes("P", (w, h), out)
    img.putpalette(data[-768:])
    return img


def main(game_dir: str, out_dir: str) -> None:
    ok = skipped = 0
    for src in sorted(Path(game_dir).glob("*/*.PIC")):
        data = src.read_bytes()
        if data[:8] != MAGIC:
            skipped += 1
            continue
        dst = Path(out_dir) / src.parent.name / (src.stem + ".png")
        dst.parent.mkdir(parents=True, exist_ok=True)
        decode_pic(data).save(dst)
        ok += 1
    print(f"converted {ok}, skipped {skipped} (no SpidyGfx magic)")


if __name__ == "__main__":
    main(*sys.argv[1:3])
