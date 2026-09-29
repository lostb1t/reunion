# Reunion

A Rust/Bevy rewrite of **Reunion** (Amnesty Design / Grandslam, 1994, DOS), built from
the original data files, with controller and keyboard support, hover highlights and
widescreen. Main target is the Steam Deck.

The original game's files are included (`game/`).

## Play

- **Browser**: https://lostb1t.github.io/reunion/
- **Download** (Linux, Windows, macOS): the
  [latest build](https://github.com/lostb1t/reunion/releases/tag/latest). Unzip and run;
  the game's files come with it.

Controls: mouse, keyboard or controller. F8 / L3 switches the upscaling filter, F9 / R3
switches between the original 4:3 and widescreen.

How the original game works, as ported: [GAMEMECHANICS.md](GAMEMECHANICS.md). Screen by
screen status: [SCREENS.md](SCREENS.md).

## Make targets

| Command | What it does |
|---|---|
| `make run` | Run the rewrite natively |
| `make deck` | Build and copy to the Steam Deck (local script, not in the repo) |
| `make test` | Rust tests |
| `make web` / `make serve` | Browser build in `dist/web` |
| `make` | Package the original game for RetroArch (DOSBox Pure) |
| `make assets` | Convert `.PIC` images to PNG in `extracted/` |
| `make decompile` | Ghidra headless decompile of `REUNION.PRG` into `ghidra/` |

## Layout

- `crates/reunion-formats` — decoders for the original file formats
- `crates/reunion` — the Bevy game
- `tools/` — Python extraction scripts and the Ghidra export script
