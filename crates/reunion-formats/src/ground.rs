//! The ground war (screens 19 and 32; REUNION.PRG segments 0x1430 and
//! 0x1537).
//!
//! Forces: your army groups and bases on the planet or moon bring their
//! ground units (the second slot's four kinds: troopers, battle tanks,
//! aircraft, rocket launchers). A kind's attack is its first two weapons
//! times DS:0x2 / 0x6 (longs); the aliens' come from their fleets (words 5-8)
//! and the planet's defences (longs 5-8), their attack per piece from
//! DS:0x10 + 2 * kind. The forces are divided into units of at most
//! DS:0x30 + 2 * kind pieces, 20 a side (FUN_1430_0479); the rest wait.
//!
//! The battle (FUN_1537_1c05): a 16 x 9 grid of 16-pixel cells. Units hold,
//! move to a cell or attack a unit (the enemy always attacks the nearest).
//! A unit fires at its target, or anything in range (troopers and rocket
//! launchers next to it, aircraft two cells, tanks two steps), every
//! DS:0x20 + 2 * kind frames: damage is the pieces times the attack per
//! piece times the target kind's weakness (DS:0x28) / 500, 1 to 15 pieces.
//! Rocket launchers further away (2-4 cells) fire rockets that hit what's
//! in the target cell when they land. Units move a pixel every
//! DS:0x7e + 2 * kind frames, finding their way around others
//! (FUN_1537_34d4). It ends when one side has no units left.

use std::collections::VecDeque;

use crate::aliens::{ALLIED, AT_WAR, FIRST_RACE, FleetId, LAST_RACE};
use crate::exe::GameExe;
use crate::sim::Random;
use crate::state::{GameState, UnitList, unit};

pub const COLUMNS: usize = 16;
pub const ROWS: usize = 9;
pub const MAX_UNITS: usize = 20;
/// Where a side's colony (its headquarters) is: an id past the units.
pub const HEADQUARTERS: usize = 21;

pub const HOLD: u8 = 1;
pub const MOVE: u8 = 2;
pub const ATTACK: u8 = 3;

/// A side's forces per kind (1-4, index kind - 1).
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Forces {
    pub pieces: [i64; 4],
    /// Attack of all the pieces of a kind together.
    pub attack: [i64; 4],
}

impl Forces {
    fn per_piece(&self, kind: u8) -> i64 {
        let k = usize::from(kind.clamp(1, 4)) - 1;
        if self.pieces[k] <= 0 { 0 } else { self.attack[k] / self.pieces[k] }
    }
}

/// A unit on the battlefield (a 39-byte record in the original).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Unit {
    pub kind: u8,
    pub pieces: i32,
    pub column: u8,
    pub row: u8,
    /// Frames until it may fire again.
    pub reload: u16,
    /// 1 left, 2 up, 3 right, 4 down.
    pub facing: u8,
    pub moving: bool,
    /// Pixels moved towards the next cell, 0-16.
    pub progress: u8,
    step_wait: u8,
    /// An animation (1-4 firing, 5 rocket launch, 7 exploding, 8 hit) and
    /// its frames left.
    pub animation: u8,
    pub frames_left: u8,
    frame_wait: u8,
    pub order: u8,
    /// The target unit (ATTACK) or cell (MOVE: column, row).
    pub target: u8,
    pub target_row: u8,
}

impl Unit {
    /// Where it's drawn: pixels from the battlefield's corner.
    pub fn position(&self) -> (i32, i32) {
        let (dx, dy) = step(self.facing);
        (
            i32::from(self.column) * 16 + i32::from(self.progress) * dx,
            i32::from(self.row) * 16 + i32::from(self.progress) * dy,
        )
    }
}

/// The cell offset of a direction (DS:0x44 / 0x4e).
fn step(facing: u8) -> (i32, i32) {
    match facing {
        1 => (-1, 0),
        2 => (0, -1),
        3 => (1, 0),
        4 => (0, 1),
        _ => (0, 0),
    }
}

/// A rocket in flight.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Rocket {
    pub from: (i32, i32),
    pub to: (i32, i32),
    pub frame: i32,
    pub frames: i32,
    /// 0-15, for its picture.
    pub heading: u8,
    damage: i64,
}

impl Rocket {
    pub fn position(&self) -> (i32, i32) {
        (
            self.from.0 + (self.to.0 - self.from.0) * self.frame / self.frames.max(1),
            self.from.1 + (self.to.1 - self.from.1) * self.frame / self.frames.max(1),
        )
    }
}

#[derive(Debug, Clone)]
pub struct GroundBattle {
    pub place: (u8, u8, u8),
    pub you_attack: bool,
    /// Yours (0) and the enemy's (1).
    pub units: [Vec<Unit>; 2],
    pub forces: [Forces; 2],
    /// Pieces not in a unit, per kind.
    pub reserve: [[i64; 4]; 2],
    pub rockets: [Vec<Rocket>; 2],
    /// Which unit is in each cell: side * 100 + unit (1-based); 0 empty.
    grid: [[u8; ROWS]; COLUMNS],
    /// Headquarters' corner cells (yours, the enemy's) and sizes.
    pub headquarters: [((u8, u8), u8); 2],
    pub over: bool,
    tables: Tables,
}

