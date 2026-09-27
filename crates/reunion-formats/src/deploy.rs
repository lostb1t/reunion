//! What ships in orbit can put on a planet, from PLANET INFO's extra icons
//! (REUNION.PRG FUN_28cd_062b decides which, FUN_28cd_0948 runs them), and
//! founding a colony (COLONIZATION, screen 28: FUN_2841_0060).
//!
//! Satellite carriers (unit type 4) carry satellites, spy satellites, spy
//! ships and solar satellites as the four values of their first kind; trade
//! companies (type 2) carry miner stations as the fifth value of each kind.
//! What's taken comes from every such group in orbit around the planet or
//! on it, first come first served.

use crate::colony;
use crate::exe::GameExe;
use crate::sim::{Random, Texts, planet};
use crate::state::{GameState, UnitList, unit};
use crate::story::Event;

pub const COLONIZATION: u8 = 5;
pub const ADD_MINER_STATION: u8 = 10;
pub const ADD_SATELLITE: u8 = 20;
pub const ADD_SPY_SAT: u8 = 67;
pub const ADD_SPY_SHIP: u8 = 68;
pub const ADD_SOLAR_SAT: u8 = 69;
/// Launching a satellite from New Earth's stock.
pub const LAUNCH_SATELLITE: u8 = 70;

/// A colony costs this before its buildings (FUN_2841_0060).
pub const COLONY_COST: u32 = 100_000;
/// The buildings a new colony may start with: DS:0x651e + 10 * k (k 1-6),
/// a type and its box on the colonization screen (x, y, and size in cells).
const SETUP_BUILDINGS: u16 = 0x651e;
const SETUPS: u16 = 0xa21a;
const SETUP: u16 = 0xa20c;
const MAX_SETUPS: u16 = 10;
const SATELLITE_STOCK: u16 = 0x5e31;
/// Satellites got destroyed once (the story's first turn).
const SATELLITES_LOST: u16 = 0x5d4c;
const SATELLITES_LOST_TIMER: u16 = 0x5d4a;
const COLONY_STATUS: u16 = 0x5efb;
const HIRED_BUILDER: u16 = 0x95b8;
const BUILDER_LEVEL: u16 = 0x95a6;
const MONEY: u16 = 0x95be;

/// A building a new colony may start with.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SetupBuilding {
    pub kind: u8,
    pub x: i16,
    pub y: i16,
    pub width: i16,
    pub height: i16,
}

impl GameExe {
    /// The six buildings of COLONIZATION, k 1-6 at index k - 1.
    pub fn setup_buildings(&self) -> Vec<SetupBuilding> {
        (1..=6u16)
            .filter_map(|k| {
                let b = self.ds_bytes(SETUP_BUILDINGS + 10 * k, 10)?;
                let word = |i: usize| i16::from_le_bytes([b[2 * i], b[2 * i + 1]]);
                Some(SetupBuilding {
                    kind: word(0) as u8,
                    x: word(1),
                    y: word(2),
                    width: word(3),
                    height: word(4),
                })
            })
            .collect()
    }
}

impl GameState {
    /// Groups of `kind` in orbit around or on planet `planet` of `system`.
    fn groups_at(&self, system: u8, planet: u8, kind: u8) -> Vec<usize> {
        (1..=self.unit_count(UnitList::Groups))
            .filter(|&n| {
                self.unit(UnitList::Groups, n).is_some_and(|g| {
                    g[unit::SYSTEM] == system
                        && g[unit::PLANET] == planet
                        && matches!(g[unit::STATUS], 1 | 2)
                        && g[unit::TYPE] == kind
                })
            })
            .collect()
    }

    fn group_word(&self, n: usize, at: usize) -> u16 {
        self.unit(UnitList::Groups, n).map_or(0, |g| u16::from_le_bytes([g[at], g[at + 1]]))
    }

    /// Takes up to `count` from word `at` of the groups, in order.
    fn take(&mut self, groups: &[usize], at: &[usize], mut count: u16) {
        for &n in groups {
            for &at in at {
                let have = self.group_word(n, at);
                let taken = have.min(count);
                if let Some(g) = self.unit_mut(UnitList::Groups, n) {
                    g[at..at + 2].copy_from_slice(&(have - taken).to_le_bytes());
                }
                count -= taken;
            }
        }
    }

