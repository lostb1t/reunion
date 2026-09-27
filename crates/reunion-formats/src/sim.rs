//! What happens as time passes: REUNION.PRG's main loop runs these every
//! game hour, in this order, after the clock (FUN_1b8a_0000):
//!
//! - invention timers (first part of FUN_1b8a_3d07): an invention your
//!   developers thought of becomes developable;
//! - research (FUN_1b8a_04d8);
//! - production (FUN_1b8a_0705);
//! - at midnight, the day's colony update (FUN_1b8a_206e / 0cba): people,
//!   morale, taxes, disasters, satellite observation, colonies being set
//!   up, planets found by observatories;
//! - commanders' levels and the university (FUN_1b8a_3b83);
//! - construction and derricks (FUN_1b8a_2178);
//! - colonies being set up (FUN_1b8a_25b5);
//! - mining (FUN_1b8a_2837).
//!
//! Travel (FUN_1b8a_2a0e) is [`GameState::travel_hour`], the aliens
//! (FUN_1b8a_3546) [`GameState::aliens_hour`], and the story's timeline
//! (the rest of 3d07) [`GameState::story_hour`]. The characters of the pub
//! (FUN_1b8a_006e) aren't here yet.
//!
//! What happens is reported as [`Event`]s: message boxes (also logged),
//! story pictures and conversations.

use crate::colony::{self, building, BuildingType};
pub use crate::story::Event;
use crate::exe::GameExe;
use crate::state::{GameState, UnitList};
use crate::text::decrypt_lines;

/// Lines of TEXT/MESSAGE.TXT (story events) and TEXT/KITALAL.TXT ("The
/// ... is invented"), 1-based line n at index n - 1.
#[derive(Debug, Clone, Default)]
pub struct Texts {
    pub messages: Vec<String>,
    pub inventions: Vec<String>,
}

impl Texts {
    pub fn parse(message_txt: &[u8], kitalal_txt: &[u8]) -> Self {
        let lines = |data: &[u8]| {
            decrypt_lines(data)
                .into_iter()
                .map(|l| l.into_iter().map(char::from).collect())
                .collect()
        };
        Self {
            messages: lines(message_txt),
            inventions: lines(kitalal_txt),
        }
    }

    fn message(&self, n: usize) -> String {
        self.messages.get(n.wrapping_sub(1)).cloned().unwrap_or_default()
    }
}

/// The game's Random(n): 0 to n - 1.
pub type Random<'a> = &'a mut dyn FnMut(u16) -> u16;

const INVENTIONS: u16 = 35;
const INVENTION: u16 = 0x5d77;
const INVENTION_LEN: u16 = 0x35;
/// Per invention, hours until it may be developed (-1 none).
const INVENTION_TIMERS: u16 = 0x919e;
/// Research project, then the commanders' levels (2 * category).
const PROJECT: u16 = 0x95a2;
const LEVELS: u16 = 0x95a2;
const HIRED: u16 = 0x95b4;
const DEVELOPER_SKILLS: u16 = 0x95aa;
const DEVELOPER: u16 = 4;
const BUILDER: u16 = 2;
/// Hours of production done in an hour (the word before the hired table).
const PRODUCTION_RATE: u16 = 0x95b4;
const UNIVERSITY_HOURS: u16 = 0x5d9c;
const UNIVERSITY_STUDENT: u16 = 0x5d9e;
const UNIVERSITY_COURSE: u16 = 0x5da0;
/// Your scientist is away with the Jaanosians.
const SCIENTIST_AWAY: u16 = 0x5d52;
const MONEY: u16 = 0x95be;
/// New Earth's ore n (1-6) at + 4 * n.
const NEW_EARTH_ORES: u16 = 0x95c2;
const NEW_EARTH: (u8, u8, u8) = (1, 5, 0);
/// Colonies being set up: a count, then 16-byte records (a flag, the place,
/// six hour counters), record n at + 16 * n.
const SETUPS: u16 = 0xa21a;
const SETUP: u16 = 0xa20c;
/// The buildings a colony gets, in the order of the setup counters.
const SETUP_BUILDINGS: u16 = 0x651e;
/// Per system, DS:0x47cd + 8 * system + planet: the map visibility.
const VISIBILITY: u16 = 0x47cd;
const SYSTEM_KNOWN: u16 = 0x4815;
const RACE: u16 = 0x6a06;
const RACE_LEN: u16 = 0xe4;