#[derive(Debug, Clone)]
struct Tables {
    reload: [u16; 5],
    weakness: [i64; 5],
    largest: [i64; 5],
    step_wait: [u8; 5],
    animation: [(u8, u8, u8); 11],
}

impl Tables {
    fn read(exe: &GameExe) -> Self {
        let word = |at: u16| exe.ds_bytes(at, 2).map_or(0, |b| i16::from_le_bytes([b[0], b[1]]));
        Self {
            reload: std::array::from_fn(|k| word(0x20 + 2 * k as u16).max(0) as u16),
            weakness: std::array::from_fn(|k| i64::from(word(0x28 + 2 * k as u16))),
            largest: std::array::from_fn(|k| i64::from(word(0x30 + 2 * k as u16))),
            step_wait: std::array::from_fn(|k| word(0x7e + 2 * k as u16) as u8),
            animation: std::array::from_fn(|a| {
                let at = 0x82 + 6 * a as u16;
                (word(at) as u8, word(at + 2) as u8, word(at + 4) as u8)
            }),
        }
    }

    /// Where an animation's frames are: first cell, row, count.
    pub fn frames(&self, animation: u8) -> (u8, u8, u8) {
        self.animation.get(usize::from(animation)).copied().unwrap_or((1, 1, 1))
    }
}

impl GameState {
    /// FUN_1430_0479: both sides' ground forces at a planet or moon.
    pub fn ground_forces(&self, exe: &GameExe, (s, p, m): (u8, u8, u8)) -> [Forces; 2] {
        let long = |at: u16| exe.ds_bytes(at, 4).map_or(0, |b| i64::from(i32::from_le_bytes([b[0], b[1], b[2], b[3]])));
        let word = |at: u16| exe.ds_bytes(at, 2).map_or(0, |b| i64::from(i16::from_le_bytes([b[0], b[1]])));
        let (w1, w2) = (long(0x2), long(0x6));
        let mut forces = [Forces::default(), Forces::default()];
        for list in [UnitList::Groups, UnitList::Bases] {
            for n in 1..=self.unit_count(list) {
                let Some(g) = self.unit(list, n) else { continue };
                if (g[unit::SYSTEM], g[unit::PLANET], g[unit::MOON]) != (s, p, m) || !matches!(g[unit::STATUS], 1 | 2 | 7) {
                    continue;
                }
                if g[unit::TYPE] != 1 && g[unit::TYPE] != 5 {
                    continue;
                }
                for k in 0..4 {
                    let at = 0x45 + 10 * k;
                    let w = |i: usize| i64::from(i16::from_le_bytes([g[at + 2 * i], g[at + 2 * i + 1]]));
                    forces[0].pieces[k] += w(0);
                    forces[0].attack[k] += w(1) * w1 + w(2) * w2;
                }
            }
        }
        let attack = |k: usize| word(0x12 + 2 * k as u16);
        for race in FIRST_RACE..=LAST_RACE {
            let standing = self.standing(race);
            if standing != ALLIED && standing != AT_WAR {
                continue;
            }
            let side = usize::from(standing == AT_WAR);
            for number in 1..=self.fleet_count(race) {
                let id = FleetId { race, number };
                if self.fleet_place(id) != (s, p, m) || self.fleet_byte(id, 2) != 1 {
                    continue;
                }
                for k in 0..4 {
                    let pieces = i64::from(self.fleet_forces(id, 5 + k as u16));
                    forces[side].pieces[k] += pieces;
                    forces[side].attack[k] += pieces * attack(k);
                }
            }
        }
        if let Some(body) = self.body_number(exe, (s, p, m))
            && let Some(r) = self.body(s.into(), body)
            && r[0] > 1
        {
            let standing = self.standing(r[0]);
            if standing == ALLIED || standing == AT_WAR {
                let side = usize::from(standing == AT_WAR);
                for k in 0..4 {
                    let at = 0x2b + 4 * k;
                    let pieces = i64::from(i32::from_le_bytes([r[at], r[at + 1], r[at + 2], r[at + 3]]));
                    forces[side].pieces[k] += pieces;
                    forces[side].attack[k] += pieces * attack(k);
                }
            }
        }
        forces
    }

    fn body_number(&self, exe: &GameExe, (s, p, m): (u8, u8, u8)) -> Option<usize> {
        if m == 0 {
            return Some(p.into());
        }
        let l = exe.star_systems().ok()?;
        l.get(usize::from(s).checked_sub(1)?)?.moons.get(usize::from(p).checked_sub(1)?)?.get(usize::from(m) - 1).map(|&b| b.into())
    }
}

