//! Space battles (screen 29, REUNION.PRG segment 0x20d4).
//!
//! FUN_20d4_01d3 lines the ships up: yours (army groups, secret forces and
//! bases at the planet, and fleets of races allied with you) on the left,
//! the enemy's (fleets of races at war with you, and the defences of their
//! planets and moons there) mirrored on the right, 500 a side at most.
//! Each ship is 12 bytes: who it belongs to, its kind (1 hunter, 2 fighter,
//! 3 destroyer, 4 cruiser), its fire power and hit points, where it is, and
//! how it moves.
//!
//! FUN_20d4_0efd runs the battle, one step every fourth frame: ships fly
//! along short paths (DS:0x56b) and pick a new heading at the end of one
//! (towards the middle when near an edge, DS:0x72f); every ship fires at a
//! random enemy, hitting one time in nine; small ships hardly scratch big
//! ones, but may destroy another small one outright. Destroyed ships
//! explode over four steps. The battle is over when one side is gone.
//!
//! FUN_20d4_17f0 afterwards: of the ships each group, fleet or planet lost,
//! a fifth come back; the weapons go down in proportion.

use crate::aliens::{ALLIED, AT_WAR, FIRST_RACE, FleetId, LAST_RACE};
use crate::exe::GameExe;
use crate::sim::Random;
use crate::state::{GameState, UnitList, unit};

const MAX_SHIPS: usize = 500;
/// The battle area is 160 x 151 pixels.
pub const WIDTH: u8 = 160;
pub const HEIGHT: u8 = 151;
/// The explosion's last frame.
pub const EXPLOSION_FRAMES: u8 = 4;
/// A planet or moon's defences: 200 + moon.
const DEFENCES: u8 = 200;

/// DS:0x56b - 0x7e0 of the executable: the battle's tables.
#[derive(Debug, Clone)]
pub struct Tables {
    bytes: Vec<u8>,
    /// Fire power of the four weapons (DS:0x5c8, longs).
    weapons: [i64; 4],
    /// Hit points per kind (DS:0x5d4 + 4 * kind).
    hit_points: [i16; 5],
    /// Enemy ships' fire power per kind (DS:0x5e4 + 4 * kind).
    alien_fire: [i16; 5],
}

const TABLES: u16 = 0x56b;

impl Tables {
    pub fn read(exe: &GameExe) -> Option<Self> {
        let bytes = exe.ds_bytes(TABLES, 0x7e0 - usize::from(TABLES))?.to_vec();
        let long = |at: u16| exe.ds_bytes(at, 4).map_or(0, |b| i32::from_le_bytes([b[0], b[1], b[2], b[3]]));
        let word = |at: u16| exe.ds_bytes(at, 2).map_or(0, |b| i16::from_le_bytes([b[0], b[1]]));
        Some(Self {
            bytes,
            weapons: std::array::from_fn(|j| i64::from(long(0x5c8 + 4 * j as u16))),
            hit_points: std::array::from_fn(|k| word(0x5d4 + 4 * k as u16)),
            alien_fire: std::array::from_fn(|k| word(0x5e4 + 4 * k as u16)),
        })
    }

    fn byte(&self, address: usize) -> u8 {
        self.bytes.get(address.wrapping_sub(usize::from(TABLES))).copied().unwrap_or(0)
    }

    fn signed(&self, address: usize) -> i8 {
        self.byte(address) as i8
    }

    fn word(&self, address: usize) -> i16 {
        i16::from_le_bytes([self.byte(address), self.byte(address + 1)])
    }
}

/// A ship in battle.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Ship {
    /// Yours: 100 * list + unit; aliens': 10 * race + fleet; defences:
    /// 200 + moon.
    pub owner: u8,
    pub kind: u8,
    pub fire: i16,
    pub hit_points: i16,
    pub x: u8,
    pub y: u8,
    speed: u8,
    heading: u8,
    /// Steps along the path; after it's destroyed, the explosion's frame.
    pub step: u8,
    wait: u8,
}

#[derive(Debug, Clone)]
pub struct SpaceBattle {
    pub place: (u8, u8, u8),
    /// Your side and the enemy's; the ships still flying come first.
    pub sides: [Vec<Ship>; 2],
    pub flying: [usize; 2],
    tables: Tables,
    /// Your fighter's level decides your fire power.
    frame: u8,
    pub over: bool,
}

