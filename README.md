# Reunion

A Rust/Bevy rewrite of **Reunion** (Amnesty Design / Grandslam, 1994, DOS), built from
the original data files, with controller and keyboard support, hover highlights and
widescreen. Main target is the Steam Deck.

This repository contains no game data. You need your own copy of the DOS version
(`Reunion_DOS_EN.zip` in the project root).

## Make targets

| Command | What it does |
|---|---|
| `make run` | Run the rewrite natively |
| `make deck` | Build on the k3s builder (`deck/builder.yaml`) and copy to the Steam Deck |
| `make test` | Rust tests |
| `make web` / `make serve` | Browser build in `dist/web` |
| `make` | Package the original game for RetroArch (DOSBox Pure) |
| `make assets` | Convert `.PIC` images to PNG in `extracted/` |
| `make decompile` | Ghidra headless decompile of `REUNION.PRG` into `ghidra/` |

## Layout

- `crates/reunion-formats` — decoders for the original file formats
- `crates/reunion` — the Bevy game
- `tools/` — Python extraction scripts and the Ghidra export script