impl GroundBattle {
    /// The forces divided into units (FUN_1430_0479) and lined up
    /// (FUN_1537_0200).
    pub fn new(state: &GameState, exe: &GameExe, place: (u8, u8, u8), you_attack: bool) -> Self {
        let tables = Tables::read(exe);
        let forces = state.ground_forces(exe, place);
        let mut battle = Self {
            place,
            you_attack,
            units: [Vec::new(), Vec::new()],
            reserve: [forces[0].pieces, forces[1].pieces],
            forces,
            rockets: [Vec::new(), Vec::new()],
            grid: [[0; ROWS]; COLUMNS],
            headquarters: if you_attack { [((0, 4), 1), ((14, 4), 2)] } else { [((0, 4), 2), ((15, 4), 1)] },
            over: false,
            tables,
        };
        for side in 0..2 {
            for kind in 1..=4u8 {
                while battle.units[side].len() < MAX_UNITS && battle.reserve[side][usize::from(kind) - 1] > 0 {
                    battle.add_unit(side, kind);
                }
            }
        }
        battle
    }

    /// A unit of `kind` from the reserve (FUN_1430_000f's ADD ... UNIT).
    pub fn add_unit(&mut self, side: usize, kind: u8) -> bool {
        let k = usize::from(kind) - 1;
        if self.units[side].len() >= MAX_UNITS || self.reserve[side][k] <= 0 {
            return false;
        }
        let pieces = self.reserve[side][k].min(self.tables.largest[usize::from(kind)]);
        self.reserve[side][k] -= pieces;
        self.units[side].push(Unit {
            kind,
            pieces: pieces as i32,
            column: 0,
            row: 0,
            reload: 0,
            facing: if side == 0 { 3 } else { 1 },
            moving: false,
            progress: 0,
            step_wait: 1,
            animation: 0,
            frames_left: 0,
            frame_wait: 0,
            order: if side == 0 { HOLD } else { ATTACK },
            target: 0,
            target_row: 0,
        });
        true
    }

    /// A unit back to the reserve (Remove unit).
    pub fn remove_unit(&mut self, side: usize, i: usize) {
        if i < self.units[side].len() {
            let u = self.units[side].remove(i);
            self.reserve[side][usize::from(u.kind) - 1] += i64::from(u.pieces);
        }
    }

    /// Changes a unit's size by `change` pieces, within the reserve and the
    /// largest unit of its kind (Add/sub).
    pub fn resize_unit(&mut self, side: usize, i: usize, change: i64) {
        let Some(u) = self.units[side].get_mut(i) else { return };
        let k = usize::from(u.kind) - 1;
        let largest = self.tables.largest[usize::from(u.kind)];
        let change = change.clamp(1 - i64::from(u.pieces), (largest - i64::from(u.pieces)).min(self.reserve[side][k]));
        u.pieces += change as i32;
        self.reserve[side][k] -= change;
    }

    /// FUN_1537_0200: the units take their places: yours from the third
    /// column, the enemy's from the fourteenth, nine to a column,
    /// centred. The enemy picks its targets.
    pub fn deploy(&mut self) {
        self.units[0].retain(|u| u.pieces > 0);
        self.units[1].retain(|u| u.pieces > 0);
        for side in 0..2 {
            let count = self.units[side].len();
            for (i, u) in self.units[side].iter_mut().enumerate() {
                let column = i / 9;
                let in_column = (count - column * 9).min(8) / 2;
                u.column = if side == 0 { column as u8 + 2 } else { 13 - column as u8 };
                u.row = (i % 9) as u8 + 4 - in_column as u8;
            }
        }
        for (side, ((c, r), size)) in self.headquarters.into_iter().enumerate() {
            for dc in 0..size {
                for dr in 0..size {
                    self.grid[usize::from(c + dc)][usize::from(r + dr)] = (side * 100 + HEADQUARTERS) as u8;
                }
            }
        }
        for i in 0..self.units[1].len() {
            self.units[1][i].target = self.nearest_enemy(1, i).map_or(0, |t| t as u8 + 1);
        }
    }

    fn id(side: usize, i: usize) -> u8 {
        (side * 100 + i + 1) as u8
    }

    fn cell(&self, column: i32, row: i32) -> Option<u8> {
        (0..COLUMNS as i32).contains(&column).then_some(())?;
        (0..ROWS as i32).contains(&row).then_some(())?;
        Some(self.grid[column as usize][row as usize])
    }

    /// Cell distance by the larger of the two offsets (FUN_1537_13c7).
    fn reach(&self, a: &Unit, b: &Unit) -> i32 {
        let (ax, ay) = a.position();
        let (bx, by) = b.position();
        (((ax - bx).abs().max((ay - by).abs()) - 3) >> 4) + 1
    }

