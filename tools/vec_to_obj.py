#!/usr/bin/env python3
"""Export Reunion VECTORS/*.VEC as edge-only Wavefront OBJ files.

The observed records contain signed 16-bit XYZ vertices and one-based edge
indices. The remaining record data (faces, colors, and other attributes) is
not decoded here, so the OBJ output is a wireframe rather than a solid mesh.
"""

from __future__ import annotations

import argparse
import struct
from dataclasses import dataclass
from pathlib import Path


@dataclass
class Part:
    vertices: list[tuple[int, int, int]]
    edges: list[tuple[int, int]]


def read_vec(path: Path) -> list[Part]:
    data = path.read_bytes()
    if len(data) < 8 or data[-2:] != b"\xff\xff":
        raise ValueError(f"{path}: missing VEC terminator")

    offset = 0
    parts: list[Part] = []
    while offset < len(data) - 2:
        if offset + 4 > len(data) - 2:
            raise ValueError(f"{path}: incomplete part header at byte {offset}")
        record_size, vertex_count = struct.unpack_from("<HH", data, offset)
        record_end = offset + record_size
        if record_size < 6 or record_end + 2 > len(data) - 2:
            raise ValueError(f"{path}: invalid part size at byte {offset}")

        vertices_end = offset + 4 + vertex_count * 6
        if vertices_end + 2 > record_end:
            raise ValueError(f"{path}: vertices exceed part at byte {offset}")
        vertices = [
            struct.unpack_from("<hhh", data, offset + 4 + index * 6)
            for index in range(vertex_count)
        ]

        edge_count = struct.unpack_from("<H", data, vertices_end)[0]
        edges_start = vertices_end + 2
        if edges_start + edge_count * 2 > record_end:
            raise ValueError(f"{path}: edges exceed part at byte {offset}")
        edges = [
            struct.unpack_from("<BB", data, edges_start + index * 2)
            for index in range(edge_count)
        ]
        if any(a < 1 or b < 1 or a > vertex_count or b > vertex_count for a, b in edges):
            raise ValueError(f"{path}: invalid edge index at byte {offset}")

        parts.append(Part(vertices, edges))
        # Each record has a two-byte trailer. Most are zero; V27 also has 0x1819.
        offset = record_end + 2

    if offset != len(data) - 2:
        raise ValueError(f"{path}: could not parse complete file")
    return parts


def write_obj(source: Path, destination: Path, parts: list[Part], scale: float) -> None:
    destination.parent.mkdir(parents=True, exist_ok=True)
    with destination.open("w", encoding="ascii") as output:
        output.write(f"# Wireframe export of {source.name}; scale={scale:g}\n")
        vertex_base = 0
        for part_number, part in enumerate(parts, start=1):
            output.write(f"o {source.stem}_part_{part_number:02d}\n")
            for x, y, z in part.vertices:
                output.write(f"v {x * scale:.6f} {y * scale:.6f} {z * scale:.6f}\n")
            for a, b in part.edges:
                output.write(f"l {vertex_base + a} {vertex_base + b}\n")
            vertex_base += len(part.vertices)


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("source", nargs="?", type=Path, default=Path("game/VECTORS"))
    parser.add_argument("--output-dir", type=Path, default=Path("extracted/vectors/obj"))
    parser.add_argument("--scale", type=float, default=1 / 1024)
    args = parser.parse_args()

    sources = (
        sorted(args.source.glob("*.VEC"), key=lambda path: int(path.stem[1:]))
        if args.source.is_dir()
        else [args.source]
    )
    if not sources:
        parser.error(f"no .VEC files found in {args.source}")

    for source in sources:
        parts = read_vec(source)
        destination = args.output_dir / f"{source.stem}.obj"
        write_obj(source, destination, parts, args.scale)
        vertices = sum(len(part.vertices) for part in parts)
        edges = sum(len(part.edges) for part in parts)
        print(f"{source.name} -> {destination} ({len(parts)} parts, {vertices} vertices, {edges} edges)")


if __name__ == "__main__":
    main()
