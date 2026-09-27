//! The complete game state, in the original save format.
//!
//! REUNION.PRG FUN_1a3e_029c saves and loads a game as a 20-byte name followed
//! by 135 blocks of the data segment (and three heap buffers), always in the
//! same order. [`GameState`] keeps those blocks as-is, so saves round-trip
//! byte for byte, and exposes the parts identified so far as typed views.
//! A new game is the executable's initial data plus `SAVE/INIT`.

use std::collections::HashMap;

use thiserror::Error;

use crate::exe::GameExe;

/// Length of the save name at the start of a save file.
pub const SAVE_NAME_LEN: usize = 20;
/// 20 + the sum of all block sizes.
pub const SAVE_LEN: usize = SAVE_NAME_LEN + 41_650;

/// Where a saved block lives in the original: a data segment address, or a
/// heap buffer found through the far pointer stored at that address.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Block {
    Ds(u16, usize),
    Far(u16, usize),
}

impl Block {
    pub fn address(self) -> u16 {
        match self {
            Block::Ds(a, _) | Block::Far(a, _) => a,
        }
    }

    pub fn size(self) -> usize {
        match self {
            Block::Ds(_, n) | Block::Far(_, n) => n,
        }
    }
}

/// Blocks in save file order, from FUN_1a3e_029c.
pub const BLOCKS: [Block; 135] = [
    Block::Ds(0x57cc, 24),
    Block::Ds(0x57f0, 12),
    Block::Far(0x7ade, 795),
    Block::Ds(0x7ae2, 2),
    Block::Ds(0x013a, 270),
    Block::Ds(0x0424, 240),
    Block::Ds(0x6518, 12),
    Block::Ds(0x773e, 1),
    Block::Ds(0x773f, 1),
    Block::Ds(0x7740, 1),
    Block::Ds(0x7741, 1),
    Block::Ds(0x7742, 1),
    Block::Ds(0x7743, 1),
    Block::Ds(0x7744, 1),
    Block::Ds(0x7745, 1),
    Block::Ds(0x7746, 1),
    Block::Ds(0x7747, 1),
    Block::Ds(0x7748, 1),
    Block::Ds(0x7749, 1),
    Block::Ds(0x1a9c, 416),
    Block::Ds(0x1c3c, 2080),
    Block::Ds(0x248a, 156),
    Block::Ds(0x2526, 780),
    Block::Ds(0x2860, 234),
    Block::Ds(0x294a, 1170),
    Block::Ds(0x2df8, 91),
    Block::Ds(0x2e54, 455),
    Block::Ds(0x3038, 169),
    Block::Ds(0x30e2, 845),
    Block::Ds(0x3470, 299),
    Block::Ds(0x359c, 1495),
    Block::Ds(0x3b86, 65),
    Block::Ds(0x3bc8, 325),
    Block::Ds(0x3d56, 416),
    Block::Ds(0x3ef6, 2080),
    Block::Ds(0x47d6, 64),
    Block::Ds(0x4816, 8),
    Block::Ds(0x481e, 16),
    Block::Ds(0x482e, 2),
    Block::Ds(0x7ae4, 2),
    Block::Ds(0x7ae6, 16),
    Block::Ds(0x7af6, 16),
    Block::Ds(0x9276, 2),
    Block::Ds(0x9278, 2),
    Block::Ds(0x5dac, 1855),
    Block::Ds(0x95a0, 2),
    Block::Ds(0x95a2, 2),
    Block::Ds(0x95a4, 8),
    Block::Ds(0x95ac, 8),
    Block::Ds(0x95b4, 2),
    Block::Ds(0x95b6, 8),
    Block::Ds(0x95be, 4),
    Block::Ds(0x95c6, 24),
    Block::Ds(0x95de, 2),
    Block::Ds(0x95e0, 2),
    Block::Ds(0x95e2, 2),
    Block::Ds(0x95e4, 2),
    Block::Ds(0xa21a, 2),
    Block::Ds(0xa21c, 160),
    Block::Ds(0xa2c0, 2),
    Block::Far(0xa2bc, 14000),
    Block::Ds(0x6bce, 2508),
    Block::Ds(0xa2c2, 2),
    Block::Ds(0xa2c4, 2),
    Block::Ds(0xa2c6, 2),
    Block::Ds(0xa2c8, 2),
    Block::Ds(0xa2ca, 2),
    Block::Ds(0xa2cc, 2),
    Block::Ds(0xa2ce, 2),
    Block::Far(0xa2d0, 10304),
    Block::Ds(0xa2d4, 1),
    Block::Ds(0xa2d6, 4),
    Block::Ds(0x91a0, 70),
    Block::Ds(0x91ea, 2),
    Block::Ds(0x5d3c, 1),
    Block::Ds(0x5d3d, 1),
    Block::Ds(0x5d3e, 1),
    Block::Ds(0x5d3f, 1),
    Block::Ds(0x5d40, 2),
    Block::Ds(0x5d42, 2),
    Block::Ds(0x5d44, 2),
    Block::Ds(0x5d46, 2),
    Block::Ds(0x5d48, 1),
    Block::Ds(0x5d4a, 2),
    Block::Ds(0x5d4c, 1),
    Block::Ds(0x5d4e, 2),
    Block::Ds(0x5d50, 1),
    Block::Ds(0x5d52, 2),
    Block::Ds(0x5d54, 2),
    Block::Ds(0x5d56, 1),
    Block::Ds(0x5d58, 2),
    Block::Ds(0x5d5a, 1),
    Block::Ds(0x5d5c, 2),
    Block::Ds(0x5d5e, 1),
    Block::Ds(0x5d60, 2),
    Block::Ds(0x5d62, 1),
    Block::Ds(0x5d64, 2),
    Block::Ds(0x5d66, 1),
    Block::Ds(0x5d67, 1),
    Block::Ds(0x5d68, 1),
    Block::Ds(0x5d6a, 2),
    Block::Ds(0x5d6c, 1),
    Block::Ds(0x5d6d, 1),
    Block::Ds(0x5d6e, 2),
    Block::Ds(0x5d70, 1),
    Block::Ds(0x5d71, 1),
    Block::Ds(0x5d72, 2),
    Block::Ds(0x5d74, 1),
    Block::Ds(0x5d75, 1),
    Block::Ds(0x5d76, 2),
    Block::Ds(0x5d78, 1),
    Block::Ds(0x5d7a, 2),
    Block::Ds(0x5d7c, 1),
    Block::Ds(0x5d7e, 2),
    Block::Ds(0x5d80, 1),
    Block::Ds(0x5d82, 2),
    Block::Ds(0x5d84, 1),
    Block::Ds(0x5d86, 2),
    Block::Ds(0x5d88, 1),
    Block::Ds(0x5d8a, 2),
    Block::Ds(0x5d8c, 1),
    Block::Ds(0x5d8e, 2),
    Block::Ds(0x5d90, 1),
    Block::Ds(0x5d91, 1),
    Block::Ds(0x5d92, 2),
    Block::Ds(0x5d94, 1),
    Block::Ds(0x5d95, 1),
    Block::Ds(0x5d96, 2),
    Block::Ds(0x5d98, 1),
    Block::Ds(0x5d99, 1),
    Block::Ds(0x5d9a, 1),
    Block::Ds(0x5d9c, 2),
    Block::Ds(0x5d9e, 2),
    Block::Ds(0x5da0, 2),
    Block::Ds(0x5da2, 4),
];