    /// Steps (FUN_1537_12aa).
    fn steps(a: &Unit, b: &Unit) -> i32 {
        let (ax, ay) = a.position();
        let (bx, by) = b.position();
        (((ax - bx).abs() + (ay - by).abs() - 5) >> 4) + 1
    }

    /// Squared distance, in rocket range units (FUN_1537_14ff).
    fn far(a: &Unit, b: &Unit) -> i32 {
        let (ax, ay) = a.position();
        let (bx, by) = b.position();
        (((ax - bx).pow(2) + (ay - by).pow(2) - 1) >> 8) + 1
    }

    /// FUN_1537_16a3: whether `shooter` can hit `target`.
    fn in_range(&self, shooter: &Unit, target: &Unit) -> bool {
        match shooter.kind {
            1 => self.reach(shooter, target) < 2,
            2 => Self::steps(shooter, target) < 3,
            3 => self.reach(shooter, target) < 3,
            4 => self.reach(shooter, target) < 2 || (4..=16).contains(&Self::far(shooter, target)),
            _ => false,
        }
    }

    /// FUN_1537_1b80: the nearest living enemy of unit `i` of `side`.
    fn nearest_enemy(&self, side: usize, i: usize) -> Option<usize> {
        let me = self.units[side].get(i)?;
        self.units[1 - side]
            .iter()
            .enumerate()
            .filter(|(_, u)| u.pieces > 0)
            .min_by_key(|(_, u)| Self::far(u, me))
            .map(|(j, _)| j)
    }

    fn animate(&mut self, side: usize, i: usize, animation: u8) {
        let frames = self.tables.frames(animation).2;
        if let Some(u) = self.units[side].get_mut(i) {
            u.animation = animation;
            u.frames_left = frames;
            u.frame_wait = 3;
        }
    }

    /// Where the frames of animation `a` of a unit are (cell, row, sheet:
    /// false GRICON, true GRICON2), as FUN_1537_0da8 draws them.
    pub fn picture(&self, side: usize, u: &Unit) -> (u8, u8, bool) {
        if u.animation == 0 {
            return ((u.kind - 1) * 5 + 1, side as u8 + 1, false);
        }
        let (first, row, count) = self.tables.frames(u.animation);
        let mut cell = first + count - u.frames_left;
        let mut row = row;
        if side == 1 {
            if u.animation < 5 {
                row += 1;
            }
            if u.animation == 5 {
                cell += 9;
            }
        }
        (cell, row, matches!(u.animation, 7 | 8))
    }

    /// FUN_1537_34d4: the first step towards `to` around the others (a
    /// search over the free cells).
    fn way(&self, from: (u8, u8), to: (u8, u8), me: u8) -> u8 {
        let mut seen = [[false; ROWS]; COLUMNS];
        let mut first = [[0u8; ROWS]; COLUMNS];
        let mut queue = VecDeque::new();
        seen[usize::from(from.0)][usize::from(from.1)] = true;
        queue.push_back(from);
        while let Some((c, r)) = queue.pop_front() {
            if (c, r) == to {
                return first[usize::from(c)][usize::from(r)];
            }
            for facing in 1..=4u8 {
                let (dx, dy) = step(facing);
                let (nc, nr) = (i32::from(c) + dx, i32::from(r) + dy);
                let Some(occupant) = self.cell(nc, nr) else { continue };
                let (nc, nr) = (nc as usize, nr as usize);
                if seen[nc][nr] || (occupant != 0 && occupant != me && (nc as u8, nr as u8) != to) {
                    continue;
                }
                seen[nc][nr] = true;
                first[nc][nr] = if (c, r) == from { facing } else { first[usize::from(c)][usize::from(r)] };
                queue.push_back((nc as u8, nr as u8));
            }
        }
        0
    }

    /// Orders for one of your units.
    pub fn order_move(&mut self, i: usize, (column, row): (u8, u8)) {
        if let Some(u) = self.units[0].get_mut(i) {
            u.order = MOVE;
            u.target = column.min(COLUMNS as u8 - 1);
            u.target_row = row.min(ROWS as u8 - 1);
        }
    }

    pub fn order_attack(&mut self, i: usize, target: usize) {
        if let Some(u) = self.units[0].get_mut(i) {
            u.order = ATTACK;
            u.target = target as u8 + 1;
        }
    }

    /// The unit at a battlefield pixel (FUN_1537_18d3: the nearest one within
    /// about 22 pixels): side and index.
    pub fn unit_at(&self, (x, y): (i32, i32)) -> Option<(usize, usize)> {
        let mut best = None;
        let mut nearest = 501;
        for side in 0..2 {
            for (i, u) in self.units[side].iter().enumerate() {
                if u.pieces <= 0 {
                    continue;
                }
                let (ux, uy) = u.position();
                let d = (ux + 8 - x).pow(2) + (uy + 8 - y).pow(2);
                if d < nearest {
                    nearest = d;
                    best = Some((side, i));
                }
            }
        }
        best
    }