    /// FUN_28cd_0a98: satellites of kind `j` (1 satellite, 2 spy satellite,
    /// 3 spy ship, 4 solar satellite) on carriers at the planet.
    pub fn carried_satellites(&self, system: u8, planet: u8, j: usize) -> u32 {
        self.groups_at(system, planet, 4)
            .into_iter()
            .map(|n| u32::from(self.group_word(n, unit::SLOTS + 2 * j)))
            .sum()
    }

    /// FUN_28cd_0c81: miner stations on trade ships at the planet.
    pub fn carried_miner_stations(&self, system: u8, planet: u8) -> u32 {
        self.groups_at(system, planet, 2)
            .into_iter()
            .map(|n| (1..=4).map(|k| u32::from(self.group_word(n, unit::SLOTS + 10 * k - 2))).sum::<u32>())
            .sum()
    }

    /// FUN_28cd_062b: the extra icons PLANET INFO shows for a planet or moon.
    pub fn deploy_actions(&self, exe: &GameExe, place: (u8, u8, u8), body: usize) -> Vec<u8> {
        let Some(r) = self.body(place.0.into(), body) else {
            return Vec::new();
        };
        let (s, p, _) = place;
        let owner = r[planet::OWNER];
        let observed = r[planet::OBSERVED] as i8;
        let unwatched = r[planet::SATELLITES] == 0
            && ((r[planet::COLONY] == 0 && r[planet::DROIDS] == 0) || owner > 1);
        let mut actions = Vec::new();
        let lost = self.byte(SATELLITES_LOST).unwrap_or(0) != 0;
        if (!lost || (s, p) == (1, 5)) && unwatched && self.word(SATELLITE_STOCK).unwrap_or(0) as i16 > 0 {
            actions.push(LAUNCH_SATELLITE);
        }
        if unwatched && self.carried_satellites(s, p, 1) > 0 && !actions.contains(&LAUNCH_SATELLITE) {
            actions.push(ADD_SATELLITE);
        }
        if unwatched && self.carried_satellites(s, p, 2) > 0 {
            actions.push(ADD_SPY_SAT);
        }
        if r[9] == 0 && owner > 1 && observed > 29 && self.carried_satellites(s, p, 3) > 0 {
            actions.push(ADD_SPY_SHIP);
        }
        if owner == 1 && r[planet::COLONY] != 0 && r[planet::SOLAR] < 5 && self.carried_satellites(s, p, 4) > 0 {
            actions.push(ADD_SOLAR_SAT);
        }
        if owner == 0
            && r[planet::COLONY] == 0
            && r[planet::DROIDS] == 0
            && observed > 9
            && r[planet::FOUNDING] == 0
            && r[planet::HAS_ORES] != 0
            && self.carried_miner_stations(s, p) > 0
        {
            actions.push(ADD_MINER_STATION);
        }
        let terrain = exe.terrain_for_type(r[planet::TYPE]).unwrap_or(0);
        if self.word(COLONY_STATUS) == Some(5)
            && r[planet::COLONY] == 0
            && r[planet::FOUNDING] == 0
            && observed > 29
            && r[planet::HABITABLE] != 0
            && owner < 2
            && exe.habitable(terrain).unwrap_or(false)
            && self.word(SETUPS).unwrap_or(0) < MAX_SETUPS
        {
            actions.push(COLONIZATION);
        }
        actions
    }