impl SpaceBattle {
    /// FUN_20d4_01d3: the ships at planet `place.1` of system `place.0`.
    pub fn new(state: &GameState, exe: &GameExe, place: (u8, u8, u8), random: Random) -> Option<Self> {
        let tables = Tables::read(exe)?;
        let mut sides: [Vec<Ship>; 2] = [Vec::new(), Vec::new()];
        let (s, p, _) = place;
        let fighter = i64::from(state.word(0x95a8).unwrap_or(0));
        // Your units.
        for (list_number, list) in [UnitList::Groups, UnitList::Bases].into_iter().enumerate() {
            for n in 1..=state.unit_count(list) {
                let Some(g) = state.unit(list, n) else { continue };
                if g[unit::SYSTEM] != s || g[unit::PLANET] != p || !matches!(g[unit::STATUS], 1 | 2 | 7) {
                    continue;
                }
                if !matches!(g[unit::TYPE], 1 | 3 | 5) {
                    continue;
                }
                for kind in 1..=4u8 {
                    let at = unit::SLOTS + 10 * (usize::from(kind) - 1);
                    let word = |i: usize| i64::from(i16::from_le_bytes([g[at + 2 * i], g[at + 2 * i + 1]]));
                    let count = word(0);
                    let mut fire: i64 = (1..=4).map(|j| word(j) * tables.weapons[j - 1]).sum();
                    fire = if fighter == 0 { fire * 2 / 3 } else { fire * (fighter + 200) / 220 };
                    let fire = if count < 1 { 0 } else { fire / count };
                    let owner = (list_number * 100 + n) as u8;
                    for _ in 0..count.max(0) {
                        push(&mut sides[0], &tables, owner, kind, fire as i16, false, random);
                    }
                }
            }
        }
        // Fleets of races at war (the enemy) or allied (on your side).
        for race in FIRST_RACE..=LAST_RACE {
            let standing = state.standing(race);
            if standing != AT_WAR && standing != ALLIED {
                continue;
            }
            let side = usize::from(standing == AT_WAR);
            for number in 1..=state.fleet_count(race) {
                let id = FleetId { race, number };
                let (fs, fp, _) = state.fleet_place(id);
                if fs != s || fp != p || state.fleet_byte(id, 2) != 1 {
                    continue;
                }
                for kind in 1..=4u8 {
                    for _ in 0..state.fleet_forces(id, kind.into()) {
                        let fire = tables.alien_fire[usize::from(kind)];
                        push(&mut sides[side], &tables, race * 10 + number, kind, fire, true, random);
                    }
                }
            }
        }
        // The defences of the planet and its moons.
        let moons = exe
            .star_systems()
            .ok()?
            .get(usize::from(s).checked_sub(1)?)?
            .moons
            .get(usize::from(p).checked_sub(1)?)?
            .clone();
        let bodies = std::iter::once((0u8, usize::from(p))).chain(moons.iter().enumerate().map(|(i, &b)| (i as u8 + 1, b.into())));
        for (moon, body) in bodies {
            let Some(r) = state.body(s.into(), body) else { continue };
            let owner = r[0];
            if owner < 2 {
                continue;
            }
            let standing = state.standing(owner);
            if standing != AT_WAR && standing != ALLIED {
                continue;
            }
            let side = usize::from(standing == AT_WAR);
            for kind in 1..=4u8 {
                let at = 0x17 + 4 * usize::from(kind);
                let count = i32::from_le_bytes([r[at], r[at + 1], r[at + 2], r[at + 3]]);
                for _ in 0..count.max(0) {
                    let fire = tables.alien_fire[usize::from(kind)];
                    push(&mut sides[side], &tables, DEFENCES + moon, kind, fire, true, random);
                }
            }
        }
        let flying = [sides[0].len(), sides[1].len()];
        Some(Self {
            place,
            sides,
            flying,
            tables,
            frame: 0,
            over: false,
        })
    }

    /// One frame of the original's main loop; the battle moves on every
    /// fourth (FUN_20d4_0efd). Returns whether it moved.
    pub fn frame(&mut self, cheat: bool, random: Random) -> bool {
        self.frame = self.frame % 4 + 1;
        if self.frame != 1 || self.over {
            return false;
        }
        self.step_side(0, cheat, random);
        self.step_side(1, false, random);
        // Explosions.
        let mut exploding = false;
        for side in 0..2 {
            for ship in &mut self.sides[side][self.flying[side]..] {
                if ship.step <= EXPLOSION_FRAMES {
                    ship.step += 1;
                    exploding = true;
                }
            }
        }
        self.over = (self.flying[0] == 0 || self.flying[1] == 0) && !exploding;
        true
    }