    /// One frame of the battle (FUN_1537_1c05). Returns the sound to play
    /// (the original's FUN_431a_05ca), 0 for none.
    pub fn frame(&mut self, cheat: bool, random: Random) -> u8 {
        if self.over {
            return 0;
        }
        let mut sound = 0;
        // Fire, both sides in a random order.
        let first = usize::from(random(2) == 1);
        for s in [first, 1 - first] {
            let enemy = 1 - s;
            for i in 0..self.units[s].len() {
                let me = self.units[s][i];
                if me.pieces <= 0 {
                    continue;
                }
                // A dead target: yours hold, the enemy's pick another.
                if me.order == ATTACK
                    && self.units[enemy].get(usize::from(me.target).wrapping_sub(1)).is_none_or(|t| t.pieces <= 0)
                {
                    if s == 0 {
                        self.units[s][i].order = HOLD;
                    } else {
                        self.units[s][i].target = self.nearest_enemy(s, i).map_or(0, |t| t as u8 + 1);
                    }
                }
                let me = self.units[s][i];
                if self.units[enemy].iter().all(|u| u.pieces <= 0) || me.animation != 0 || me.reload != 0 {
                    if self.units[s][i].reload > 0 {
                        self.units[s][i].reload -= 1;
                    }
                    continue;
                }
                let mut target = None;
                if me.order == ATTACK
                    && let Some(t) = self.units[enemy].get(usize::from(me.target).wrapping_sub(1))
                    && self.in_range(&me, t)
                {
                    target = Some(usize::from(me.target) - 1);
                }
                if target.is_none() {
                    for (j, t) in self.units[enemy].iter().enumerate() {
                        if t.pieces > 0 && self.in_range(&me, t) {
                            target = Some(j);
                        }
                    }
                }
                let Some(j) = target else {
                    if self.units[s][i].reload > 0 {
                        self.units[s][i].reload -= 1;
                    }
                    continue;
                };
                let victim = self.units[enemy][j];
                let attack = self.forces[s].per_piece(me.kind) * i64::from(me.pieces);
                if me.kind == 4 && self.reach(&me, &victim) >= 2 {
                    // A rocket.
                    sound = 5;
                    self.animate(s, i, 5);
                    let from = me.position();
                    let to = victim.position();
                    let frames = self.reach(&me, &victim) * 8;
                    let heading = heading((victim.column, victim.row), (me.column, me.row));
                    self.rockets[s].push(Rocket { from, to, frame: 1, frames, heading, damage: attack });
                } else {
                    let mut damage = (attack * self.tables.weakness[usize::from(victim.kind)] / 500).clamp(1, 15);
                    if cheat && s == 0 {
                        damage = i64::from(victim.pieces);
                    }
                    self.units[enemy][j].pieces -= damage as i32;
                    self.animate(s, i, me.kind);
                    if self.units[enemy][j].pieces < 1 {
                        self.units[enemy][j].pieces = 0;
                        sound = victim.kind + 5;
                        self.animate(enemy, j, 7);
                    } else {
                        sound = me.kind;
                        self.animate(enemy, j, 8);
                    }
                    if !me.moving {
                        self.units[s][i].facing = facing_towards((me.column, me.row), (victim.column, victim.row));
                    }
                }
                self.units[s][i].reload = self.tables.reload[usize::from(me.kind)];
            }
        }
        // Move.
        for s in 0..2 {
            for i in 0..self.units[s].len() {
                self.move_unit(s, i);
            }
        }
        // Rockets land.
        for s in 0..2 {
            let mut k = 0;
            while k < self.rockets[s].len() {
                let r = self.rockets[s][k];
                if r.frame < r.frames {
                    self.rockets[s][k].frame += 1;
                    k += 1;
                    continue;
                }
                self.rockets[s].remove(k);
                let (c, row) = ((r.to.0 + 8) / 16, (r.to.1 + 8) / 16);
                let occupant = self.cell(c, row).unwrap_or(0);
                let enemy = 1 - s;
                let hit = if enemy == 0 { (1..=20).contains(&occupant) } else { (101..=120).contains(&occupant) };
                if hit {
                    let j = usize::from(occupant % 100) - 1;
                    let Some(victim) = self.units[enemy].get(j).copied() else { continue };
                    let damage = (r.damage * self.tables.weakness[usize::from(victim.kind)] / 500).clamp(1, 15);
                    self.units[enemy][j].pieces -= damage as i32;
                    if self.units[enemy][j].pieces < 1 {
                        self.units[enemy][j].pieces = 0;
                        sound = victim.kind + 5;
                        self.animate(enemy, j, 7);
                    } else {
                        sound = 7;
                        self.animate(enemy, j, 8);
                    }
                }
            }
        }
        // Animations.
        for side in 0..2 {
            for u in &mut self.units[side] {
                if u.animation == 0 {
                    continue;
                }
                if u.frame_wait == 0 {
                    u.frames_left = u.frames_left.saturating_sub(1);
                    if u.frames_left == 0 {
                        u.animation = 0;
                    } else {
                        u.frame_wait = 1;
                    }
                } else {
                    u.frame_wait -= 1;
                }
            }
        }
        // The dead leave the grid.
        for c in 0..COLUMNS {
            for r in 0..ROWS {
                let id = self.grid[c][r];
                let side = usize::from(id >= 100);
                let i = usize::from(id % 100);
                if id != 0 && i <= MAX_UNITS && self.units[side].get(i - 1).is_none_or(|u| u.pieces <= 0) {
                    self.grid[c][r] = 0;
                }
            }
        }
        let gone = |units: &[Unit]| units.iter().all(|u| u.pieces <= 0);
        let busy = self.units.iter().flatten().any(|u| u.pieces <= 0 && u.animation != 0)
            || self.rockets.iter().any(|r| !r.is_empty());
        if (gone(&self.units[0]) || gone(&self.units[1]) || cheat) && !busy {
            self.over = true;
        }
        sound
    }

