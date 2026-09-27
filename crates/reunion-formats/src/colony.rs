//! Buildings and colonies.
//!
//! Building types: 63-byte records at DS:0x6553 + 0x3f * type (1-25). A
//! building is a 14-byte record in the list at 0xa2bc (count 0xa2c0).
//!
//! The colony model (FUN_357b_3cc0 and helpers, run whenever buildings
//! change): the planet's population staffs the finished, switched-on
//! buildings; generators make energy in proportion to their staffing and
//! condition, solar satellites add 10000 each; while the energy made is less
//! than half of what's used, the switched-on building with the highest
//! priority number is switched off. Then every building gets its share of
//! workers, energy and a working percentage, and the builder plants' output
//! is the planet's building capacity.

use crate::exe::{ExeError, GameExe};
use crate::state::GameState;

pub const BUILDING_TYPES: u8 = 25;
pub const COMMAND_CENTRE: u8 = 1;
pub const MINE: u8 = 4;
pub const DERRICK: u8 = 5;
pub const HOUSING: u8 = 7;
pub const STORAGE_BAY: u8 = 11;
pub const BUILDER_PLANT: u8 = 22;
pub const VEHICLE_PLANT: u8 = 23;
pub const MINER_STATION: u8 = 25;
/// Generators' categories.
const POWER_CATEGORIES: [u8; 2] = [1, 8];

#[derive(Debug, Clone, PartialEq)]
pub struct BuildingType {
    pub name: String,
    /// Invention needed (0: none, then the builders' level counts).
    pub invention: u8,
    pub builder_level: i8,
    /// 1 power, 2 mines, ...: picks the info lines (DS:0x183e + 3 * category).
    pub category: u8,
    pub needs_builder_plant: bool,
    pub needs_vehicle_plant: bool,
    /// Per planet type (1-11): non-zero where it may be built (also sets
    /// how sturdy it's built there).
    pub terrain: [u8; 12],
    pub workers: u16,
    pub energy: u32,
    pub production: u16,
    pub cost: u32,
    /// Switched off first when energy runs short: the highest number.
    pub priority: u8,
    /// Construction goes on in an hour when Random(this) < 10.
    pub build_time: u8,
    pub width: u8,
    pub height: u8,
    /// Tiles in the building sheet, rows of 4; None for empty cells.
    pub tiles: [Option<u8>; 16],
}

impl BuildingType {
    pub fn tile(&self, dx: u8, dy: u8) -> Option<u8> {
        if dx < self.width && dy < self.height {
            self.tiles[dy as usize * 4 + dx as usize]
        } else {
            None
        }
    }
}

impl GameExe {
    /// Building type `t` (1-25).
    pub fn building_type(&self, t: u8) -> Result<BuildingType, ExeError> {
        let base = 0x6553 + 0x3f * u16::from(t);
        let r = self.ds_bytes(base, 0x3f).ok_or(ExeError::OutOfRange)?;
        let word = |at: usize| u16::from_le_bytes([r[at], r[at + 1]]);
        let long = |at: usize| u32::from_le_bytes([r[at], r[at + 1], r[at + 2], r[at + 3]]);
        let len = (r[0] as usize).min(14);
        Ok(BuildingType {
            name: r[1..=len].iter().map(|&c| c as char).collect::<String>().trim_end().to_string(),
            invention: r[0x0f],
            builder_level: r[0x10] as i8,
            category: r[0x11],
            needs_builder_plant: r[0x12] != 0,
            needs_vehicle_plant: r[0x13] != 0,
            terrain: std::array::from_fn(|p| if p > 0 { r[0x13 + p] } else { 0 }),
            workers: word(0x1f),
            energy: long(0x21),
            production: word(0x25),
            cost: long(0x27),
            priority: r[0x2b],
            build_time: r[0x2c],
            width: r[0x2d],
            height: r[0x2e],
            tiles: std::array::from_fn(|i| Some(r[0x2f + i]).filter(|&v| v != 0xff)),
        })
    }

    /// Info lines (1-based rows, 0 none) of production, workers and energy
    /// for a building category (DS:0x183e + 3 * category).
    pub fn building_info_rows(&self, category: u8) -> [i8; 3] {
        self.ds_bytes(0x183e + 3 * u16::from(category), 3)
            .map_or([0; 3], |b| [b[0] as i8, b[1] as i8, b[2] as i8])
    }