/// Byte offsets in a planet or moon record.
pub mod planet {
    /// 0 nobody, 1 you, 2+ an alien race.
    pub const OWNER: usize = 0x00;
    pub const HABITABLE: usize = 0x02;
    pub const HAS_ORES: usize = 0x03;
    /// i16: what the builder plants there build in an hour.
    pub const BUILDERS: usize = 0x04;
    pub const COLONY: usize = 0x06;
    /// Days until the colony landed there is set up.
    pub const FOUNDING: usize = 0x07;
    /// i8: satellites watching it (negative: destroyed, recovering).
    pub const SATELLITES: usize = 0x08;
    pub const DROIDS: usize = 0x0a;
    pub const SOLAR: usize = 0x0b;
    /// How much the satellites have seen, 0-60.
    pub const OBSERVED: usize = 0x0c;
    /// u32.
    pub const POPULATION: usize = 0x0d;
    pub const WEALTH: usize = 0x11;
    pub const TAX: usize = 0x12;
    pub const MORALE: usize = 0x13;
    /// Days to the next outbreak of disease; 0 an outbreak, 100 meteorites.
    pub const DISEASE: usize = 0x14;
    pub const TYPE: usize = 0x15;
    /// u32 ore n (1-6) at + 4 * n.
    pub const ORES: usize = 0x17;
    /// u32 days of revolt.
    pub const REVOLT: usize = 0x33;
    /// Ore n's richness at + n.
    pub const RICHNESS: usize = 0x3a;
}

fn long(r: &[u8], at: usize) -> i32 {
    i32::from_le_bytes([r[at], r[at + 1], r[at + 2], r[at + 3]])
}

fn set_long(r: &mut [u8], at: usize, value: i32) {
    r[at..at + 4].copy_from_slice(&value.to_le_bytes());
}

/// A planet or moon of a known system: its place and body number.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Place {
    pub place: (u8, u8, u8),
    pub body: usize,
}

impl GameState {
    fn int(&self, address: u16) -> i16 {
        self.word(address).unwrap_or(0) as i16
    }

    fn set_int(&mut self, address: u16, value: i16) {
        self.set_word(address, value as u16);
    }

    fn long_at(&self, address: u16) -> i32 {
        i32::from(self.word(address).unwrap_or(0)) | i32::from(self.word(address + 2).unwrap_or(0)) << 16
    }

    fn set_long_at(&mut self, address: u16, value: i32) {
        self.set_word(address, value as u16);
        self.set_word(address + 2, (value >> 16) as u16);
    }

    fn invention_word(&self, n: u16, at: u16) -> i16 {
        self.int(INVENTION + INVENTION_LEN * n + at)
    }

    fn set_invention_word(&mut self, n: u16, at: u16, value: i16) {
        self.set_int(INVENTION + INVENTION_LEN * n + at, value);
    }

    /// Every planet and moon of the known systems, in the original's order.
    pub fn known_places(&self, exe: &GameExe) -> Vec<Place> {
        let Ok(systems) = exe.star_systems() else {
            return Vec::new();
        };
        let mut places = Vec::new();
        for (s, layout) in systems.iter().enumerate() {
            let s = s as u8 + 1;
            if !self.system_known(s.into()) {
                continue;
            }
            for (p, moons) in layout.moons.iter().enumerate() {
                let p = p as u8 + 1;
                places.push(Place { place: (s, p, 0), body: p.into() });
                for (m, &body) in moons.iter().enumerate() {
                    places.push(Place { place: (s, p, m as u8 + 1), body: body.into() });
                }
            }
        }
        places
    }

    /// A planet's or moon's name.
    pub fn place_name(&self, system: u8, body: usize) -> String {
        self.star_systems()
            .into_iter()
            .nth(usize::from(system).wrapping_sub(1))
            .and_then(|s| s.bodies.into_iter().nth(body.wrapping_sub(1)))
            .map(|b| b.name.trim_end().to_string())
            .unwrap_or_default()
    }

    /// FUN_29b9_136e: invention `n` will be thought of in `base` + Random(`spread`)
    /// hours, unless it's known or already coming.
    pub fn start_invention_timer(&mut self, n: u16, spread: u16, base: u16, random: Random) {
        if self.invention_word(n, 0x11) == 0 && self.int(INVENTION_TIMERS + 2 * n) == -1 {
            let hours = base + random(spread);
            self.set_int(INVENTION_TIMERS + 2 * n, hours as i16);
        }
    }

    /// The game's hour after the clock moved on: what it shows.
    pub fn simulate_hour(&mut self, exe: &GameExe, texts: &Texts, random: Random) -> Vec<Event> {
        let mut reports = Vec::new();
        self.invention_timers(texts, &mut reports);
        let mut events: Vec<Event> = reports.drain(..).map(Event::Message).collect();
        events.extend(self.story_hour(exe, texts, random));
        self.research(texts, random, &mut reports);
        self.production();
        if self.date()[3] == 0 {
            self.day(exe, texts, random, &mut reports);
        }
        self.levels(exe, random, &mut reports);
        self.construction(exe, random);
        self.colony_setups(exe, random);
        self.mining(exe, random);
        events.extend(reports.into_iter().map(Event::Message));
        events
    }