    /// The movement part of FUN_1537_1c05 for one unit.
    fn move_unit(&mut self, s: usize, i: usize) {
        let me = self.units[s][i];
        if me.pieces <= 0 {
            return;
        }
        if me.step_wait > 1 {
            self.units[s][i].step_wait -= 1;
            return;
        }
        let id = Self::id(s, i);
        let (dx, dy) = step(me.facing);
        let next = self.cell(i32::from(me.column) + dx, i32::from(me.row) + dy);
        let next_free = next.is_some_and(|o| o == 0 || o == id);
        let mut u = me;
        if !me.moving || next_free || me.order > HOLD {
            if next_free || !me.moving {
                u.progress += 1;
            }
            if u.progress >= 16 || !me.moving {
                // Arrived in a cell (or starting): leave the last one and
                // pick the next step.
                self.grid[usize::from(u.column)][usize::from(u.row)] = 0;
                if me.moving && next.is_some() {
                    self.grid[(i32::from(u.column) + dx) as usize][(i32::from(u.row) + dy) as usize] = 0;
                    u.column = (i32::from(u.column) + dx) as u8;
                    u.row = (i32::from(u.row) + dy) as u8;
                }
                u.moving = true;
                let mut way = 0;
                match u.order {
                    MOVE => {
                        way = self.way((u.column, u.row), (u.target, u.target_row), id);
                        if (u.column, u.row) == (u.target, u.target_row) {
                            u.order = HOLD;
                        }
                    }
                    ATTACK => {
                        if let Some(t) = self.units[1 - s].get(usize::from(u.target).wrapping_sub(1)).copied() {
                            way = self.way((u.column, u.row), (t.column, t.row), id);
                            if self.in_range(&u, &t) {
                                u.moving = false;
                            }
                        }
                    }
                    _ => {}
                }
                if way == 0 {
                    u.moving = false;
                } else {
                    u.facing = way;
                }
                let (dx, dy) = step(u.facing);
                let blocked = match self.cell(i32::from(u.column) + dx, i32::from(u.row) + dy) {
                    None => true,
                    Some(o) => o != 0 && o != id,
                };
                if blocked {
                    u.moving = false;
                }
                self.grid[usize::from(u.column)][usize::from(u.row)] = id;
                if u.moving {
                    self.grid[(i32::from(u.column) + dx) as usize][(i32::from(u.row) + dy) as usize] = id;
                }
                u.progress = 0;
            }
            u.step_wait = self.tables.step_wait[usize::from(u.kind)].max(1);
        }
        self.units[s][i] = u;
    }

    /// Whether you won: the enemy has no units left and you do.
    pub fn won(&self) -> bool {
        self.units[1].iter().all(|u| u.pieces <= 0) && self.units[0].iter().any(|u| u.pieces > 0)
    }

