"""Annotated disassembly of a REUNION.PRG code range.

  asm.py <segment> <start> <end>        e.g. asm.py 2d66 03e9 074c

Resolves code-segment Pascal strings loaded with `mov di, <offset>; push cs`
and names the common runtime and drawing calls, so text-heavy functions
(which Ghidra decompiles with calls missing) read as: which string, drawn
where. Text draws (FUN_405f_1218 and friends) take (len, y, x, text), pushed
right to left: after the text come x, y, len.
"""
import sys
from pathlib import Path

from capstone import CS_ARCH_X86, CS_MODE_16, Cs

EXE = Path(__file__).resolve().parent.parent / "game" / "GRWAR" / "REUNION.PRG"
HEADER = 0x4AE0
LOAD_SEGMENT = 0x1000

CALLS = {
    "0xc96": "strcopy",
    "0xc7c": "strload",
    "0xcfb": "strcat",
    "0x1a38": "Str(int)",
    "0x1218": "TEXT yellow",
    "0x124b": "TEXT red",
    "0x12e4": "TEXT (disk normal)",
    "0x1317": "TEXT (disk selected)",
    "0x1056": "TEXT color",
    "0x5f2": "FILLRECT",
    "0x25": "BLIT",
    "0xde": "BLIT stride",
    "0x31a": "SOUND",
    "0x18b3": "Random",
}


def main(segment: str, start: str, end: str) -> None:
    data = EXE.read_bytes()
    seg, lo, hi = int(segment, 16), int(start, 16), int(end, 16)
    base = HEADER + (seg - LOAD_SEGMENT) * 16

    def pascal(offset: int) -> str | None:
        n = data[base + offset]
        text = data[base + offset + 1 : base + offset + 1 + n]
        if 0 < n < 80 and all(32 <= c < 127 for c in text):
            return text.decode()
        return None

    md = Cs(CS_ARCH_X86, CS_MODE_16)
    code = list(md.disasm(data[base + lo : base + hi], lo))
    for k, ins in enumerate(code):
        ops = ins.op_str
        if ins.mnemonic == "mov" and ops.startswith("di, 0x") and k + 1 < len(code) and code[k + 1].op_str == "cs":
            offset = int(ops.split(", ")[1], 16)
            text = pascal(offset)
            print(f"{ins.address:#06x}   STR {text!r}" if text is not None else f"{ins.address:#06x} mov {ops}")
            continue
        if ins.mnemonic in ("lcall", "call"):
            target = ops.split(", ")[-1]
            print(f"{ins.address:#06x}   CALL {CALLS.get(target, ops)}")
            continue
        if ins.mnemonic in ("push", "lea") and ("bp -" in ops or ops in ("ss", "di", "cs", "ds", "es")):
            continue
        print(f"{ins.address:#06x} {ins.mnemonic} {ops}")


if __name__ == "__main__":
    main(*sys.argv[1:4])
