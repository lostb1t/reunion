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

## How the game runs

Everything below was read from `REUNION.PRG` (Ghidra decompile plus disassembly) and
lives in `crates/reunion-formats` (`sim.rs`, `story.rs`, `aliens.rs`, `battle.rs`,
`ground.rs`, `deploy.rs`, `pub_people.rs`, `colony.rs`).

### The clock

The original counts VGA frames in its main loop. One game hour passes every 101 frames of
the 70.086 Hz VGA refresh, about 1.44 seconds. Message boxes, battles, conversations,
colonization and the disk screen stop the clock.

### Every hour, in the original's order

1. **Invention timers**: while you have a developer who isn't away, inventions your
   developers have thought of count down. At 0 the invention becomes developable
   (TEXT/KITALAL.TXT message).
2. **Story timeline** (see *Aliens and attacks*).
3. **Research**: every project being developed progresses by the developer's level,
   but stalls where the developer's skills fall short of the project's requirements.
   When finished: "The … is ready for production", and some inventions start others.
4. **Production** on New Earth: ordered pieces are made one after another into stock.
5. **At midnight, the day** for every planet and moon of the known systems:
   - Colonies: population grows towards what housing, food and hospitals allow, and
     morale (higher taxes lower it; leisure buildings raise it). Taxes (population ×
     tax × morale × wealth) go to your money.
   - Troubles: disease without a medicine plant, radiation near the sun without an
     anti-radiation shield, meteorites, starvation, lack of space, revolts. A colony
     under 1000 people is lost.
   - Satellites, droids and colonies reveal a planet over the days: ores at 10, whether
     people can live there at 30, whose colony it is at 40.
   - A landed colony is set up after a day or two.
   - Observatories, ships in a system and the Psy-radar find hidden planets.
6. **Commanders**: now and then a level better; the university returns them much better.
7. **Construction**: sites progress by the planet's builder plants and your builder.
   Derricks pump Detoxin.
8. **Colonies being set up** get their chosen buildings one by one.
9. **Mining**: droids dig ores by the planet's richness, into storage (buildings give
   storage).
10. **Travel**: groups move; arriving reveals moons and meets new races.
11. **The pub**: hired people come back from their missions.
12. **Aliens**: fleets on their way arrive; fleets told to attack count down and attack.

### Aliens and attacks

There is no free-roaming alien AI. Alien fleets sit at their planets until the story
moves them, and every attack on you is scripted. It is set off by an event, then lands
after a delay:

- **First contact** decides a race's standing: the Morgruls, the League (Lisonians,
  Undorlings, Hirachi, Druedians) and the Earthlings go to war; the Kalls and the
  Syonians become allies; the others are neutral. Contact happens when your ships arrive
  where a race lives or has a fleet, or when an alien fleet arrives at your colony.
- **Contact with the Jaanosians** starts the first chain: their calls (conversations, if
  you have the communicator), their S.O.S., then their fall to the Morgruls (about 3000
  hours after contact). About 600 hours later the Morgruls invade New Earth, then twice
  more (about 1800 and 3000 hours later), then a scheduled attack about 8000 hours after
  that, and another 4500 hours later.
- **Contact with the Kalls**: after about 1500–2000 hours they and the Morgruls take
  the Phelonians, the Kalls turn on you, and both attack New Earth about 300 hours later.
- **Contact with a League race**: they attack New Earth 200–250 hours later, then the
  League follows up with a chain of scheduled attacks 1000–3000 hours apart.
- **Contact with the Earthlings**: an attack 1000–1200 hours later, and follow-ups
  2000–2500 hours apart.
- A fleet sent to attack strikes after 5 to (105 − the race's aggressiveness) hours.
  Scheduled follow-ups only happen while that race is still at war with you.
- **You** start fights too: ATTACK on an alien fleet or GROUND WAR on an alien planet
  (from the galactic map) puts that race at war.

An attack starts with a space battle around the planet. An invasion that wins in space
goes on to a ground war on your colony. Winning a ground war on an alien planet makes it
your colony; taking a race's capital eliminates it. Taking the Earthlings' capital wins
the game, and losing New Earth ends it.

## Cheats

The executable contains two developer cheats, but nothing in the shipped game turns
them on: the only writes to their flags are resets to 0 (at start-up and at the start of
each battle). The key combination that set them was apparently removed before release.

- **Money and research** (flag `DS:0x91e7`, `FUN_26eb_0000`, checked every main-loop
  frame): each time it fires, +10,000 credits. With a mouse button held, the first time
  instead sets money to 1,000,000, finishes every invention, and adds 10 pieces to
  everything being produced.
- **Win battles** (flag `DS:0x91e8`): in space battles every hit destroys its target; a
  ground war ends at once as a win.

The rewrite honours the battle cheat (the battles read `0x91e8`); for testing, the
autopilot can set it with `byte 91e8 1`. The money cheat isn't ported.