    /// FUN_1b8a_3d07's invention timers: while you have a developer who
    /// isn't away, each counts down; at 0 the invention can be developed.
    fn invention_timers(&mut self, texts: &Texts, reports: &mut Vec<String>) {
        let away = self.int(UNIVERSITY_HOURS) != 0 && self.int(UNIVERSITY_STUDENT) == DEVELOPER as i16;
        if self.int(LEVELS + 2 * DEVELOPER) == 0 || self.int(SCIENTIST_AWAY) != 0 || away {
            return;
        }
        for n in 1..=INVENTIONS {
            let timer = INVENTION_TIMERS + 2 * n;
            if self.int(timer) <= 0 {
                continue;
            }
            self.set_int(timer, self.int(timer) - 1);
            // The galleon waits for the trade ship.
            if n == 15 && self.invention_word(9, 0x11) != 5 && self.int(timer) == 0 {
                self.set_int(timer, 10);
            }
            if n == 34 && self.int(timer) == 50 {
                reports.push(texts.message(33));
            }
            if self.int(timer) == 0 {
                // FUN_34b0_01df: KITALAL.TXT line n.
                let line = texts.inventions.get(usize::from(n) - 1).cloned().unwrap_or_default();
                reports.push(format!(" {line} "));
                self.set_invention_word(n, 0x11, 1);
            }
        }
    }

    /// FUN_1b8a_04d8: every project being developed gets on by the
    /// developer's level, down to where the developer's skills fall short.
    fn research(&mut self, texts: &Texts, random: Random, reports: &mut Vec<String>) {
        if self.int(SCIENTIST_AWAY) != 0 || self.int(UNIVERSITY_STUDENT) == DEVELOPER as i16 {
            return;
        }
        let level = i32::from(self.word(LEVELS + 2 * DEVELOPER).unwrap_or(0));
        for n in 1..=INVENTIONS {
            let status = self.invention_word(n, 0x11);
            if status != 2 && status != 4 {
                continue;
            }
            let record = INVENTION + INVENTION_LEN * n;
            let shortfall: i32 = (1..=4u16)
                .map(|k| {
                    let needed = i32::from(self.byte(record + 0x30 + k).unwrap_or(0));
                    let skill = i32::from(self.word(DEVELOPER_SKILLS + 2 * k).unwrap_or(0));
                    (needed - skill).max(0)
                })
                .sum();
            let floor = 10_000 - 10_000 / (shortfall + 1);
            let mut progress = i32::from(self.invention_word(n, 0x15));
            if floor < progress {
                let difficulty = i32::from(self.invention_word(n, 0x13)).max(1);
                progress -= level * 10_000 / difficulty;
            }
            if progress >= 1 {
                self.set_invention_word(n, 0x15, progress as i16);
                continue;
            }
            self.set_invention_word(n, 0x15, 0);
            self.set_invention_word(n, 0x11, 5);
            self.set_int(PROJECT, 0);
            let name = self.inventions().get(usize::from(n) - 1).map(|i| i.name.clone()).unwrap_or_default();
            reports.push(format!("The {name} is ready for production"));
            match n {
                5 => {
                    self.start_invention_timer(6, 10, 30, random);
                    reports.push(texts.message(3));
                }
                14 => {
                    self.start_invention_timer(15, 70, 50, random);
                    reports.push(texts.message(12));
                }
                13 => self.start_invention_timer(15, 30, 200, random),
                23 => {
                    self.start_invention_timer(24, 70, 50, random);
                    reports.push(texts.message(22));
                }
                _ => {}
            }
            // Unit types that can be created now.
            let unlocks: &[u16] = match n {
                4 => &[0xa2d4, 0xa2d9],
                6 => &[0xa2d4, 0xa2d7],
                10 => &[0xa2d6],
                _ => &[],
            };
            for &flag in unlocks {
                self.set_byte(flag, 1);
            }
        }
    }

    /// FUN_1b8a_0705: pieces ordered on New Earth get made one after the
    /// other; a finished one goes into stock.
    fn production(&mut self) {
        let rate = self.int(PRODUCTION_RATE);
        for n in 1..=INVENTIONS {
            let ordered = self.invention_word(n, 0x1d);
            if ordered == 0 {
                continue;
            }
            let left = self.invention_word(n, 0x21).wrapping_sub(rate);
            if left >= 1 {
                self.set_invention_word(n, 0x21, left);
                continue;
            }
            self.set_invention_word(n, 0x21, 0);
            self.set_invention_word(n, 0x1b, self.invention_word(n, 0x1b) + 1);
            self.set_invention_word(n, 0x1d, ordered - 1);
            if ordered - 1 > 0 {
                self.set_invention_word(n, 0x21, self.invention_word(n, 0x1f));
            }
        }
    }