/// Building buffer (up to 1000 records of 14 bytes) and its count; SAVE/INIT
/// holds the count followed by the whole buffer.
const BUILDINGS: u16 = 0xa2bc;
const BUILDING_COUNT: u16 = 0xa2c0;
const BUILDING_LEN: usize = 14;

/// Star systems: a table of 13-byte names (Pascal strings) and a table of
/// 65-byte records, one per planet or moon. System names aren't stored here.
const STAR_SYSTEMS: [(u16, u16, usize); 8] = [
    (0x1a9c, 0x1c3c, 32),
    (0x248a, 0x2526, 12),
    (0x2860, 0x294a, 18),
    (0x2df8, 0x2e54, 7),
    (0x3038, 0x30e2, 13),
    (0x3470, 0x359c, 23),
    (0x3b86, 0x3bc8, 5),
    (0x3d56, 0x3ef6, 32),
];
const BODY_NAME_LEN: usize = 13;
pub const BODY_LEN: usize = 65;

/// Values `entry` sets when a new game starts (REUNION.PRG, right after the
/// hero is chosen). Only the ones that differ from zero; the rest of that
/// memory is uninitialized and already zero here.
const NEW_GAME_WORDS: [(u16, u16); 10] = [
    (0x7ae4, 1),
    (0x91ea, 1),
    (0x95b4, 10),
    // Money: 120000 as a 32-bit value.
    (0x95be, 0xd4c0),
    (0x95c0, 0x0001),
    // Date: year 2927, month 8, day 13, hour 23.
    (0x95de, 2927),
    (0x95e0, 8),
    (0x95e2, 13),
    (0x95e4, 23),
    (0x5d58, 6000), // plus Random(400), see new_game
];
/// Year, month, day, hour.
const DATE: [u16; 4] = [0x95de, 0x95e0, 0x95e2, 0x95e4];