    /// FUN_28cd_0948: an extra icon of PLANET INFO was used. Returns the
    /// message boxes. COLONIZATION isn't done here but on its own screen.
    pub fn deploy(
        &mut self,
        exe: &GameExe,
        texts: &Texts,
        action: u8,
        place: (u8, u8, u8),
        body: usize,
        random: Random,
    ) -> Vec<Event> {
        let mut reports = Vec::new();
        if !self.deploy_actions(exe, place, body).contains(&action) {
            return reports;
        }
        let (s, p, _) = place;
        let owner = self.body(s.into(), body).map_or(0, |r| r[planet::OWNER]);
        let set = |state: &mut GameState, at: usize, value: u8| {
            if let Some(r) = state.planet_mut(s.into(), body) {
                r[at] = value;
            }
        };
        let carriers = self.groups_at(s, p, 4);
        match action {
            // FUN_28cd_0000: launched from New Earth's stock; away from New
            // Earth they're shot down (and the story begins).
            LAUNCH_SATELLITE => {
                let stock = self.word(SATELLITE_STOCK).unwrap_or(0);
                self.set_word(SATELLITE_STOCK, stock.saturating_sub(1));
                if (s, p) == (1, 5) {
                    set(self, planet::SATELLITES, 1);
                } else {
                    set(self, planet::SATELLITES, 0xe2);
                    if self.word(SATELLITES_LOST_TIMER).unwrap_or(0) == 0 {
                        self.set_word(SATELLITES_LOST_TIMER, 5 + random(10));
                    }
                }
            }
            // FUN_28cd_0117.
            ADD_SATELLITE => {
                self.take(&carriers, &[unit::SLOTS + 2], 1);
                if owner < 2 {
                    set(self, planet::SATELLITES, 1);
                    self.found_by_satellite(texts, place, random, &mut reports);
                } else {
                    reports.push(Event::Message("| You lost the contact with your satellite ! |".into()));
                }
            }
            // FUN_28cd_018d.
            ADD_SPY_SAT => {
                set(self, planet::SATELLITES, 2);
                self.take(&carriers, &[unit::SLOTS + 4], 1);
                self.found_by_satellite(texts, place, random, &mut reports);
            }
            // FUN_28cd_01d6.
            ADD_SPY_SHIP => {
                set(self, 9, 1);
                self.take(&carriers, &[unit::SLOTS + 6], 1);
                self.found_by_satellite(texts, place, random, &mut reports);
            }
            // FUN_28cd_021f.
            ADD_SOLAR_SAT => {
                let solar = self.body(s.into(), body).map_or(0, |r| r[planet::SOLAR]);
                set(self, planet::SOLAR, solar + 1);
                self.take(&carriers, &[unit::SLOTS + 2 * 4], 1);
                self.update_colony(exe, place, body);
            }
            // FUN_28cd_0254: the planet is yours now, with a droid at work.
            ADD_MINER_STATION => {
                let Some(r) = self.body(s.into(), body).map(<[u8]>::to_vec) else {
                    return reports;
                };
                set(self, planet::OBSERVED, (r[planet::OBSERVED] as i8 + 10).min(60) as u8);
                set(self, planet::OWNER, 1);
                set(self, planet::DROIDS, 1);
                let traders = self.groups_at(s, p, 2);
                let at: Vec<usize> = (1..=4).map(|k| unit::SLOTS + 10 * k - 2).collect();
                self.take(&traders, &at, 1);
                let terrain = exe.terrain_for_type(r[planet::TYPE]).unwrap_or(0);
                if self.word(COLONY_STATUS) == Some(0)
                    && self.word(0x919e + 2 * 7) == Some(0xffff)
                    && r[planet::HABITABLE] != 0
                    && exe.habitable(terrain).unwrap_or(false)
                {
                    self.start_invention_timer(7, 20, 40, random);
                    let observed = self.body(s.into(), body).map_or(0, |r| r[planet::OBSERVED]);
                    set(self, planet::OBSERVED, observed.wrapping_add(10));
                    reports.push(Event::Message(texts.messages.get(3).cloned().unwrap_or_default()));
                }
                if let Ok(kind) = exe.building_type(colony::MINER_STATION) {
                    self.add_building(&kind, colony::MINER_STATION, place, (0xff, 0xff), r[planet::TYPE], |n| {
                        random(n)
                    });
                }
            }
            _ => {}
        }
        reports
    }