    /// FUN_1b8a_3b83: a commander gets a level better now and then, up to
    /// what they can be; one at the university comes back much better.
    fn levels(&mut self, exe: &GameExe, random: Random, reports: &mut Vec<String>) {
        let most = |state: &GameState, category: u16| {
            let hired = state.word(HIRED + 2 * category).unwrap_or(0);
            u16::from(exe.ds_bytes(0x57e0 + 3 * category + hired, 1).map_or(0, |b| b[0]))
        };
        for category in 1..=4 {
            if self.word(HIRED + 2 * category).unwrap_or(0) == 0 || random(100) != 0 {
                continue;
            }
            let level = self.word(LEVELS + 2 * category).unwrap_or(0);
            if level < most(self, category) {
                self.set_word(LEVELS + 2 * category, level + 1);
            }
        }
        let student = self.word(UNIVERSITY_STUDENT).unwrap_or(0);
        if student == 0 || student > 4 {
            return;
        }
        let hours = self.int(UNIVERSITY_HOURS);
        if hours > 0 {
            self.set_int(UNIVERSITY_HOURS, hours - 1);
        }
        if self.int(UNIVERSITY_HOURS) != 0 {
            return;
        }
        let level = self.word(LEVELS + 2 * student).unwrap_or(0) + 0x69 + random(4);
        self.set_word(LEVELS + 2 * student, level.min(most(self, student)));
        if student == DEVELOPER {
            let course = self.word(UNIVERSITY_COURSE).unwrap_or(0);
            let hired = self.word(HIRED + 2 * DEVELOPER).unwrap_or(0);
            let era = self.word(0x91ea).unwrap_or(0);
            for k in 1..=4u16 {
                let gain = u16::from(exe.ds_bytes(0x125 + course * 4 + k, 1).map_or(0, |b| b[0]));
                let cap = u16::from(exe.ds_bytes(0x57eb + era * 12 + hired * 4 + k, 1).map_or(0, |b| b[0]));
                let skill = self.word(DEVELOPER_SKILLS + 2 * k).unwrap_or(0) + gain;
                self.set_word(DEVELOPER_SKILLS + 2 * k, skill.min(cap));
            }
        }
        self.set_word(UNIVERSITY_STUDENT, 0);
        reports.push("| Your advisor has returned from the university |".into());
    }

    fn record_of(&self, exe: &GameExe, place: (u8, u8, u8)) -> Option<usize> {
        if place.2 == 0 {
            return Some(place.1.into());
        }
        let systems = exe.star_systems().ok()?;
        let moons = systems.get(usize::from(place.0).checked_sub(1)?)?.moons.get(usize::from(place.1).checked_sub(1)?)?;
        moons.get(usize::from(place.2) - 1).map(|&b| b.into())
    }

    /// Ore `n` (1-6) in stock at a place, and where to put it back.
    fn ore(&self, place: (u8, u8, u8), body: usize, n: usize) -> i32 {
        if place == NEW_EARTH {
            self.long_at(NEW_EARTH_ORES + 4 * n as u16)
        } else {
            self.body(place.0.into(), body).map_or(0, |r| long(r, planet::ORES + 4 * n))
        }
    }

    fn set_ore(&mut self, place: (u8, u8, u8), body: usize, n: usize, value: i32) {
        if place == NEW_EARTH {
            self.set_long_at(NEW_EARTH_ORES + 4 * n as u16, value);
        } else if let Some(r) = self.planet_mut(place.0.into(), body) {
            set_long(r, planet::ORES + 4 * n, value);
        }
    }

    /// FUN_1b8a_2178: building sites get on by the planet's builder plants
    /// and your builder; a site crossing 40 or 80 hours left, or finishing,
    /// changes the colony. Working derricks pump Detoxin into storage.
    fn construction(&mut self, exe: &GameExe, random: Random) {
        let level = i32::from(self.word(LEVELS + 2 * BUILDER).unwrap_or(0));
        let hired = self.word(HIRED + 2 * BUILDER).unwrap_or(0);
        let mut n = 1;
        let mut kinds: Vec<Option<BuildingType>> = vec![None; usize::from(colony::BUILDING_TYPES) + 1];
        while let Some(b) = self.buildings().get(n - 1).map(|b| b.to_vec()) {
            let place = (b[1], b[2], b[3]);
            let kind = b[building::TYPE];
            if kinds.get(usize::from(kind)).is_some_and(Option::is_none)
                && let Ok(k) = exe.building_type(kind)
            {
                kinds[usize::from(kind)] = Some(k);
            }
            let Some(Some(k)) = kinds.get(usize::from(kind)).cloned() else {
                n += 1;
                continue;
            };
            let Some(body) = self.record_of(exe, place) else {
                n += 1;
                continue;
            };
            let before = b[building::CONSTRUCTION];
            if before != 0 {
                let builders = self
                    .body(place.0.into(), body)
                    .map_or(0, |r| i32::from(i16::from_le_bytes([r[planet::BUILDERS], r[planet::BUILDERS + 1]])));
                let mut speed = (builders + 4000 + level * 100) / 1500;
                if hired == 1 {
                    speed /= 2;
                }
                if hired == 3 {
                    speed *= 2;
                }
                if random(u16::from(k.build_time)) < 10
                    && let Some(r) = self.building_mut(n)
                {
                    r[building::CONSTRUCTION] = if speed >= i32::from(before) { 0 } else { before - speed as u8 };
                }
                let after = self.buildings()[n - 1][building::CONSTRUCTION];
                if after == 0 || (after < 40 && before >= 40) || (after < 80 && before >= 80) {
                    self.update_colony(exe, place, body);
                }
            }
            let b = self.buildings()[n - 1].to_vec();
            if b[building::CONSTRUCTION] == 0 && b[building::ACTIVE] != 0 && kind == colony::DERRICK {
                let room = i32::from(self.storage_points(place)) * 1000;
                if self.ore(place, body, 1) < room && random(100) < 35 {
                    let rich = self.body(place.0.into(), body).map_or(0, |r| r[planet::RICHNESS + 1]);
                    let ore = self.ore(place, body, 1) + i32::from(rich / 10);
                    self.set_ore(place, body, 1, ore);
                }
            }
            n += 1;
        }
    }