/// 35 words set to 0xffff at the start of a new game.
const NEW_GAME_FILL: (u16, usize, u16) = (0x91a0, 35, 0xffff);

/// Message log (FUN_34b0_0001): a heap buffer of 15 records of 53 bytes (a
/// u16 kind, then the text as a Pascal string of up to 50 characters) and the
/// number in use.
const MESSAGES: u16 = 0x7ade;
const MESSAGE_COUNT: u16 = 0x7ae2;
const MESSAGE_LEN: usize = 0x35;
pub const MAX_MESSAGES: usize = 15;
const MESSAGE_TEXT_LEN: usize = 50;

/// Units (FUN_2b8d_16d0): the heap buffer at 0xa2d0 holds two lists of 32
/// records of 161 bytes, groups (ships and troops) then planet bases, with
/// their counts at 0xa2c4 and 0xa2c6.
const UNITS: u16 = 0xa2d0;
const UNIT_COUNTS: [u16; 2] = [0xa2c4, 0xa2c6];
pub const UNIT_LEN: usize = 0xa1;
pub const MAX_UNITS: usize = 32;
const UNIT_NAME_CAPACITY: usize = 17;

/// Byte offsets in a unit record.
pub mod unit {
    /// 1 army group, 2 trade company, 3 secret forces, 4 satellite carrier,
    /// 5 planet base.
    pub const TYPE: usize = 0x00;
    /// Pascal string, up to 17 characters.
    pub const NAME: usize = 0x01;
    pub const SYSTEM: usize = 0x13;
    pub const PLANET: usize = 0x14;
    pub const MOON: usize = 0x15;
    /// 1-2 on a planet or in orbit, 4-6 travelling, 7 a base.
    pub const STATUS: usize = 0x16;
    /// Travel: u16 distance parts at 0x17 and 0x1b, u16 days at 0x19.
    pub const TRAVEL_A: usize = 0x17;
    pub const TRAVEL_DAYS: usize = 0x19;
    pub const TRAVEL_B: usize = 0x1b;
    /// Two slots of four unit kinds, 10 bytes each (a u16 number, then
    /// per-kind values), from here and 0x45.
    pub const SLOTS: usize = 0x1d;
    pub const SLOT_LEN: usize = 0x28;
    pub const KIND_LEN: usize = 10;
}

/// What happened to a group in an hour of travel.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TravelEvent {
    /// The group reached its destination (now in orbit).
    Arrived(usize),
    /// ... which was an unexplored system's star: the system is known now
    /// and its planets are on the map.
    Explored(usize, u8),
}

/// The two unit lists.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum UnitList {
    Groups = 0,
    Bases = 1,
}

const INVENTIONS: (u16, usize) = (0x5dac, 53);
const RACES: (u16, usize) = (0x6bce, 228);
const CHARACTERS: (u16, usize) = (0x013a, 27);

#[derive(Debug, Error)]
pub enum StateError {
    #[error("save file is {0} bytes, expected {SAVE_LEN}")]
    SaveLength(usize),
    #[error("SAVE/INIT is {0} bytes, expected 14002")]
    InitLength(usize),
}

#[derive(Clone, PartialEq)]
pub struct GameState {
    pub name: [u8; SAVE_NAME_LEN],
    blocks: HashMap<u16, Vec<u8>>,
}

/// A planet or moon.
pub struct Body<'a> {
    pub name: String,
    pub data: &'a [u8],
}

pub struct StarSystem<'a> {
    pub bodies: Vec<Body<'a>>,
}

/// How a body circles its star or planet on the galactic map.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Orbit {
    /// Tenths of a degree per step.
    pub speed: u8,
    pub radius_x: u8,
    pub radius_y: u8,
}

/// A fixed-size record whose first field is a Pascal string name.
pub struct Named<'a> {
    pub name: String,
    pub data: &'a [u8],
}