    /// FUN_1537_2a19: the survivors go back (the reserve with them); what's
    /// lost is lost from every group, fleet and planet in proportion.
    /// Returns the losses per kind (yours, the enemy's).
    pub fn finish(&self, state: &mut GameState, exe: &GameExe) -> [[i64; 4]; 2] {
        let mut left = self.reserve;
        for (left, units) in left.iter_mut().zip(&self.units) {
            for u in units {
                left[usize::from(u.kind) - 1] += i64::from(u.pieces.max(0));
            }
        }
        let mut losses = [[0i64; 4]; 2];
        for side in 0..2 {
            for k in 0..4 {
                losses[side][k] = self.forces[side].pieces[k] - left[side][k];
            }
        }
        let scale = |value: i64, side: usize, k: usize| {
            let total = self.forces[side].pieces[k];
            if total <= 0 { 0 } else { value * left[side][k] / total }
        };
        let (s, p, m) = self.place;
        for list in [UnitList::Groups, UnitList::Bases] {
            for n in 1..=state.unit_count(list) {
                let Some(g) = state.unit_mut(list, n) else { continue };
                if (g[unit::SYSTEM], g[unit::PLANET], g[unit::MOON]) != (s, p, m) || !matches!(g[unit::STATUS], 1 | 2 | 7) {
                    continue;
                }
                if g[unit::TYPE] != 1 && g[unit::TYPE] != 5 {
                    continue;
                }
                for k in 0..4 {
                    for w in 0..4 {
                        let at = 0x45 + 10 * k + 2 * w;
                        let v = i64::from(i16::from_le_bytes([g[at], g[at + 1]]));
                        g[at..at + 2].copy_from_slice(&(scale(v, 0, k) as i16).to_le_bytes());
                    }
                }
            }
        }
        for race in FIRST_RACE..=LAST_RACE {
            let standing = state.standing(race);
            if standing != ALLIED && standing != AT_WAR {
                continue;
            }
            let side = usize::from(standing == AT_WAR);
            for number in 1..=state.fleet_count(race) {
                let id = FleetId { race, number };
                if state.fleet_place(id) != (s, p, m) || state.fleet_byte(id, 2) != 1 {
                    continue;
                }
                for k in 0..4 {
                    let v = i64::from(state.fleet_forces(id, 5 + k as u16));
                    state.set_fleet_forces(id, 5 + k as u16, scale(v, side, k) as u16);
                }
            }
        }
        if let Some(body) = state.body_number(exe, (s, p, m))
            && let Some(owner) = state.body(s.into(), body).map(|r| r[0])
            && owner > 1
        {
            let standing = state.standing(owner);
            if standing == ALLIED || standing == AT_WAR {
                let side = usize::from(standing == AT_WAR);
                if let Some(r) = state.planet_mut(s.into(), body) {
                    for k in 0..4 {
                        let at = 0x2b + 4 * k;
                        let v = i64::from(i32::from_le_bytes([r[at], r[at + 1], r[at + 2], r[at + 3]]));
                        r[at..at + 4].copy_from_slice(&(scale(v, side, k) as i32).to_le_bytes());
                    }
                }
            }
        }
        losses
    }
}

/// FUN_1537_1725: the direction (1-4) from `from` towards `to`.
fn facing_towards(from: (u8, u8), to: (u8, u8)) -> u8 {
    let dx = i32::from(to.0) - i32::from(from.0);
    let dy = i32::from(to.1) - i32::from(from.1);
    if dy.abs() < dx.abs() {
        if dx < 1 { 1 } else { 3 }
    } else if dy < 1 {
        2
    } else {
        4
    }
}

/// FUN_1537_177e: a rocket's heading, 0-15, for its picture.
fn heading(target: (u8, u8), from: (u8, u8)) -> u8 {
    let (x1, y1) = (i32::from(target.0), i32::from(target.1));
    let (x2, y2) = (i32::from(from.0), i32::from(from.1));
    let side = facing_towards((target.0, target.1), (from.0, from.1));
    let mut a = (i32::from(side) - 1) * 4;
    let ady = (y1 - y2).abs().max(1);
    let adx = (x1 - x2).abs().max(1);
    a = match a {
        0 => 4 - ((x1 - x2) * 4 + ady * 5) / (ady * 2),
        4 => ((y1 - y2) * 4 + adx * 5) / (adx * 2) + 4,
        8 => ((x1 - x2) * 4 + ady * 5) / (ady * 2) + 8,
        _ => 16 - ((y1 - y2) * 4 + adx * 5) / (adx * 2),
    };
    let a = a + 2;
    (if a > 15 { a - 16 } else { a }).clamp(0, 15) as u8
}

/// How the game goes on after a ground war.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Outcome {
    Continue,
    /// New Earth fell (screen 35).
    Lost,
    /// The Earthlings' capital fell (screen 36).
    Won,
}