    /// FUN_1b8a_2837 / 26cf: on your planets with a colony or droids, the
    /// droids dig each ore (2-6) by its richness, into storage; without a
    /// colony they manage it three hours in ten.
    fn mining(&mut self, exe: &GameExe, random: Random) {
        for Place { place, body } in self.known_places(exe) {
            let Some(r) = self.body(place.0.into(), body).map(<[u8]>::to_vec) else {
                continue;
            };
            if r[planet::OWNER] != 1 || (r[planet::COLONY] == 0 && r[planet::DROIDS] == 0) {
                continue;
            }
            let room = i32::from(self.storage_points(place)) * 1000;
            for ore in 2..=6 {
                if self.ore(place, body, ore) >= room || (r[planet::COLONY] == 0 && random(100) >= 30) {
                    continue;
                }
                let dug = i32::from(r[planet::RICHNESS + ore]) * i32::from(r[planet::DROIDS]) / 10;
                self.set_ore(place, body, ore, self.ore(place, body, ore) + dug);
            }
        }
    }

    /// FUN_1b8a_25b5: a colony being set up gets its buildings one by one.
    fn colony_setups(&mut self, exe: &GameExe, random: Random) {
        let mut n = 1u16;
        while n <= self.word(SETUPS).unwrap_or(0).min(10) {
            let at = SETUP + 16 * n;
            if self.byte(at).unwrap_or(0) == 0 {
                n += 1;
                continue;
            }
            let place = (self.byte(at + 1).unwrap_or(0), self.byte(at + 2).unwrap_or(0), self.byte(at + 3).unwrap_or(0));
            let mut done = true;
            for k in 1..=6u16 {
                let counter = at + 2 + 2 * k;
                let hours = self.int(counter);
                if hours == 0 {
                    continue;
                }
                self.set_int(counter, hours - 1);
                if hours - 1 == 0 {
                    let kind = exe.ds_bytes(SETUP_BUILDINGS + 10 * k, 1).map_or(0, |b| b[0]);
                    self.add_unplaced_building(exe, kind, place, random);
                } else {
                    done = false;
                }
            }
            if done {
                let count = self.word(SETUPS).unwrap_or(0);
                for m in n..count {
                    for i in 0..16 {
                        let byte = self.byte(SETUP + 16 * (m + 1) + i).unwrap_or(0);
                        self.set_byte(SETUP + 16 * m + i, byte);
                    }
                }
                self.set_word(SETUPS, count - 1);
            } else {
                n += 1;
            }
        }
    }

    /// FUN_2ef2_29b1 away from the planet's view: a new building that finds
    /// its place when the planet is next looked at.
    fn add_unplaced_building(&mut self, exe: &GameExe, kind: u8, place: (u8, u8, u8), random: Random) {
        let (Ok(k), Some(body)) = (exe.building_type(kind), self.record_of(exe, place)) else {
            return;
        };
        let planet_type = self.body(place.0.into(), body).map_or(0, |r| r[planet::TYPE]);
        self.add_building(&k, kind, place, (0xff, 0xff), planet_type, random);
    }

    /// FUN_29b9_0666 / 05b5: a colony is lost, with its buildings and base.
    fn lose_colony(&mut self, place: (u8, u8, u8), body: usize) {
        if let Some(r) = self.planet_mut(place.0.into(), body) {
            r[planet::OWNER] = 0;
            r[1] = 0;
            r[planet::BUILDERS..planet::BUILDERS + 2].fill(0);
            r[planet::COLONY] = 0;
            r[planet::FOUNDING] = 0;
            r[planet::DROIDS] = 0;
            r[planet::SOLAR] = 0;
            r[planet::POPULATION..planet::POPULATION + 4].fill(0);
            r[planet::WEALTH] = 0;
            r[planet::TAX] = 0;
            r[planet::MORALE] = 0;
            r[0x1b..0x3b].fill(0);
        }
        while let Some(&n) = self.buildings_at(place).first() {
            self.remove_building(n);
        }
        while let Some(n) = self.base_at(place.0, place.1, place.2) {
            self.disband_unit(UnitList::Bases, n);
        }
    }