    /// Whether terrain tile `tile` of terrain `terrain` can't be built on
    /// (DS:0xc92 + 0xf0 * terrain).
    pub fn tile_blocked(&self, terrain: u16, tile: u8) -> bool {
        let at = 0xc92 + 0xf0 * terrain + (u16::from(tile) / 20 + 1) * 20 + u16::from(tile) % 20;
        self.ds_bytes(at, 1).is_some_and(|b| b[0] != 0)
    }
}

/// Byte offsets in a building record.
pub mod building {
    pub const TYPE: usize = 0;
    pub const SYSTEM: usize = 1;
    pub const PLANET: usize = 2;
    pub const MOON: usize = 3;
    /// Map cell, 0xff both for "not placed yet".
    pub const X: usize = 4;
    pub const Y: usize = 5;
    /// Construction still to do (0: finished).
    pub const CONSTRUCTION: usize = 6;
    pub const CONDITION: usize = 7;
    /// Switched on.
    pub const ACTIVE: usize = 8;
    pub const WORKERS: usize = 9;
    pub const ENERGY: usize = 11;
    pub const WORKING: usize = 13;
}

const BUILDINGS: u16 = 0xa2bc;
const BUILDING_COUNT: u16 = 0xa2c0;
const BUILDING_LEN: usize = 14;
pub const MAX_BUILDINGS: usize = 1000;

impl GameState {
    fn building_count(&self) -> usize {
        (self.word(BUILDING_COUNT).unwrap_or(0) as usize).min(MAX_BUILDINGS)
    }

    /// Building record `n` (1-based), for editing.
    pub fn building_mut(&mut self, n: usize) -> Option<&mut [u8]> {
        if n == 0 || n > self.building_count() {
            return None;
        }
        self.block_mut(BUILDINGS)?
            .get_mut((n - 1) * BUILDING_LEN..n * BUILDING_LEN)
    }

    /// Numbers of the buildings at a planet or moon.
    pub fn buildings_at(&self, place: (u8, u8, u8)) -> Vec<usize> {
        self.buildings()
            .iter()
            .enumerate()
            .filter(|(_, b)| (b[1], b[2], b[3]) == place)
            .map(|(i, _)| i + 1)
            .collect()
    }

    /// FUN_2ef2_2463: a new building under construction (100-159 hours of
    /// work), placed at (x, y). None with 1000 buildings.
    pub fn add_building(
        &mut self,
        kind: &BuildingType,
        kind_number: u8,
        place: (u8, u8, u8),
        (x, y): (u8, u8),
        planet_type: u8,
        mut random: impl FnMut(u16) -> u16,
    ) -> Option<usize> {
        let n = self.building_count() + 1;
        let sturdiness = kind.terrain.get(planet_type as usize).copied().unwrap_or(0);
        if n > MAX_BUILDINGS || sturdiness == 0 {
            return None;
        }
        self.set_word(BUILDING_COUNT, n as u16);
        let construction = 100 + random(60) as u8;
        let condition = sturdiness.wrapping_mul(40).wrapping_add(random(80) as u8);
        let record = self.building_mut(n)?;
        record.fill(0);
        record[building::TYPE] = kind_number;
        (record[1], record[2], record[3]) = place;
        record[building::X] = x;
        record[building::Y] = y;
        record[building::CONSTRUCTION] = construction;
        record[building::CONDITION] = condition;
        Some(n)
    }

    /// FUN_2ef2_2ac5: removes building `n`; the ones after it move up.
    pub fn remove_building(&mut self, n: usize) {
        let count = self.building_count();
        if n == 0 || n > count {
            return;
        }
        if let Some(list) = self.block_mut(BUILDINGS) {
            list.copy_within(n * BUILDING_LEN..count * BUILDING_LEN, (n - 1) * BUILDING_LEN);
        }
        self.set_word(BUILDING_COUNT, (count - 1) as u16);
    }

    /// FUN_357b_3644's storage points of the finished buildings at a place:
    /// the planet stores 1000 of each ore per point.
    pub fn storage_points(&self, place: (u8, u8, u8)) -> u16 {
        self.buildings()
            .iter()
            .filter(|b| (b[1], b[2], b[3]) == place && b[building::CONSTRUCTION] == 0)
            .map(|b| match b[0] {
                STORAGE_BAY => 20,
                MINE | MINER_STATION => 10,
                DERRICK => 5,
                _ => 0,
            })
            .sum()
    }