impl GameState {
    /// A new game: every block starts with the executable's initial data (or
    /// zeros for uninitialized memory), `entry`'s new-game values are applied,
    /// and the buildings come from SAVE/INIT. `seed` feeds the game's one
    /// random start value.
    ///
    /// Not yet covered: the heap buffers at 0x7ade and 0xa2d0.
    pub fn new_game(exe: &GameExe, init: &[u8], seed: u32) -> Result<Self, StateError> {
        if init.len() != 2 + 14_000 {
            return Err(StateError::InitLength(init.len()));
        }
        let mut blocks = HashMap::new();
        for block in BLOCKS {
            let bytes = match block {
                Block::Ds(address, len) => exe
                    .ds_bytes(address, len)
                    .map_or_else(|| vec![0; len], <[u8]>::to_vec),
                Block::Far(_, len) => vec![0; len],
            };
            blocks.insert(block.address(), bytes);
        }
        blocks.insert(BUILDING_COUNT, init[..2].to_vec());
        blocks.insert(BUILDINGS, init[2..].to_vec());
        let mut state = Self {
            name: [0; SAVE_NAME_LEN],
            blocks,
        };
        for (address, value) in NEW_GAME_WORDS {
            state.set_word(address, value);
        }
        let (fill, count, value) = NEW_GAME_FILL;
        for i in 0..count as u16 {
            state.set_word(fill + i * 2, value);
        }
        let mut random = BorlandRandom(seed);
        state.set_word(0x5d58, 6000 + random.below(400));
        // FUN_2b8d_1d09: no groups, and the New Earth base (FUN_2b8d_17fd).
        state.set_word(UNIT_COUNTS[0], 0);
        state.set_word(UNIT_COUNTS[1], 1);
        if let Some(base) = state.unit_mut(UnitList::Bases, 1) {
            init_unit(base, 5, exe.ds_string(0x595d + 5 * 0x11).as_deref().unwrap_or(""));
            base[unit::STATUS] = 7;
        }
        Ok(state)
    }

    /// A 16-bit value anywhere in the saved part of the data segment.
    pub fn word(&self, address: u16) -> Option<u16> {
        let (block, offset) = self.locate(address)?;
        let bytes = self.blocks[&block].get(offset..offset + 2)?;
        Some(u16::from_le_bytes([bytes[0], bytes[1]]))
    }

    /// An 8-bit value anywhere in the saved part of the data segment.
    pub fn byte(&self, address: u16) -> Option<u8> {
        let (block, offset) = self.locate(address)?;
        self.blocks[&block].get(offset).copied()
    }

    pub fn set_byte(&mut self, address: u16, value: u8) {
        if let Some((block, offset)) = self.locate(address)
            && let Some(byte) = self
                .blocks
                .get_mut(&block)
                .and_then(|b| b.get_mut(offset))
        {
            *byte = value;
        }
    }

    pub fn set_word(&mut self, address: u16, value: u16) {
        if let Some((block, offset)) = self.locate(address)
            && let Some(bytes) = self
                .blocks
                .get_mut(&block)
                .and_then(|b| b.get_mut(offset..offset + 2))
        {
            bytes.copy_from_slice(&value.to_le_bytes());
        }
    }

    /// The data segment block holding `address`, and the offset into it.
    fn locate(&self, address: u16) -> Option<(u16, usize)> {
        BLOCKS.iter().find_map(|block| match *block {
            Block::Ds(start, size)
                if (start..start.saturating_add(size as u16)).contains(&address) =>
            {
                Some((start, (address - start) as usize))
            }
            _ => None,
        })
    }

    pub fn money(&self) -> u32 {
        let low = self.word(0x95be).unwrap_or(0) as u32;
        let high = self.word(0x95c0).unwrap_or(0) as u32;
        high << 16 | low
    }

    /// Year, month, day and hour, as shown in the main screen's text strip.
    pub fn date(&self) -> [u16; 4] {
        DATE.map(|a| self.word(a).unwrap_or(0))
    }

    /// One hour passes, as FUN_1b8a_0000 counts it: 24 hours, 30 days in
    /// every month, 12 months. The day is checked every hour, not only at
    /// midnight, exactly like the original.
    pub fn advance_hour(&mut self) {
        let [mut year, mut month, mut day, mut hour] = self.date();
        hour += 1;
        if hour > 23 {
            day += 1;
            hour = 0;
        }
        if day > 30 {
            month += 1;
            day = 1;
        }
        if month > 12 {
            year += 1;
            month = 1;
        }
        for (address, value) in DATE.into_iter().zip([year, month, day, hour]) {
            self.set_word(address, value);
        }
    }

    pub fn from_save(data: &[u8]) -> Result<Self, StateError> {
        if data.len() != SAVE_LEN {
            return Err(StateError::SaveLength(data.len()));
        }
        let mut name = [0; SAVE_NAME_LEN];
        name.copy_from_slice(&data[..SAVE_NAME_LEN]);
        let mut blocks = HashMap::new();
        let mut pos = SAVE_NAME_LEN;
        for block in BLOCKS {
            blocks.insert(block.address(), data[pos..pos + block.size()].to_vec());
            pos += block.size();
        }
        Ok(Self { name, blocks })
    }

    pub fn to_save(&self) -> Vec<u8> {
        let mut out = Vec::with_capacity(SAVE_LEN);
        out.extend_from_slice(&self.name);
        for block in BLOCKS {
            out.extend_from_slice(&self.blocks[&block.address()]);
        }
        out
    }