    /// FUN_1b8a_206e: the day's update of every planet and moon in the
    /// known systems; the taxes go to your money.
    fn day(&mut self, exe: &GameExe, texts: &Texts, random: Random, reports: &mut Vec<String>) {
        let mut income = 0i64;
        let mut found = false;
        let stars: Vec<String> = exe
            .star_systems()
            .map(|s| s.into_iter().map(|l| l.name.trim_end().to_string()).collect())
            .unwrap_or_default();
        for Place { place, body } in self.known_places(exe) {
            income += self.colony_day(exe, place, body, random, reports);
            self.observation_day(exe, texts, place, body, random, reports);
            self.discovery_day(place, &stars, random, &mut found, reports);
        }
        let money = i64::from(self.money()) + income;
        let money = money.clamp(0, i64::from(u32::MAX)) as u32;
        self.set_word(MONEY, money as u16);
        self.set_word(MONEY + 2, (money >> 16) as u16);
    }

    /// Finished, switched-on buildings of a type at a place (FUN_2ef2_2b9e).
    fn working(&self, place: (u8, u8, u8), kind: u8) -> i32 {
        self.buildings()
            .iter()
            .filter(|b| {
                (b[1], b[2], b[3]) == place
                    && b[building::TYPE] == kind
                    && b[building::CONSTRUCTION] == 0
                    && b[building::ACTIVE] != 0
            })
            .count() as i32
    }

    /// FUN_1b8a_0cba's colony part: what the people want and get, how many
    /// they become, and their taxes.
    fn colony_day(
        &mut self,
        exe: &GameExe,
        place: (u8, u8, u8),
        body: usize,
        random: Random,
        reports: &mut Vec<String>,
    ) -> i64 {
        let Some(r) = self.body(place.0.into(), body).map(<[u8]>::to_vec) else {
            return 0;
        };
        if r[planet::OWNER] != 1 || r[planet::COLONY] == 0 {
            return 0;
        }
        let name = self.place_name(place.0, body);
        let on = format!("{name} ");
        let mut pop = i64::from(long(&r, planet::POPULATION));
        let mut morale = i32::from(r[planet::MORALE] as i8);
        let mut disease = r[planet::DISEASE] as i8;
        let tax = i32::from(r[planet::TAX] as i8);
        let mut wealth = i32::from(r[planet::WEALTH] as i8);
        let count = |state: &GameState, kind: u8| i64::from(state.working(place, kind));
        let living = count(self, colony::HOUSING) * 7000 + 10_000;
        let food = count(self, 21) * 20_000 + count(self, 16) * 8000 + 20_000;
        let medicine = count(self, 24);
        let hospital = count(self, 15) * 20_000 + 20_000;
        let university = count(self, 13);
        let mut target = (7 - tax) * 10;
        if disease > 0 && disease < 100 {
            disease -= 1;
        }
        let factor = if disease == 0 { 2 } else { 1 };
        let say = |random: Random, reports: &mut Vec<String>, chance: u16, text: String| {
            if random(chance) == 0 {
                reports.push(text);
            }
        };
        if disease == 0 && medicine == 0 {
            say(random, reports, 5, format!("Because of disease your people need a medicine plant|  on {on}"));
            target = (target - 10).min(25);
        }
        if factor * pop > hospital {
            say(random, reports, 5, format!(" Due to sanitary conditions you need a hospital |  on {on}"));
            target = (target - 10).min(25);
            pop -= 200;
        }
        for (more, less) in [(30_000, 10), (60_000, 10), (100_000, 20), (150_000, 20)] {
            if pop > more {
                target -= less;
            }
        }
        for kind in [8, 9, 10, 14] {
            if count(self, kind) > 0 {
                target += 10;
            }
        }
        if pop > 50_000 && count(self, 9) == 0 {
            say(random, reports, 5, format!("  Your people ask for a stadium | on {on}"));
            target -= 20;
        }
        if pop > 70_000 && university == 0 {
            say(random, reports, 5, format!("  Your people demand a University | on {on}"));
            target -= 20;
        }
        if place.1 <= 2 && count(self, 19) == 0 {
            if random(5) == 0 {
                let dead = i64::from(random((pop / 4).clamp(0, 0xffff) as u16)) + 500;
                pop -= dead;
                self.start_invention_timer(30, 25, 50, random);
                reports.push(format!(" Your people are dying because of radiation |  on {on}"));
            }
            target -= 40;
        }
        if disease == 100 && self.int(0x6354) < 5 && random(10) == 0 {
            let mut hit = None;
            for n in self.buildings_at(place) {
                let b = &self.buildings()[n - 1];
                if b[building::CONSTRUCTION] == 0 && b[building::TYPE] != colony::COMMAND_CENTRE && random(10) == 0 {
                    hit = Some(n);
                }
            }
            if let Some(n) = hit {
                self.remove_building(n);
                pop -= i64::from(random((pop / 20).clamp(0, 0xffff) as u16));
                self.start_invention_timer(28, 100, 250, random);
                morale -= 10;
                reports.push(format!(
                    "  One of your buildings has exploded |      due to meteorite impact| on {on}"
                ));
            } else {
                reports.push(format!("  A meteorite hit your colony on | {name} |        but causes no harm."));
            }
            target -= 30;
        }
        if pop > food {
            say(random, reports, 3, format!(" Your people are starving |  on {on}"));
            target = (target - 15).min(25);
            morale -= 2;
        }
        if pop > living {
            say(random, reports, 3, format!(" There isn't enough living space|  on {on}"));
            target = (target - 10).min(25);
            morale -= 2;
        }
        let target = target.max(0);
        // How many people the colony can hold.
        let mut most = (i64::from(morale) + 200) * living / 230;
        if pop > food {
            most = most.min(food + 5000);
        }
        most = most.min(hospital / factor + 5000).min(200_000);
        if morale < 30 {
            most /= 2;
        }
        if morale < 20 {
            most = most.min(pop / 2);
        }
        if morale < 10 {
            most = 500;
        }
        if university == 0 {
            most = most.min(80_000);
        }
        let income = (pop as f64 * f64::from(tax) * f64::from(morale + 40) * f64::from(wealth) / 200_000.0).round() as i64;
        if i32::from(random((wealth * wealth).clamp(0, 0xffff) as u16)) < 100 {
            wealth += 1;
        }
        wealth = wealth.min(60);
        let most = most.max(1);
        let growth = if most > pop {
            (pop * 1000 / most) * (most - pop) / most
        } else {
            (most - pop) * 1000 / pop.max(1)
        };
        pop += growth;
        morale += (target - morale) / 10;
        if morale < target {
            morale += 1;
        }
        if morale > target {
            morale -= 3;
        }
        let morale = morale.clamp(0, 69);
        let mut revolt = long(&r, planet::REVOLT);
        let mut lost = false;
        if morale < 10 {
            revolt = (revolt + 1).min(5);
            if revolt == 5 && random(3) < 2 {
                revolt = 4;
            }
            if revolt == 5 {
                reports.push(format!(" Your people have revolted  |  on {on}"));
                lost = true;
            } else {
                reports.push(format!(" Your advisors report that| there are rebellious forces |  on {on}"));
            }
        } else {
            revolt = 0;
        }
        if let Some(r) = self.planet_mut(place.0.into(), body) {
            set_long(r, planet::POPULATION, pop.clamp(i64::from(i32::MIN), i64::from(i32::MAX)) as i32);
            r[planet::MORALE] = morale as u8;
            r[planet::DISEASE] = disease as u8;
            r[planet::WEALTH] = wealth as u8;
            set_long(r, planet::REVOLT, revolt);
        }
        if !lost && pop < 1000 {
            reports.push(format!(" Your colony has been destroyed  |  on {on}"));
            lost = true;
        }
        if lost {
            self.lose_colony(place, body);
        } else {
            self.update_colony(exe, place, body);
        }
        income
    }

