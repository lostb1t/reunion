# Screens

Every screen of the original, from the screen table in `REUNION.PRG`
(`DS:0x5106 + 25 * screen`: the action that opens it, then its icon bar).
"Internal" screens are opened by the game itself (buying, building, combat).

| # | Screen | Opened by | Icon bar | Status |
|---|---|---|---|---|
| 1 | Main screen (control room) | BACK TO M.SCREEN | INFO-BUY, RESEARCH-DESIGN, SHIP INFO, GALACTIC MAP, PLANET MAIN, MESSAGES, COMMANDERS, SPACE LOCAL, MAIN COMPUTER, DISK OPERATIONS | Done (people in the room missing) |
| 2 | Commanders | COMMANDERS | BACK, PILOTS, BUILDERS, FIGHTERS, DEVELOPERS, HIRE MAN | |
| 3 | Research / design | RESEARCH-DESIGN | BACK, INFO-BUY | |
| 4 | Resource mine | RESOURCE-MINE | BACK, PLANET MAIN, ADD DROIDS | |
| 5 | Info / buy | INFO-BUY | BACK, RESEARCH-DESIGN, BUY ITEM, SELECT, PROJECT UP, PROJECT DOWN | |
| 6 | Trade | TRADE | BACK, TRANSFER | |
| 7 | Galactic map | GALACTIC MAP | BACK | |
| 8 | Planet info | PLANET INFO | BACK, GALACTIC MAP, PLANET MAIN (+ INCREASE / DECREASE TAX) | Done (hotspots on the pictures missing) |
| 9 | Trade (2) | TRADE | BACK | |
| 10 | Game credits | GAME CREDITS | BACK | |
| 11 | Messages | MESSAGES | BACK | Done (the game doesn't generate messages yet) |
| 12 | Disk operations (save / load) | DISK OPERATIONS | BACK, LOAD, SAVE, GAME CREDITS, EXIT TO DOS | Done: save/load in the original format; save names are the game date (no typing yet); no music |
| 13 | Transfer | TRANSFER | BACK, SHIP INFO, CONTROL PANEL, GROUP | |
| 14 | Info / buy (selecting) | internal | BACK, RESEARCH-DESIGN, BUY ITEM, SELECT OFF, PROJECT UP, PROJECT DOWN | |
| 15 | Buy amount | internal | ADD TEN, ADD ONE, MINUS ONE, MINUS TEN, OK BUY, CANCEL BUY | |
| 16 | Ship info | SHIP INFO | BACK | |
| 17 | Control panel | CONTROL PANEL | BACK, SHIP INFO, GROUP | |
| 18 | Ship moving | internal | ABORT MOVE | |
| 19 | Attack prompt | internal | OK, ATTACK | |
| 20 | Planet main (surface) | PLANET MAIN | BACK, GALACTIC MAP, PLANET INFO, SPACEPORT | Surface done; buildings, build panel missing |
| 21 | Create (ship design?) | internal | OK CREATE IT, ABORT | |
| 22 | Group | GROUP | BACK, SHIP INFO | |
| 23 | Space local | SPACE LOCAL | BACK | |
| 24 | internal | internal | BACK | |
| 25-27 | internal | internal | - | |
| 28 | Build confirm | internal | OK BUILD IT, ABORT | |
| 29 | Battle (retreat) | internal | RETREAT | |
| 30 | Battle end | internal | END BATTLE | |
| 31 | internal | internal | - | |
| 32 | Battle (retreat, 2) | internal | RETREAT | |
| 33 | internal | internal | BACK | |
| 34-36 | internal | internal | - | |
| 37 | Main computer | MAIN COMPUTER | BACK, YOUR PLANETS, USEFUL PLANETS, ALIEN PLANETS | Done (scroll bar not drawn; scroll with the right stick) |
| 38 | Start-up load | internal | LOAD, EXIT TO DOS | |

Each screen's background is in the same table: `grafika\<name>` from the
25-byte record's first field (e.g. 8 `bolygo`, 11 `uzenet`, 37 `colinfo`).

Also outside the screen table: main menu, choose hero and hero portrait (done),
intro and credits animations, pub talks (`TEXT/KERDES*`, `VALASZ*`).