    /// Moves and fires the ships of `side` at the other side.
    fn step_side(&mut self, side: usize, cheat: bool, random: Random) {
        let t = &self.tables;
        let enemy = 1 - side;
        let dx_table = if side == 0 { 0x789 } else { 0x793 };
        for i in 0..self.flying[side] {
            let mut ship = self.sides[side][i];
            if ship.hit_points <= 0 {
                continue;
            }
            if ship.wait == 0 {
                let path = |heading: u8, speed: u8, step: u8| {
                    usize::from(t.byte(0x6e7 + usize::from(heading))) * 0x78
                        + (usize::from(speed).wrapping_sub(1)) * 0x14
                        + usize::from(step)
                        + 0x56b
                };
                let v = t.byte(path(ship.heading, ship.speed, ship.step));
                let way = usize::from(t.byte(usize::from(ship.heading) * 9 + usize::from(v) + 0x6e6));
                ship.x = ship.x.wrapping_add(t.signed(way + dx_table) as u8);
                ship.y = ship.y.wrapping_add(t.signed(way + 0x79d) as u8);
                ship.step = ship.step.wrapping_add(1);
                if t.byte(path(ship.heading, ship.speed, ship.step)) == 0 {
                    let third = |v: u8, a: u8| -> usize { if v < a { 0 } else if v < 0x6f { 1 } else { 2 } };
                    let mut column = third(ship.x, 0x32);
                    if side == 1 {
                        column = 2 - column;
                    }
                    let row = third(ship.y, 0x28);
                    ship.speed = random(6) as u8 + 2;
                    let region = column + row * 3 + 1;
                    ship.heading = t.byte(region * 9 + usize::from(random(8)) + 0x72f);
                    ship.step = 1;
                }
                let kind = usize::from(ship.kind);
                let range = t.word(kind * 2 + 0x7ae).max(0) as u16;
                ship.wait = (t.word(kind * 2 + 0x7a6) as u8).wrapping_add(random(range) as u8);
            } else {
                ship.wait -= 1;
            }
            self.sides[side][i] = ship;
            // Fire.
            if self.flying[enemy] == 0 {
                continue;
            }
            let target = usize::from(random(self.flying[enemy] as u16));
            if random(100) >= 11 {
                continue;
            }
            let mut victim = self.sides[enemy][target];
            let damage = if ship.kind < 3 && victim.kind >= 3 {
                (ship.fire as u16 / 3) as i16
            } else {
                ship.fire
            };
            victim.hit_points = victim.hit_points.wrapping_sub(damage);
            if ship.kind < 3 && victim.kind < 3 {
                let chance = t.word(usize::from(ship.kind) * 4 + usize::from(victim.kind) * 2 + 0x7ca);
                if chance < 0 || random(100) as i16 <= chance {
                    victim.hit_points = 0;
                }
            }
            if cheat {
                victim.hit_points = 0;
            }
            self.sides[enemy][target] = victim;
            if victim.hit_points < 1 {
                // It explodes, moved behind the ships still flying.
                self.sides[enemy][target].step = 1;
                let last = self.flying[enemy] - 1;
                self.sides[enemy].swap(target, last);
                self.flying[enemy] = last;
            }
        }
    }

    /// Whether you won: the enemy is gone and you're still there.
    pub fn won(&self) -> bool {
        self.flying[0] > 0
    }