    /// FUN_1b8a_0cba's satellite part: satellites, droids and colonies see
    /// more of a planet each day; at 10 you learn whether it has ores, at
    /// 30 whether people can live there (or that aliens do), at 40 whose
    /// colony it is. A colony that landed gets set up after some days.
    fn observation_day(
        &mut self,
        exe: &GameExe,
        texts: &Texts,
        place: (u8, u8, u8),
        body: usize,
        random: Random,
        reports: &mut Vec<String>,
    ) {
        let Some(r) = self.body(place.0.into(), body).map(<[u8]>::to_vec) else {
            return;
        };
        let name = self.place_name(place.0, body);
        let owner = r[planet::OWNER];
        let mut satellites = r[planet::SATELLITES] as i8;
        if satellites < 0 {
            satellites = satellites.saturating_add(10);
        }
        let mut seen = 0i32;
        if satellites > 0 {
            seen = i32::from(random(5)) + 2;
        }
        if r[planet::DROIDS] != 0 && owner == 1 {
            seen = i32::from(random(5)) + 3;
        }
        if r[planet::COLONY] != 0 && owner == 1 {
            seen = i32::from(random(5)) + 4;
        }
        let race = RACE + RACE_LEN * u16::from(owner);
        if owner > 1 && self.byte(race + 0x1b).unwrap_or(0) == 0 && r[planet::OBSERVED] as i8 > 30 {
            seen = 0;
        }
        let observed = (i32::from(r[planet::OBSERVED] as i8) + seen).min(60);
        let crossed = |at: i32| observed - seen < at && observed >= at;
        if crossed(10) && owner < 2 {
            let what = if r[planet::HAS_ORES] == 0 {
                "is not suitable for mining"
            } else {
                "is ready for mining"
            };
            reports.push(format!(" {name}  |    {what} "));
            if self.int(0x5e91) == 0 && self.int(0x91a8) == -1 && r[planet::HAS_ORES] != 0 {
                self.start_invention_timer(5, 40, 20, random);
                reports.push(texts.message(2));
            }
        }
        if crossed(30) {
            if owner < 2 {
                let terrain = exe.terrain_for_type(r[planet::TYPE]).unwrap_or(0);
                let living = r[planet::HABITABLE] != 0 && exe.habitable(terrain).unwrap_or(false);
                let what = if living {
                    "is a suitable place for living"
                } else {
                    "is not suitable for living"
                };
                reports.push(format!(" {name}  |    {what}"));
            } else {
                reports.push(format!(" Your satellite reports alien activity on | {name} "));
            }
        }
        if crossed(40) && owner > 1 {
            let race_name = self.block(0x6bce).and_then(|_| {
                self.races().into_iter().nth(usize::from(owner).checked_sub(2)?).map(|r| r.name)
            });
            reports.push(format!(
                " Your satellite found {} colony at | {name} ",
                race_name.unwrap_or_default()
            ));
        }
        let mut setup = false;
        if let Some(r) = self.planet_mut(place.0.into(), body) {
            r[planet::SATELLITES] = satellites as u8;
            r[planet::OBSERVED] = observed as u8;
            if r[planet::COLONY] == 0 && r[planet::FOUNDING] as i8 > 0 {
                r[planet::FOUNDING] -= 1;
                setup = r[planet::FOUNDING] == 0;
            }
        }
        if setup {
            self.set_up_colony(exe, place, body, random);
            reports.push(format!("  The building of a colony started  | on {name} "));
        }
    }

