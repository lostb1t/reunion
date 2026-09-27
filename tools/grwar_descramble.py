"""Descramble GRWAR/*.PIC files (animation scripts, not images).

Scheme from REUNION.PRG FUN_4265_000c: the file is stored byte-reversed with
0x2F subtracted from every byte. Decode = reverse, then add 0x2F (mod 256).

Usage: grwar_descramble.py <game_dir> <out_dir>
"""
import sys
from pathlib import Path


def descramble(data: bytes) -> bytes:
    return bytes((b + 0x2F) & 0xFF for b in reversed(data))


def main(game_dir: str, out_dir: str) -> None:
    out = Path(out_dir)
    out.mkdir(parents=True, exist_ok=True)
    for src in sorted(Path(game_dir, "GRWAR").glob("*.PIC")):
        (out / (src.stem + ".bin")).write_bytes(descramble(src.read_bytes()))
        print(src.name)


if __name__ == "__main__":
    main(*sys.argv[1:3])
