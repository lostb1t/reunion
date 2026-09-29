# Reunion

A Rust/Bevy rewrite of **Reunion** (Amnesty Design / Grandslam, 1994, DOS), built from
the original data files:

- **Native builds** for Linux, Windows and macOS, and a browser version.
- **Controller support**, next to mouse and keyboard.

The original game's files are included (`game/`).

## Play

- **Browser**: https://lostb1t.github.io/reunion/
- **Download** (Linux, Windows, macOS): the
  [latest build](https://github.com/lostb1t/reunion/releases/tag/latest). Unzip and run;
  the game's files come with it.

Controls: mouse, keyboard or controller. F8 / L3 switches the upscaling filter, F9 / R3
switches between the original 4:3 and widescreen (experimental).

### Settings

Not in the original: the main screen's icon bar has a **Settings** icon (a cogwheel, on
the bar's second page after Disk Operations). The page has:

- **Widescreen (experimental)**: use the width of wide screens; off keeps the original 4:3.
- **Upscaling**: off (plain pixels) or CRT.
- **Music**: track 1, track 2 or off.
- **Speech**: icons say what they do, or just click.
- **Cheat: money and research**: money up to 1,000,000 and the research in progress
  finished when turned on, then +10,000 credits an hour.
- **Cheat: win every battle**: space battles' hits always destroy, ground wars are won at
  once.

The cheats are the original developers' (see [GAMEMECHANICS.md](GAMEMECHANICS.md#cheats)),
which nothing in the shipped game turns on. Settings are kept in `assets/SAVE/SETTINGS.TXT`.

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