    /// The colony landed at a place is set up: the setup counters start,
    /// the first building goes up, and the settlers move in.
    fn set_up_colony(&mut self, exe: &GameExe, place: (u8, u8, u8), body: usize, random: Random) {
        for n in 1..=self.word(SETUPS).unwrap_or(0).min(10) {
            let at = SETUP + 16 * n;
            let here = (self.byte(at + 1), self.byte(at + 2), self.byte(at + 3));
            if here != (Some(place.0), Some(place.1), Some(place.2)) {
                continue;
            }
            self.set_byte(at, 1);
            let mut hours = random(10) + 50;
            for k in 1..=6u16 {
                if self.int(at + 2 + 2 * k) != 0 {
                    self.set_word(at + 2 + 2 * k, hours);
                    hours += 5 + random(5);
                }
            }
        }
        // FUN_2ef2_29b1(place, 1): the command centre goes up first.
        self.add_unplaced_building(exe, colony::COMMAND_CENTRE, place, random);
        let settlers = i32::from(random(3000)) + 2000;
        if let Some(r) = self.planet_mut(place.0.into(), body) {
            r[planet::FOUNDING] = 0;
            r[planet::COLONY] = 1;
            let pop = long(r, planet::POPULATION) + settlers;
            set_long(r, planet::POPULATION, pop);
            r[planet::WEALTH] = 20;
            r[planet::TAX] = 3;
            r[planet::MORALE] = 30;
            r[planet::SATELLITES] = 0;
        }
    }

    /// FUN_1b8a_0cba's last part: a hidden planet of a known system may be
    /// found, by an observatory there (visibility -2 or -3), by your ships
    /// there (-2), or with the Psy-radar (-4). One a day at most.
    fn discovery_day(
        &mut self,
        place: (u8, u8, u8),
        stars: &[String],
        random: Random, found: &mut bool, reports: &mut Vec<String>) {
        let (s, p, m) = place;
        if m != 0 || self.byte(SYSTEM_KNOWN + u16::from(s)) != Some(1) {
            return;
        }
        let at = VISIBILITY + 8 * u16::from(s) + u16::from(p);
        let visibility = self.byte(at).unwrap_or(0) as i8;
        let star = stars.get(usize::from(s) - 1).cloned().unwrap_or_default();
        let observatories = self
            .buildings()
            .iter()
            .filter(|b| b[1] == s && b[building::TYPE] == 6 && b[building::CONSTRUCTION] == 0)
            .count();
        if observatories > 0 && (visibility == -2 || visibility == -3) && random(50) == 1 && !*found {
            self.set_byte(at, 0);
            reports.push(format!(" Your observatory found a new planet |          in {star} system"));
            *found = true;
            return;
        }
        if visibility == -2 && random(5) == 1 && !*found {
            let mut who = None;
            for n in 1..=self.unit_count(UnitList::Groups) {
                if let Some(g) = self.unit(UnitList::Groups, n)
                    && g[0x13] == s
                    && g[0x16] > 2
                {
                    who = Some(if g[0] == 4 { "satellite carrier" } else { "pilots" });
                }
            }
            if let Some(who) = who {
                self.set_byte(at, 0);
                reports.push(format!(" Your {who} found a new planet |        in {star} system "));
                *found = true;
                return;
            }
        }
        if self.invention_word(34, 0x11) == 5 && visibility == -4 && random(10) == 1 && !*found {
            self.set_byte(at, 0);
            reports.push(format!(" Your Psy-radar radar found a new planet |          in {star} system"));
            *found = true;
        }
    }
}