    /// Raw bytes of a block, by its address in the original.
    pub fn block(&self, address: u16) -> Option<&[u8]> {
        self.blocks.get(&address).map(Vec::as_slice)
    }

    pub fn block_mut(&mut self, address: u16) -> Option<&mut [u8]> {
        self.blocks.get_mut(&address).map(Vec::as_mut_slice)
    }

    pub fn star_systems(&self) -> Vec<StarSystem<'_>> {
        STAR_SYSTEMS
            .iter()
            .map(|&(names, data, count)| {
                let names = &self.blocks[&names];
                let data = &self.blocks[&data];
                StarSystem {
                    bodies: (0..count)
                        .map(|i| Body {
                            name: pascal(&names[i * BODY_NAME_LEN..]),
                            data: &data[i * BODY_LEN..(i + 1) * BODY_LEN],
                        })
                        .collect(),
                }
            })
            .collect()
    }

    /// The 65-byte record of a body (1-based system and body number).
    pub fn body(&self, system: usize, body: usize) -> Option<&[u8]> {
        let &(_, data, count) = STAR_SYSTEMS.get(system.checked_sub(1)?)?;
        if body == 0 || body > count {
            return None;
        }
        let start = (body - 1) * BODY_LEN;
        self.blocks.get(&data)?.get(start..start + BODY_LEN)
    }

    /// Whether a star system (1-based) has been discovered (DS:0x4815 + system).
    pub fn system_known(&self, system: usize) -> bool {
        self.byte(0x4815 + system as u16) == Some(1)
    }

    /// A body's orbit on the galactic map, from bytes 10-12 of its name
    /// record (FUN_357b_23db).
    pub fn orbit(&self, system: usize, body: usize) -> Option<Orbit> {
        let &(names, _, count) = STAR_SYSTEMS.get(system.checked_sub(1)?)?;
        if body == 0 || body > count {
            return None;
        }
        let record = self
            .blocks
            .get(&names)?
            .get((body - 1) * BODY_NAME_LEN..body * BODY_NAME_LEN)?;
        Some(Orbit {
            speed: record[10],
            radius_x: record[11],
            radius_y: record[12],
        })
    }

    /// What the galactic map shows of a planet (DS:0x47cd + 8 * system +
    /// planet): negative hides it, 0 shows it with its moons unknown, 1 or
    /// more shows its moons too.
    pub fn planet_visibility(&self, system: usize, planet: usize) -> i8 {
        self.byte(0x47cd + 8 * system as u16 + planet as u16)
            .map_or(-1, |b| b as i8)
    }

    /// The 65-byte record of a planet (1-based system and planet), for editing.
    pub fn planet_mut(&mut self, system: usize, planet: usize) -> Option<&mut [u8]> {
        let &(_, data, count) = STAR_SYSTEMS.get(system.checked_sub(1)?)?;
        if planet == 0 || planet > count {
            return None;
        }
        let start = (planet - 1) * BODY_LEN;
        self.blocks.get_mut(&data)?.get_mut(start..start + BODY_LEN)
    }

    /// Number of units in a list.
    pub fn unit_count(&self, list: UnitList) -> usize {
        (self.word(UNIT_COUNTS[list as usize]).unwrap_or(0) as usize).min(MAX_UNITS)
    }

    /// A unit's record (1-based), if it's in use.
    pub fn unit(&self, list: UnitList, n: usize) -> Option<&[u8]> {
        if n == 0 || n > self.unit_count(list) {
            return None;
        }
        let start = (list as usize * MAX_UNITS + n - 1) * UNIT_LEN;
        self.blocks.get(&UNITS)?.get(start..start + UNIT_LEN)
    }

    pub fn unit_mut(&mut self, list: UnitList, n: usize) -> Option<&mut [u8]> {
        if n == 0 || n > self.unit_count(list) {
            return None;
        }
        let start = (list as usize * MAX_UNITS + n - 1) * UNIT_LEN;
        self.blocks.get_mut(&UNITS)?.get_mut(start..start + UNIT_LEN)
    }

    /// NEW UNIT (FUN_26fe_001b, FUN_2b8d_16d0): a new army group called "New
    /// Group" at New Earth. Returns its number, or None with 32 groups.
    pub fn add_group(&mut self) -> Option<usize> {
        let n = self.unit_count(UnitList::Groups) + 1;
        if n > MAX_UNITS {
            return None;
        }
        self.set_word(UNIT_COUNTS[0], n as u16);
        init_unit(self.unit_mut(UnitList::Groups, n)?, 1, "New Group");
        Some(n)
    }

    /// A new base at a planet or moon (FUN_2841_0060 founding a colony), made
    /// the selected unit. None with 32 bases.
    pub fn add_base(&mut self, (system, planet, moon): (u8, u8, u8)) -> Option<usize> {
        let n = self.unit_count(UnitList::Bases) + 1;
        if n > MAX_UNITS {
            return None;
        }
        self.set_word(UNIT_COUNTS[1], n as u16);
        let base = self.unit_mut(UnitList::Bases, n)?;
        init_unit(base, 5, "");
        base[unit::SYSTEM] = system;
        base[unit::PLANET] = planet;
        base[unit::MOON] = moon;
        base[unit::STATUS] = 7;
        self.set_word(0xa2ce, 1);
        for selection in [0xa2c2, 0xa2c8, 0xa2cc] {
            self.set_word(selection, n as u16);
        }
        Some(n)
    }

    /// Drops the last group (ABORT while creating it).
    pub fn remove_last_group(&mut self) {
        let n = self.unit_count(UnitList::Groups);
        self.set_word(UNIT_COUNTS[0], n.saturating_sub(1) as u16);
    }

    /// The base at a planet or moon (FUN_2b8d_1c9f).
    pub fn base_at(&self, system: u8, planet: u8, moon: u8) -> Option<usize> {
        (1..=self.unit_count(UnitList::Bases)).rev().find(|&n| {
            self.unit(UnitList::Bases, n).is_some_and(|b| {
                b[unit::SYSTEM] == system && b[unit::PLANET] == planet && b[unit::MOON] == moon
            })
        })
    }

    /// DISBAND UNIT (FUN_29b9_0b93): a group's ships and troops (army groups;
    /// the ships of secret forces) go to the base where it is, and the units
    /// after it move up. Clears the selection.
    pub fn disband_unit(&mut self, list: UnitList, n: usize) {
        let count = self.unit_count(list);
        if n == 0 || n > count {
            return;
        }
        if list == UnitList::Groups
            && let Some(group) = self.unit(list, n).map(<[u8]>::to_vec)
            && let Some(base) =
                self.base_at(group[unit::SYSTEM], group[unit::PLANET], group[unit::MOON])
            && let Some(base) = self.unit_mut(UnitList::Bases, base)
        {
            for slot in 1..=2 {
                if group[unit::TYPE] != 1 && !(group[unit::TYPE] == 3 && slot == 1) {
                    continue;
                }
                for kind in 0..4 {
                    for value in 0..5 {
                        let at = unit::SLOTS
                            + (slot - 1) * unit::SLOT_LEN
                            + kind * unit::KIND_LEN
                            + 2 * value;
                        let sum = u16::from_le_bytes([base[at], base[at + 1]])
                            .wrapping_add(u16::from_le_bytes([group[at], group[at + 1]]));
                        base[at..at + 2].copy_from_slice(&sum.to_le_bytes());
                    }
                }
            }
        }
        if let Some(units) = self.blocks.get_mut(&UNITS) {
            let first = list as usize * MAX_UNITS;
            let start = (first + n - 1) * UNIT_LEN;
            let end = (first + count) * UNIT_LEN;
            units.copy_within(start + UNIT_LEN..end, start);
        }
        self.set_word(UNIT_COUNTS[list as usize], (count - 1) as u16);
        if self.word(0xa2ce) == Some(list as u16) {
            let shown = self.word(0xa2c2).unwrap_or(1);
            self.set_word(0xa2c2, shown.saturating_sub(1));
        }
        for selection in [0xa2c8, 0xa2ca, 0xa2cc] {
            self.set_word(selection, 0);
        }
    }

    /// Sends a group on its way (FUN_29b9_0e66): its location becomes the
    /// destination at once; the trip is two distances and days in between,
    /// counted down each hour. `random(n)` is the game's Random(n).
    pub fn start_travel(
        &mut self,
        n: usize,
        (system, planet, moon): (u8, u8, u8),
        mut random: impl FnMut(u16) -> u16,
    ) {
        let Some(group) = self.unit_mut(UnitList::Groups, n) else {
            return;
        };
        let here = (group[unit::SYSTEM], group[unit::PLANET], group[unit::MOON]);
        if here == (system, planet, moon) {
            return;
        }
        let (a, days, b) = if here.0 != system {
            (200 + random(50), 1, 200 + random(50))
        } else if here.1 == planet {
            (0, 0, 80 + random(20))
        } else {
            (0, 0, 150 + random(50))
        };
        let words = [(unit::TRAVEL_A, a), (unit::TRAVEL_DAYS, days), (unit::TRAVEL_B, b)];
        for (at, value) in words {
            group[at..at + 2].copy_from_slice(&value.to_le_bytes());
        }
        group[unit::STATUS] = if here.0 != system { 6 } else { 4 };
        group[unit::SYSTEM] = system;
        group[unit::PLANET] = planet;
        group[unit::MOON] = moon;
    }

    /// An hour of travel for every moving group (FUN_1b8a_2a0e): the first
    /// distance and the last go down by the pilots' level + 1, the days in
    /// between by one; status 6, 5, 4 on the way, 2 (in orbit) on arrival.
    pub fn travel_hour(&mut self) -> Vec<TravelEvent> {
        let speed = self.word(0x95a4).unwrap_or(0) as i16 + 1;
        let mut events = Vec::new();
        for n in 1..=self.unit_count(UnitList::Groups) {
            let Some(group) = self.unit_mut(UnitList::Groups, n) else {
                continue;
            };
            if !(4..=6).contains(&group[unit::STATUS]) {
                continue;
            }
            let get = |g: &[u8], at: usize| i16::from_le_bytes([g[at], g[at + 1]]);
            let (mut a, mut days, mut b) = (
                get(group, unit::TRAVEL_A),
                get(group, unit::TRAVEL_DAYS),
                get(group, unit::TRAVEL_B),
            );
            if a > 0 {
                a -= speed;
            } else if days > 0 {
                days -= 1;
            } else if b > 0 {
                b -= speed;
            }
            let (a, days, b) = (a.max(0), days.max(0), b.max(0));
            for (at, value) in [(unit::TRAVEL_A, a), (unit::TRAVEL_DAYS, days), (unit::TRAVEL_B, b)] {
                group[at..at + 2].copy_from_slice(&value.to_le_bytes());
            }
            group[unit::STATUS] = if b == 0 {
                2
            } else if days == 0 {
                4
            } else if a == 0 {
                5
            } else {
                6
            };
            if b == 0 {
                events.push(TravelEvent::Arrived(n));
                let (system, planet) = (group[unit::SYSTEM], group[unit::PLANET]);
                if planet == 0 && self.byte(0x4815 + u16::from(system)) == Some(0) {
                    self.explore(n, system);
                    events.push(TravelEvent::Explored(n, system));
                }
            }
        }
        events
    }

    /// A group reached an unexplored system: it's known now, the planets
    /// nobody knew of are on the map, and the group is at the last of them.
    fn explore(&mut self, n: usize, system: u8) {
        self.set_byte(0x4815 + u16::from(system), 1);
        let mut last = 0;
        for planet in 1..=8u8 {
            let at = 0x47cd + 8 * u16::from(system) + u16::from(planet);
            if self.byte(at) == Some(0xff) {
                self.set_byte(at, 0);
                last = planet;
            }
        }
        if last > 0
            && let Some(group) = self.unit_mut(UnitList::Groups, n)
        {
            group[unit::PLANET] = last;
            group[unit::MOON] = 0;
        }
    }

    /// Renames a unit, cut to 17 characters.
    pub fn rename_unit(&mut self, list: UnitList, n: usize, name: &str) {
        if let Some(record) = self.unit_mut(list, n) {
            set_pascal(&mut record[unit::NAME..unit::SYSTEM], name);
        }
    }

    /// The message log, oldest first: (kind, text). Kinds 1 and 99 are shown
    /// highlighted.
    pub fn messages(&self) -> Vec<(u16, String)> {
        let count = (self.word(MESSAGE_COUNT).unwrap_or(0) as usize).min(MAX_MESSAGES);
        self.blocks[&MESSAGES]
            .chunks_exact(MESSAGE_LEN)
            .take(count)
            .map(|m| (u16::from_le_bytes([m[0], m[1]]), pascal(&m[2..])))
            .collect()
    }

    /// Adds a message like FUN_34b0_0001: when the log already holds more
    /// than 14, the oldest is dropped.
    pub fn add_message(&mut self, kind: u16, text: &str) {
        let mut count = self.word(MESSAGE_COUNT).unwrap_or(0) as usize;
        let log = self.blocks.get_mut(&MESSAGES).expect("message block");
        if count > MAX_MESSAGES - 1 {
            log.copy_within(MESSAGE_LEN..count * MESSAGE_LEN, 0);
            count -= 1;
        }
        let record = &mut log[count * MESSAGE_LEN..(count + 1) * MESSAGE_LEN];
        record.fill(0);
        record[..2].copy_from_slice(&kind.to_le_bytes());
        let text: Vec<u8> = text.bytes().take(MESSAGE_TEXT_LEN).collect();
        record[2] = text.len() as u8;
        record[3..3 + text.len()].copy_from_slice(&text);
        self.set_word(MESSAGE_COUNT, count as u16 + 1);
    }

    /// An invention's 53-byte record (1-based), for editing its status.
    pub fn invention_mut(&mut self, invention: usize) -> Option<&mut [u8]> {
        let (address, len) = INVENTIONS;
        let start = invention.checked_sub(1)? * len;
        self.blocks.get_mut(&address)?.get_mut(start..start + len)
    }

    pub fn inventions(&self) -> Vec<Named<'_>> {
        self.records(INVENTIONS)
    }

    pub fn races(&self) -> Vec<Named<'_>> {
        self.records(RACES)
    }

    pub fn characters(&self) -> Vec<Named<'_>> {
        self.records(CHARACTERS)
    }

    /// Building records in use, 14 bytes each: type, ?, planet, ?, x, y, ...
    pub fn buildings(&self) -> Vec<&[u8]> {
        let count = &self.blocks[&BUILDING_COUNT];
        let count = u16::from_le_bytes([count[0], count[1]]) as usize;
        self.blocks[&BUILDINGS]
            .chunks_exact(BUILDING_LEN)
            .take(count)
            .collect()
    }

    fn records(&self, (address, len): (u16, usize)) -> Vec<Named<'_>> {
        self.blocks[&address]
            .chunks_exact(len)
            .map(|data| Named {
                name: pascal(data),
                data,
            })
            .collect()
    }
}

