# Screens

Every screen of the original, from the screen table in `REUNION.PRG`
(`DS:0x5106 + 25 * screen`: the action that opens it, then its icon bar).
"Internal" screens are opened by the game itself (buying, building, combat).

| # | Screen | Opened by | Icon bar | Status |
|---|---|---|---|---|
| 1 | Main screen (control room) | BACK TO M.SCREEN | INFO-BUY, RESEARCH-DESIGN, SHIP INFO, GALACTIC MAP, PLANET MAIN, MESSAGES, COMMANDERS, SPACE LOCAL, MAIN COMPUTER, DISK OPERATIONS | Done (you and your commanders in the room; talking to them) |
| 2 | Commanders | COMMANDERS | BACK, PILOTS, BUILDERS, FIGHTERS, DEVELOPERS, HIRE MAN | Done (hiring; levels rise over time, the university) |
| 3 | Research / design | RESEARCH-DESIGN | BACK, INFO-BUY | Done (start/stop; research progresses hourly, inventions come up) |
| 4 | Resource mine | RESOURCE-MINE | BACK, PLANET MAIN, ADD DROIDS | Done (droids, stock, mines, ores and output; mining runs hourly) |
| 5 | Info / buy | INFO-BUY | BACK, RESEARCH-DESIGN, BUY ITEM, SELECT, PROJECT UP, PROJECT DOWN | Done: spinning 3D models (software renderer; a few per-model tweaks not copied), picture / description, ore, orders; production runs hourly |
| 6 | Trade | TRADE | BACK, TRANSFER | |
| 7 | Galactic map | GALACTIC MAP | BACK (+ ZOOM OUT in the moons view) | Done: orbiting planets / moons, planet info, system buttons, destination select for moving groups; what's in orbit (your groups, alien fleets) with MOVE UNITS, ATTACK, GROUND WAR |
| 8 | Planet info | PLANET INFO | BACK, GALACTIC MAP, PLANET MAIN (+ INCREASE / DECREASE TAX) | Done, planets and moons, and what ships in orbit can put there: satellites, spy satellites / ships, solar satellites, miner stations, COLONIZATION (hotspots on the pictures missing) |
| 9 | Trade (2) | TRADE | BACK | |
| 10 | Game credits | GAME CREDITS | BACK | Done |
| 11 | Messages | MESSAGES | BACK | Done (the simulation's events go in the log) |
| 12 | Disk operations (save / load) | DISK OPERATIONS | BACK, LOAD, SAVE, GAME CREDITS, EXIT TO DOS | Done: save/load in the original format; save names are the game date (no typing yet); no music |
| 13 | Transfer | TRANSFER | BACK, SHIP INFO, CONTROL PANEL, GROUP | Done (ore and goods, room, storage, spaceport) |
| 14 | Info / buy (selecting) | internal | BACK, RESEARCH-DESIGN, BUY ITEM, SELECT OFF, PROJECT UP, PROJECT DOWN | Done (part of INFO-BUY) |
| 15 | Buy amount | internal | ADD TEN, ADD ONE, MINUS ONE, MINUS TEN, OK BUY, CANCEL BUY | Done (part of INFO-BUY) |
| 16 | Ship info | SHIP INFO | BACK (+ CONTROL PANEL, GROUP, NEW UNIT, TRANSFER, PLANET MAIN) | Done: groups / bases, details, NEW UNIT |
| 17 | Control panel | CONTROL PANEL | BACK, SHIP INFO, GROUP | Done: launch / dock, move; cockpit animations and docking story events missing |
| 18 | Ship moving | internal | ABORT MOVE | Done (destination picking on the galactic map; travel over time) |
| 19 | Ground war set-up | internal | OK, ATTACK, CANCEL ATTACK, ADD ... UNIT | Next: dividing the ground forces into units |
| 20 | Planet main (surface) | PLANET MAIN | BACK, GALACTIC MAP, PLANET INFO, SPACEPORT | Done: buildings (scaffolding while built), building / demolishing, building info, the colony model; buildings of a colony being set up find their place here. Radar minimap missing |
| 21 | New unit | internal | OK CREATE IT, ABORT | Done (name typing, type) |
| 22 | Group | GROUP | BACK, SHIP INFO (+ CONTROL PANEL, TRANSFER, GALACTIC MAP, PLANET MAIN, DISBAND UNIT) | Done: loading ships, troops and equipment from stock |
| 23 | Space local (the space station's pub) | SPACE LOCAL | BACK | Next: characters and talks (story) |
| 24 | internal | internal | BACK | Staff talk (DUMA): done |
| 25-27 | internal | internal | - | |
| 28 | Colonization | internal | OK BUILD IT, ABORT | Done: the six starting buildings, cost, founding the colony and its base |
| 29 | Space battle | internal | RETREAT | Done: the battle (ships, paths, fire, explosions), retreat, the aftermath; commander face animations on the right missing |
| 30 | Battle end | internal | END BATTLE | Done (victory / defeat with losses, part of 29) |
| 31 | internal | internal | - | |
| 32 | Ground war | internal | RETREAT | Next: the tactical ground battle (segment 0x1537) |
| 33 | internal | internal | BACK | |
| 34-36 | internal | internal | - | |
| 37 | Main computer | MAIN COMPUTER | BACK, YOUR PLANETS, USEFUL PLANETS, ALIEN PLANETS | Done (scroll bar not drawn; scroll with the right stick) |
| 38 | Start-up load | internal | LOAD, EXIT TO DOS | Done |

Each screen's background is in the same table: `grafika\<name>` from the
25-byte record's first field (e.g. 8 `bolygo`, 11 `uzenet`, 37 `colinfo`).

Also outside the screen table: main menu, choose hero and hero portrait (done),
intro and credits animations, pub talks (`TEXT/KERDES*`, `VALASZ*`).

Also done: message boxes (FUN_34b0_0237, queued while one is up; they stop
the clock), groups travelling hour by hour (arrival messages, exploring
systems), and the hourly simulation (`reunion-formats/src/sim.rs`):
invention timers, research, production, commanders' levels and the
university, construction, derricks, mining, colonies being set up, and at
midnight every colony's people, morale, taxes and troubles (disease,
radiation, meteorites, revolts), satellite observation and planets found by
observatories. Aliens: first contact (each race decides on war or
friendship), fleets moving and attacking on their timers, battles and their
aftermath. Not yet: the ground war, most of the story's own timers, and
the story scenes they start.