impl GameState {
    /// The main loop after END BATTLE on screen 32: what winning or losing
    /// a ground war at `place` does.
    pub fn ground_aftermath(
        &mut self,
        exe: &GameExe,
        texts: &crate::sim::Texts,
        place: (u8, u8, u8),
        won: bool,
        you_attack: bool,
        random: Random,
    ) -> (Vec<crate::story::Event>, Outcome) {
        use crate::story::Event;
        let message = |n: usize| Event::Message(texts.messages.get(n - 1).cloned().unwrap_or_default());
        let mut events = Vec::new();
        let mut outcome = Outcome::Continue;
        if !won && place == (1, 5, 0) {
            outcome = Outcome::Lost;
        }
        let body = self.body_number(exe, place);
        let record = body.and_then(|b| self.body(place.0.into(), b)).map(<[u8]>::to_vec);
        if won && let Some(r) = &record && r[1] == 1 && r[0] > 1 {
            // A race's capital.
            let race = r[0];
            events.push(Event::Message(format!(" Your forces have eliminated '{}' ", self.race_name(race))));
            if race == 3 {
                events.push(message(35));
                self.set_byte(0x5d8c, 1);
                self.set_byte(0x5d71, 0);
                self.set_byte(0x5d70, 1);
                self.make_known(19);
                self.start_invention_timer(19, 20, 50, random);
                if self.standing(4) as i8 > 0 {
                    self.set_standing(4, ALLIED);
                }
                for at in [0x4819, 0x481a, 0x481b] {
                    self.set_byte(at, 0);
                }
                self.set_byte(0x221, 2);
                events.push(Event::Scene(3));
                self.set_word(0x91ea, 5);
            }
            if race == 12 {
                outcome = Outcome::Won;
            }
            if self.byte(0x780c).unwrap_or(0) != 0 && self.word(0x6004) == Some(0) {
                events.push(message(13));
                self.make_known(12);
            }
            if (7..=10).contains(&race) {
                // The League surrenders when none of its races is left
                // standing against you.
                let beaten = (7..=10u8).all(|r| {
                    let at = 0x6a06 + 0xe4 * u16::from(r);
                    self.byte(at + 0x0f) == Some(0) || self.byte(at + 0x1b) == Some(ALLIED) || r == race
                });
                if beaten {
                    events.push(message(32));
                    events.push(Event::Scene(5));
                    self.set_word(0x91ea, 6);
                    self.set_byte(0x481c, 0);
                    self.give_invention(33);
                }
            }
            self.destroy_race_public(exe, race);
        }
        if won && self.byte(0x5d67) == Some(0) {
            self.set_byte(0x5d67, 1);
            self.start_invention_timer(13, 20, 20, random);
        }
        if !won && self.byte(0x5d68) == Some(0) && outcome == Outcome::Continue {
            self.set_byte(0x5d68, 1);
            events.push(message(14));
            self.start_invention_timer(17, 5, 5, random);
        }
        if won && self.byte(0x5d6d).unwrap_or(0) != 0 {
            self.set_byte(0x5d6d, 0);
            events.push(message(16));
            events.push(Event::Scene(3));
            self.set_word(0x91ea, 3);
        }
        if won && self.byte(0x5d75).unwrap_or(0) != 0 {
            self.set_byte(0x5d75, 0);
            events.push(message(16));
            self.set_byte(0x164, 2);
            self.set_byte(0x5d3c, 1);
            self.start_invention_timer(20, 10, 30, random);
            self.start_invention_timer(25, 100, 500, random);
            events.push(Event::Scene(3));
            self.set_word(0x91ea, 4);
        }
        if won && you_attack {
            self.conquer(exe, place, random);
        }
        if !won && !you_attack && outcome == Outcome::Continue {
            self.lose_colony_at(exe, place);
        }
        (events, outcome)
    }

    /// FUN_29b9_08f5: a conquered planet becomes your colony, with a command
    /// centre on the way and a base.
    fn conquer(&mut self, exe: &GameExe, (s, p, m): (u8, u8, u8), random: Random) {
        let Some(body) = self.body_number(exe, (s, p, m)) else { return };
        let settlers = i32::from(random(3000)) + 2000;
        if let Some(r) = self.planet_mut(s.into(), body) {
            r[0] = 1;
            r[1] = 0;
            r[4..6].fill(0);
            r[6] = 1;
            r[7] = 0;
            r[9] = 0;
            r[0x0a] = 0;
            r[0x0b] = 0;
            r[0x0d..0x11].copy_from_slice(&settlers.to_le_bytes());
            r[0x11] = 0x14;
            r[0x12] = 3;
            r[0x13] = 0x1e;
            r[0x1b..0x3b].fill(0);
        }
        let planet_type = self.body(s.into(), body).map_or(0, |r| r[0x15]);
        if let Ok(kind) = exe.building_type(crate::colony::COMMAND_CENTRE) {
            self.add_building(&kind, crate::colony::COMMAND_CENTRE, (s, p, m), (0xff, 0xff), planet_type, random);
        }
        if let Some(b) = self.add_base((s, p, m)) {
            let name = format!("{} forces", self.place_name(s, body));
            self.rename_unit(UnitList::Bases, b, &name);
        }
    }

    /// FUN_29b9_05b5: your colony there is lost.
    fn lose_colony_at(&mut self, exe: &GameExe, place: (u8, u8, u8)) {
        let Some(body) = self.body_number(exe, place) else { return };
        if let Some(r) = self.planet_mut(place.0.into(), body) {
            r[0] = 0;
            r[1] = 0;
            r[4..8].fill(0);
            r[0x0a] = 0;
            r[0x0b] = 0;
            r[0x0d..0x14].fill(0);
            r[0x1b..0x3b].fill(0);
        }
        while let Some(&n) = self.buildings_at(place).first() {
            self.remove_building(n);
        }
        while let Some(n) = self.base_at(place.0, place.1, place.2) {
            self.disband_unit(UnitList::Bases, n);
        }
    }
}