/// Borland Pascal's `Random`: a linear congruential generator.
struct BorlandRandom(u32);

impl BorlandRandom {
    fn below(&mut self, n: u16) -> u16 {
        self.0 = self.0.wrapping_mul(0x0808_8405).wrapping_add(1);
        ((self.0 as u64 * n as u64) >> 32) as u16
    }
}

/// A length-prefixed string, as Borland Pascal stores them.
/// FUN_2b8d_16d0: a fresh unit of `kind` at New Earth, on the planet, empty.
fn init_unit(record: &mut [u8], kind: u8, name: &str) {
    record.fill(0);
    record[unit::TYPE] = kind;
    set_pascal(&mut record[unit::NAME..unit::SYSTEM], name);
    record[unit::SYSTEM] = 1;
    record[unit::PLANET] = 5;
    record[unit::MOON] = 0;
    record[unit::STATUS] = 1;
}

/// Writes a Pascal string into `field` (length byte plus characters), cut to fit.
fn set_pascal(field: &mut [u8], text: &str) {
    let bytes = text.as_bytes();
    let len = bytes.len().min(field.len() - 1).min(UNIT_NAME_CAPACITY);
    field.fill(0);
    field[0] = len as u8;
    field[1..=len].copy_from_slice(&bytes[..len]);
}