    /// FUN_20d4_17f0: the survivors go back to their groups, fleets and
    /// planets (a fifth of the lost come back). Returns the losses per
    /// kind, yours and the enemy's.
    pub fn finish(&self, state: &mut GameState, exe: &GameExe) -> [[u32; 4]; 2] {
        let mut losses = [[0u32; 4]; 2];
        let survivors = |side: usize, owner: u8, kind: u8| {
            self.sides[side][..self.flying[side]].iter().filter(|s| s.owner == owner && s.kind == kind).count() as i64
        };
        let (s, p, _) = self.place;
        for (list_number, list) in [UnitList::Groups, UnitList::Bases].into_iter().enumerate() {
            for n in 1..=state.unit_count(list) {
                let Some(g) = state.unit(list, n).map(<[u8]>::to_vec) else { continue };
                if g[unit::SYSTEM] != s || g[unit::PLANET] != p || !matches!(g[unit::STATUS], 1 | 2 | 7) {
                    continue;
                }
                if !matches!(g[unit::TYPE], 1 | 3 | 5) {
                    continue;
                }
                let owner = (list_number * 100 + n) as u8;
                let Some(record) = state.unit_mut(list, n) else { continue };
                for kind in 1..=4u8 {
                    let at = unit::SLOTS + 10 * (usize::from(kind) - 1);
                    let count = i64::from(i16::from_le_bytes([record[at], record[at + 1]]));
                    let alive = survivors(0, owner, kind);
                    let kept = alive + (count - alive) / 5;
                    for j in 1..=4 {
                        let w = at + 2 * j;
                        let value = i64::from(i16::from_le_bytes([record[w], record[w + 1]]));
                        let value = if count > 0 { value * kept / count } else { 0 };
                        record[w..w + 2].copy_from_slice(&(value as i16).to_le_bytes());
                    }
                    losses[0][usize::from(kind) - 1] += (count - kept).max(0) as u32;
                    record[at..at + 2].copy_from_slice(&(kept as i16).to_le_bytes());
                }
            }
        }
        for race in FIRST_RACE..=LAST_RACE {
            let standing = state.standing(race);
            if standing != AT_WAR && standing != ALLIED {
                continue;
            }
            let side = usize::from(standing == AT_WAR);
            for number in 1..=state.fleet_count(race) {
                let id = FleetId { race, number };
                let (fs, fp, _) = state.fleet_place(id);
                if fs != s || fp != p || state.fleet_byte(id, 2) != 1 {
                    continue;
                }
                for kind in 1..=4u8 {
                    let count = i64::from(state.fleet_forces(id, kind.into()));
                    let alive = survivors(side, race * 10 + number, kind);
                    let kept = alive + (count - alive) / 5;
                    losses[side][usize::from(kind) - 1] += (count - kept).max(0) as u32;
                    state.set_fleet_forces(id, kind.into(), kept as u16);
                }
            }
        }
        if let Some(moons) = exe
            .star_systems()
            .ok()
            .and_then(|l| l.get(usize::from(s).wrapping_sub(1)).and_then(|l| l.moons.get(usize::from(p).wrapping_sub(1)).cloned()))
        {
            let bodies = std::iter::once((0u8, usize::from(p))).chain(moons.iter().enumerate().map(|(i, &b)| (i as u8 + 1, b.into())));
            for (moon, body) in bodies {
                let Some(owner) = state.body(s.into(), body).map(|r| r[0]) else { continue };
                if owner < 2 {
                    continue;
                }
                let standing = state.standing(owner);
                if standing != AT_WAR && standing != ALLIED {
                    continue;
                }
                let side = usize::from(standing == AT_WAR);
                let Some(r) = state.planet_mut(s.into(), body) else { continue };
                for kind in 1..=4u8 {
                    let at = 0x17 + 4 * usize::from(kind);
                    let count = i64::from(i32::from_le_bytes([r[at], r[at + 1], r[at + 2], r[at + 3]]));
                    let alive = survivors(side, DEFENCES + moon, kind);
                    let kept = alive + (count - alive) / 5;
                    losses[side][usize::from(kind) - 1] += (count - kept).max(0) as u32;
                    r[at..at + 4].copy_from_slice(&(kept as i32).to_le_bytes());
                }
            }
        }
        losses
    }
}

/// A new ship at its side's starting area (the enemy's mirrored).
fn push(side: &mut Vec<Ship>, t: &Tables, owner: u8, kind: u8, fire: i16, enemy: bool, random: Random) {
    if side.len() >= MAX_SHIPS {
        return;
    }
    let k = usize::from(kind);
    let x = t.byte(0x7b7 + k).wrapping_add(random(u16::from(t.byte(0x7bb + k))) as u8);
    let x = if enemy { 0xa0u8.wrapping_sub(x) } else { x };
    let y = t.byte(0x7bf + k).wrapping_add(random(u16::from(t.byte(0x7c3 + k))) as u8);
    side.push(Ship {
        owner,
        kind,
        fire,
        hit_points: t.hit_points[k],
        x,
        y,
        speed: random(6) as u8 + 2,
        heading: t.byte(usize::from(random(8)) + 0x753),
        step: 0,
        wait: 1,
    });
}