    /// FUN_357b_3cc0 for the planet or moon record `body` of `place`.
    pub fn update_colony(&mut self, exe: &GameExe, place: (u8, u8, u8), body: usize) {
        let storage = self.storage_points(place);
        if let Some(base) = self.base_at(place.0, place.1, place.2)
            && let Some(record) = self.unit_mut(crate::state::UnitList::Bases, base)
        {
            record[0x9f..0xa1].copy_from_slice(&storage.to_le_bytes());
        }
        let Some(planet) = self.body(place.0 as usize, body).map(<[u8]>::to_vec) else {
            return;
        };
        let population = u64::from(u32::from_le_bytes([planet[0xd], planet[0xe], planet[0xf], planet[0x10]]));
        let solar = u64::from(planet[0xb]) * 10_000;
        let numbers = self.buildings_at(place);
        let kinds: Vec<Option<BuildingType>> = numbers
            .iter()
            .map(|&n| exe.building_type(self.buildings()[n - 1][0]).ok())
            .collect();
        // FUN_357b_3644: everything finished is switched on to begin with.
        for &n in &numbers {
            if let Some(b) = self.building_mut(n)
                && b[building::CONSTRUCTION] == 0
            {
                b[building::ACTIVE] = 1;
            }
        }
        let working = |state: &GameState| -> Vec<(usize, &BuildingType, Vec<u8>)> {
            numbers
                .iter()
                .zip(&kinds)
                .filter_map(|(&n, k)| {
                    let b = state.buildings()[n - 1].to_vec();
                    let k = k.as_ref()?;
                    (b[building::CONSTRUCTION] == 0 && b[building::ACTIVE] != 0).then_some((n, k, b))
                })
                .collect()
        };
        // FUN_357b_3746 / 3846.
        let balance = |state: &GameState| {
            let on = working(state);
            let staff = population.max(1);
            let needed = on.iter().map(|(_, k, _)| u64::from(k.workers)).sum::<u64>().max(staff);
            let made = on
                .iter()
                .filter(|(_, k, _)| POWER_CATEGORIES.contains(&k.category))
                .map(|(_, k, b)| u64::from(k.production) * staff / needed * u64::from(b[building::CONDITION]) / 100)
                .sum::<u64>()
                + solar;
            let made = made.max(1);
            let used = on
                .iter()
                .map(|(_, k, _)| u64::from(k.energy) * staff / needed)
                .sum::<u64>()
                .max(made);
            (staff, needed, made, used)
        };
        let (mut staff, mut needed, mut made, mut used) = balance(self);
        // FUN_357b_3a05: switch off by priority while short of energy.
        while 2 * made < used {
            // The last of the highest priority numbers.
            let Some((n, _, _)) = working(self)
                .into_iter()
                .max_by_key(|(_, k, _)| k.priority)
            else {
                break;
            };
            if let Some(b) = self.building_mut(n) {
                b[building::ACTIVE] = 0;
            }
            (staff, needed, made, used) = balance(self);
        }
        // FUN_357b_3ae5.
        let mut builders = 0u64;
        for (n, k, _) in working(self) {
            let Some(b) = self.building_mut(n) else { continue };
            let workers = (u64::from(k.workers) * staff / needed) as u16;
            let energy = (u64::from(k.energy) * made / used) as u16;
            let percent = if POWER_CATEGORIES.contains(&k.category) {
                staff * 100 / needed
            } else {
                made * 100 / used * staff / needed
            } as u8;
            b[building::WORKERS..building::WORKERS + 2].copy_from_slice(&workers.to_le_bytes());
            b[building::ENERGY..building::ENERGY + 2].copy_from_slice(&energy.to_le_bytes());
            b[building::WORKING] = percent;
            if b[building::TYPE] == BUILDER_PLANT {
                builders += u64::from(b[building::CONDITION]) * u64::from(k.production) / 100
                    * u64::from(percent)
                    / 100;
            }
        }
        if let Some(record) = self.planet_mut(place.0 as usize, body) {
            record[4..6].copy_from_slice(&(builders as u16).to_le_bytes());
        }
    }
}