fn pascal(bytes: &[u8]) -> String {
    let len = bytes.first().copied().unwrap_or(0) as usize;
    bytes
        .get(1..1 + len)
        .unwrap_or_default()
        .iter()
        .map(|&b| b as char)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn block_sizes_add_up() {
        assert_eq!(
            SAVE_NAME_LEN + BLOCKS.iter().map(|b| b.size()).sum::<usize>(),
            SAVE_LEN
        );
    }

    fn state_at(date: [u16; 4]) -> GameState {
        let mut state = GameState {
            name: [0; SAVE_NAME_LEN],
            blocks: BLOCKS
                .iter()
                .map(|b| (b.address(), vec![0; b.size()]))
                .collect(),
        };
        for (address, value) in DATE.into_iter().zip(date) {
            state.set_word(address, value);
        }
        state
    }

    #[test]
    fn hours_roll_over_into_days_months_and_years() {
        let mut state = state_at([2927, 8, 13, 23]);
        state.advance_hour();
        assert_eq!(state.date(), [2927, 8, 14, 0]);

        let mut state = state_at([2927, 12, 30, 23]);
        state.advance_hour();
        assert_eq!(state.date(), [2928, 1, 1, 0]);
    }

    #[test]
    fn message_log_drops_the_oldest_when_full() {
        let mut state = state_at([2927, 8, 13, 23]);
        for i in 0..20 {
            state.add_message(if i == 19 { 1 } else { 0 }, &format!("message {i}"));
        }
        let messages = state.messages();
        assert_eq!(messages.len(), MAX_MESSAGES);
        assert_eq!(messages[0].1, "message 5");
        assert_eq!(messages.last().unwrap(), &(1, "message 19".to_string()));
    }

    #[test]
    fn borland_random_stays_in_range() {
        let mut random = BorlandRandom(1234);
        assert!((0..1000).all(|_| random.below(400) < 400));
    }

    #[test]
    fn rejects_wrong_lengths() {
        assert!(matches!(
            GameState::from_save(&[0; 10]),
            Err(StateError::SaveLength(10))
        ));
    }
}