    /// FUN_28cd_0050: satellites put around some planets find the story's
    /// wrecks.
    fn found_by_satellite(&mut self, texts: &Texts, place: (u8, u8, u8), random: Random, reports: &mut Vec<Event>) {
        let message = |n: usize| Event::Message(texts.messages.get(n - 1).cloned().unwrap_or_default());
        if place == (3, 2, 1) && self.word(0x6216) == Some(0) {
            reports.push(message(21));
            self.make_known(22);
            self.make_known(23);
            // Screen 33 shows MESSAGE.TXT line 36 before picture 4.
            reports.push(message(36));
            reports.push(Event::Scene(4));
        }
        if place == (7, 1, 0) && self.word(0x63f3) == Some(0) {
            reports.push(message(31));
            self.make_known(31);
            self.make_known(32);
            self.start_invention_timer(34, 20, 100, random);
            reports.push(Event::Scene(6));
        }
    }

    /// FUN_29b9_1409: an invention you hadn't heard of can be researched.
    pub fn make_known(&mut self, n: u16) {
        let at = 0x5d77 + 0x35 * n + 0x11;
        if self.word(at) == Some(0) {
            self.set_word(at, 3);
        }
    }

    /// Whether your builder can set up a colony in `system`
    /// (FUN_2841_0060): a good one anywhere, an average one near home.
    /// The message box when not.
    pub fn colony_builder_problem(&self, system: u8) -> Option<&'static str> {
        match self.word(HIRED_BUILDER).unwrap_or(0) {
            3 => None,
            2 if system < 3 => None,
            0 => Some("|  You haven't got a builder!  |"),
            _ => Some("|  You builder not able to build this colony  |"),
        }
    }

    /// Whether COLONIZATION may put building `kind` there (FUN_2841_062d):
    /// invented or within your builder's level, and buildable on the terrain.
    pub fn setup_building_available(&self, exe: &GameExe, kind: u8, planet_type: u8) -> bool {
        let Ok(k) = exe.building_type(kind) else { return false };
        let known = if k.invention == 0 {
            i32::from(k.builder_level) <= i32::from(self.word(BUILDER_LEVEL).unwrap_or(0))
        } else {
            self.word(0x5d77 + 0x35 * u16::from(k.invention) + 0x11) == Some(5)
        };
        known && k.terrain.get(usize::from(planet_type)).is_some_and(|&t| t != 0)
    }

    /// OK BUILD IT: the colony ship lands. Pays for it, starts the setup of
    /// the chosen buildings (1-6), makes the planet yours (the settlers
    /// arrive in a day or two) and creates its base. False when the money
    /// isn't enough.
    pub fn found_colony(
        &mut self,
        exe: &GameExe,
        place: (u8, u8, u8),
        body: usize,
        chosen: &[bool; 6],
        random: Random,
    ) -> bool {
        let setups = exe.setup_buildings();
        let cost = COLONY_COST
            + setups
                .iter()
                .zip(chosen)
                .filter(|(_, c)| **c)
                .filter_map(|(b, _)| exe.building_type(b.kind).ok())
                .map(|k| k.cost)
                .sum::<u32>();
        let count = self.word(SETUPS).unwrap_or(0);
        if self.money() < cost || count >= MAX_SETUPS {
            return false;
        }
        let money = self.money() - cost;
        self.set_word(MONEY, money as u16);
        self.set_word(MONEY + 2, (money >> 16) as u16);
        let n = count + 1;
        self.set_word(SETUPS, n);
        let at = SETUP + 16 * n;
        for i in 0..16 {
            self.set_byte(at + i, 0);
        }
        self.set_byte(at + 1, place.0);
        self.set_byte(at + 2, place.1);
        self.set_byte(at + 3, place.2);
        for (k, &c) in chosen.iter().enumerate() {
            self.set_word(at + 4 + 2 * k as u16, u16::from(c));
        }
        // FUN_28cd_02ec.
        if let Some(r) = self.planet_mut(place.0.into(), body) {
            r[planet::OWNER] = 1;
            r[planet::OBSERVED] = (r[planet::OBSERVED] as i8 + 10).min(60) as u8;
            r[planet::FOUNDING] = random(2) as u8 + 1;
        }
        // The base, "<planet> forces".
        if let Some(b) = self.add_base(place) {
            let name = format!("{} forces", self.place_name(place.0, body));
            self.rename_unit(UnitList::Bases, b, &name);
        }
        true
    }
}
