//! The alien races (REUNION.PRG DS:0x6a06 + 0xe4 * race, races 2-12).
//!
//! A race's record: its name (a Pascal string), at 0x11 / 0x12 its home
//! system and planet, at 0x1b how it stands with you (0 not met, 2 at war,
//! 4 met, 6 allied), at 0x26 how many fleets it has, and from 0x27 the
//! fleets, 27 bytes each:
//!
//! - 0: shown on the map when above 1;
//! - 2: 1 a fleet at its place, 2 one on its way (arriving in the hours at
//!   3);
//! - 5: hours until it attacks what's at its place, 7 how (1 space, 2 the
//!   ground, 3 / 4 the same with a warning);
//! - 8, 9, 10: system, planet, moon;
//! - 11: eight words, its ships (hunters, fighters, destroyers, cruisers)
//!   and its ground forces.
//!
//! On a planet or moon an alien race owns, the eight longs from 0x1b are its
//! defences, in the same order.

use crate::exe::GameExe;
use crate::sim::{Random, Texts};
use crate::story::Event;
use crate::state::{GameState, UnitList, unit};

pub const FIRST_RACE: u8 = 2;
pub const LAST_RACE: u8 = 12;
const RACE: u16 = 0x6a06;
const RACE_LEN: u16 = 0xe4;
const STANDING: u16 = 0x1b;
const FLEET_COUNT: u16 = 0x26;
const FLEET: u16 = 0x0c;
const FLEET_LEN: u16 = 0x1b;
pub const NOT_MET: u8 = 0;
pub const AT_WAR: u8 = 2;
pub const MET: u8 = 4;
pub const ALLIED: u8 = 6;

/// A fleet of an alien race (1-based number).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FleetId {
    pub race: u8,
    pub number: u8,
}

impl GameState {
    fn race_address(race: u8) -> u16 {
        RACE + RACE_LEN * u16::from(race)
    }

    pub fn race_name(&self, race: u8) -> String {
        let at = Self::race_address(race);
        let len = self.byte(at).unwrap_or(0).min(14);
        (1..=u16::from(len)).filter_map(|i| self.byte(at + i)).map(char::from).collect()
    }

    /// How race `race` stands with you.
    pub fn standing(&self, race: u8) -> u8 {
        self.byte(Self::race_address(race) + STANDING).unwrap_or(0)
    }

    pub fn set_standing(&mut self, race: u8, standing: u8) {
        self.set_byte(Self::race_address(race) + STANDING, standing);
    }

    pub fn fleet_count(&self, race: u8) -> u8 {
        (self.byte(Self::race_address(race) + FLEET_COUNT).unwrap_or(0) as i8).max(0) as u8
    }

    /// Address of byte `field` of a fleet.
    pub fn fleet_address(id: FleetId, field: u16) -> u16 {
        Self::race_address(id.race) + FLEET + FLEET_LEN * u16::from(id.number) + field
    }

    pub fn fleet_byte(&self, id: FleetId, field: u16) -> u8 {
        self.byte(Self::fleet_address(id, field)).unwrap_or(0)
    }

    pub fn set_fleet_byte(&mut self, id: FleetId, field: u16, value: u8) {
        self.set_byte(Self::fleet_address(id, field), value);
    }

    /// Ships (kinds 1-4) and ground forces (5-8) of a fleet.
    pub fn fleet_forces(&self, id: FleetId, kind: u16) -> u16 {
        self.word(Self::fleet_address(id, 9 + 2 * kind)).unwrap_or(0)
    }

    pub fn set_fleet_forces(&mut self, id: FleetId, kind: u16, value: u16) {
        self.set_word(Self::fleet_address(id, 9 + 2 * kind), value);
    }

    pub fn fleet_place(&self, id: FleetId) -> (u8, u8, u8) {
        (self.fleet_byte(id, 8), self.fleet_byte(id, 9), self.fleet_byte(id, 10))
    }

    /// Every fleet of every race.
    pub fn fleets(&self) -> Vec<FleetId> {
        (FIRST_RACE..=LAST_RACE)
            .flat_map(|race| (1..=self.fleet_count(race)).map(move |number| FleetId { race, number }))
            .collect()
    }

    /// FUN_29b9_0000: meeting race `race` for the first time. The race
    /// decides whether it wants war (the Morgruls, the League and the
    /// Earthlings), friendship (the Kalls and the Syonians) or neither, and
    /// the story moves on. Returns the message boxes.
    pub fn first_contact(&mut self, race: u8, message: String, texts: &Texts, random: Random) -> Vec<Event> {
        let mut reports = Vec::new();
        if race == 0 || self.standing(race) as i8 >= 1 {
            return reports;
        }
        reports.push(Event::Message(message));
        let standing = match race {
            3 | 12 | 7..=10 => AT_WAR,
            4 | 11 => ALLIED,
            _ => MET,
        };
        self.set_standing(race, standing);
        let text = |n: usize| Event::Message(texts.messages.get(n - 1).cloned().unwrap_or_default());
        let invention_unknown = |state: &GameState, n: u16| {
            state.word(0x5d77 + 0x35 * n + 0x11) == Some(0) && state.word(0x919e + 2 * n) == Some(0xffff)
        };
        let timer = |state: &mut GameState, flag: u16, at: u16, hours: u16| {
            if state.byte(flag) == Some(0) {
                state.set_word(at, hours);
            }
        };
        match race {
            2 => {
                if invention_unknown(self, 8) {
                    self.start_invention_timer(8, 20, 20, random);
                    reports.push(text(5));
                }
                let first = 650 + random(70);
                timer(self, 0x5d50, 0x5d4e, first);
                let second = self.word(0x5d4e).unwrap_or(first) + 720 + random(50);
                timer(self, 0x5d56, 0x5d54, second);
                let third = self.word(0x5d54).unwrap_or(second) + 1220 + random(100);
                timer(self, 0x5d5a, 0x5d58, third);
                let fourth = self.word(0x5d58).unwrap_or(third) + 100 + random(20);
                timer(self, 0x5d5e, 0x5d5c, fourth);
            }
            3 if invention_unknown(self, 8) => reports.push(text(5)),
            4 => {
                reports.push(text(15));
                reports.push(Event::Talk(5));
                let hours = 1500 + random(500);
                timer(self, 0x5d78, 0x5d76, hours);
            }
            5 => {
                let hours = 20 + random(20);
                timer(self, 0x5d84, 0x5d82, hours);
            }
            7..=10 if self.byte(0x5d98) == Some(0) && (self.word(0x5d96).unwrap_or(0) as i16) < 1 => {
                reports.push(text(28));
                let hours = 200 + random(50);
                timer(self, 0x5d98, 0x5d96, hours);
            }
            11 => {
                reports.push(text(29));
                reports.push(Event::Talk(10));
            }
            12 => {
                reports.push(text(38));
                self.set_byte(0x74d1, 2);
                // FUN_29b9_03b3: the Earthlings' first attack on New Earth.
                self.set_word(0x5d42, 12);
                self.set_word(0x5d44, 3);
                self.set_word(0x5d40, 1000 + random(200));
                self.set_word(0x5d46, 2);
            }
            _ => {}
        }
        reports
    }

    /// FUN_1b8a_2a0e on a group's arrival: its pilots look at the moons of
    /// a planet nobody had looked at, and meet any race they hadn't met
    /// that lives there or has a fleet there.
    pub fn arrival(&mut self, exe: &GameExe, texts: &Texts, n: usize, random: Random) -> Vec<Event> {
        let mut reports = Vec::new();
        let Some(group) = self.unit(UnitList::Groups, n).map(<[u8]>::to_vec) else {
            return reports;
        };
        let (s, p, m) = (group[unit::SYSTEM], group[unit::PLANET], group[unit::MOON]);
        if p == 0 {
            return reports;
        }
        let star = exe
            .star_systems()
            .ok()
            .and_then(|l| l.get(usize::from(s).wrapping_sub(1)).map(|l| l.name.trim_end().to_string()))
            .unwrap_or_default();
        let visibility = 0x47cd + 8 * u16::from(s) + u16::from(p);
        if self.byte(visibility) == Some(0) {
            self.set_byte(visibility, 1);
            let who = if group[unit::TYPE] == 4 { "satellite carrier" } else { "pilots" };
            reports.push(Event::Message(format!("Your {who} explored {}'s moons", self.place_name(s, p.into()))));
        }
        if group[unit::TYPE] == 4 {
            return reports;
        }
        let body = if m == 0 {
            Some(usize::from(p))
        } else {
            exe.star_systems().ok().and_then(|l| {
                l.get(usize::from(s) - 1)?.moons.get(usize::from(p) - 1)?.get(usize::from(m) - 1).map(|&b| b.into())
            })
        };
        let owner = body.and_then(|b| self.body(s.into(), b)).map_or(0, |r| r[0]);
        if owner > 1 && self.standing(owner) == NOT_MET {
            let message = format!(
                " Your pilots met with a new alien race in {star} |         They call themselves '{}'",
                self.race_name(owner)
            );
            reports.extend(self.first_contact(owner, message, texts, random));
        }
        for id in self.fleets() {
            if self.standing(id.race) == NOT_MET && self.fleet_byte(id, 2) == 1 && self.fleet_place(id) == (s, p, m) {
                let message = format!(
                    " Your pilots met with new alien race in {star} |         They call themselves '{}'",
                    self.race_name(id.race)
                );
                reports.extend(self.first_contact(id.race, message, texts, random));
            }
        }
        reports
    }
}

/// A battle the aliens start (FUN_1b8a_3546).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Attack {
    pub place: (u8, u8, u8),
    /// A ground attack on your colony (after the battle in space).
    pub ground: bool,
    pub race: u8,
}

impl GameState {
    fn body_of(&self, exe: &GameExe, (s, p, m): (u8, u8, u8)) -> Option<usize> {
        if m == 0 {
            return Some(p.into());
        }
        let layouts = exe.star_systems().ok()?;
        layouts.get(usize::from(s).checked_sub(1)?)?.moons.get(usize::from(p).checked_sub(1)?)?.get(usize::from(m) - 1).map(|&b| b.into())
    }

    /// FUN_29b9_0dab: fleet `id` sets off for `place`: a few hours within
    /// the planet's moons, a day within the system, two or three between
    /// systems.
    pub fn move_fleet(&mut self, id: FleetId, place: (u8, u8, u8), random: Random) {
        let from = self.fleet_place(id);
        if from == place {
            return;
        }
        let hours = if from.0 != place.0 {
            50 + random(20)
        } else if from.1 != place.1 {
            20 + random(10)
        } else {
            6 + random(5)
        };
        self.set_word(Self::fleet_address(id, 3), hours);
        self.set_fleet_byte(id, 8, place.0);
        self.set_fleet_byte(id, 9, place.1);
        self.set_fleet_byte(id, 10, place.2);
        self.set_fleet_byte(id, 2, 2);
        self.set_word(Self::fleet_address(id, 5), 0);
        self.set_fleet_byte(id, 7, 0);
    }

    /// FUN_29b9_0316: fleet `id` goes to `place` and attacks there (`how`:
    /// 1 in space, 2 on the ground, 3 / 4 the same, announced) after a
    /// while; the more warlike the race, the sooner.
    pub fn send_fleet_to_attack(&mut self, id: FleetId, place: (u8, u8, u8), how: u8, random: Random) {
        self.move_fleet(id, place, random);
        let warlike = u16::from(self.byte(Self::race_address(id.race) + 0x10).unwrap_or(0));
        let hours = random(100u16.saturating_sub(warlike)) + 5;
        self.set_word(Self::fleet_address(id, 5), hours);
        self.set_fleet_byte(id, 7, how);
    }

    /// FUN_1b8a_3546: the aliens' hour. Fleets on their way arrive (and
    /// meet you, or warn you they're coming); fleets waiting to attack
    /// count down, and one that's ready starts a battle.
    pub fn aliens_hour(&mut self, exe: &GameExe, texts: &Texts, random: Random) -> (Vec<Event>, Option<Attack>) {
        let mut reports = Vec::new();
        let mut attack = None;
        let star = |s: u8| {
            exe.star_systems()
                .ok()
                .and_then(|l| l.get(usize::from(s).wrapping_sub(1)).map(|l| l.name.trim_end().to_string()))
                .unwrap_or_default()
        };
        for race in FIRST_RACE..=LAST_RACE {
            for number in 1..=self.fleet_count(race) {
                let id = FleetId { race, number };
                if self.fleet_byte(id, 2) != 2 {
                    continue;
                }
                let at = Self::fleet_address(id, 3);
                let hours = (self.word(at).unwrap_or(0) as i16 - 1).max(0);
                self.set_word(at, hours as u16);
                if hours != 0 {
                    continue;
                }
                self.set_fleet_byte(id, 2, 1);
                let place = self.fleet_place(id);
                let name = self.body_of(exe, place).map(|b| self.place_name(place.0, b)).unwrap_or_default();
                let mut warn = false;
                let bases: Vec<usize> = (1..=self.unit_count(UnitList::Bases))
                    .filter(|&n| {
                        self.unit(UnitList::Bases, n)
                            .is_some_and(|b| (b[unit::SYSTEM], b[unit::PLANET], b[unit::MOON]) == place)
                    })
                    .collect();
                for _ in bases {
                    match self.standing(race) {
                        NOT_MET => {
                            let message = format!(
                                "   New alien race's ships arrived |     at your colony in {} | They call themselves '{}'",
                                star(place.0),
                                self.race_name(race)
                            );
                            reports.extend(self.first_contact(race, message, texts, random));
                        }
                        AT_WAR => warn = true,
                        _ => {}
                    }
                }
                let groups: Vec<usize> = (1..=self.unit_count(UnitList::Groups))
                    .filter(|&n| {
                        self.unit(UnitList::Groups, n).is_some_and(|g| {
                            g[unit::TYPE] != 4
                                && (g[unit::SYSTEM], g[unit::PLANET], g[unit::MOON]) == place
                                && g[unit::STATUS] < 3
                        })
                    })
                    .collect();
                for _ in groups {
                    match self.standing(race) {
                        NOT_MET => {
                            let message = format!(
                                " Your pilots met with new alien race in |   {name} |     They call themselves '{}'",
                                self.race_name(race)
                            );
                            reports.extend(self.first_contact(race, message, texts, random));
                        }
                        AT_WAR => warn = true,
                        _ => {}
                    }
                }
                if warn {
                    reports.push(Event::Message(format!(
                        "            ATTENTION!         |     Enemy fleet approaching   | {name} "
                    )));
                }
            }
            for number in 1..=self.fleet_count(race) {
                let id = FleetId { race, number };
                let how = self.fleet_byte(id, 7);
                if self.fleet_byte(id, 2) != 1 || how == 0 || attack.is_some() {
                    continue;
                }
                let at = Self::fleet_address(id, 5);
                let hours = self.word(at).unwrap_or(0).saturating_sub(1);
                self.set_word(at, hours);
                if hours != 0 {
                    continue;
                }
                let place = self.fleet_place(id);
                let owner = self.body_of(exe, place).and_then(|b| self.body(place.0.into(), b)).map_or(0, |r| r[0]);
                let ground = matches!(how, 2 | 4) && owner == 1;
                let defended = (1..=self.unit_count(UnitList::Groups)).any(|n| {
                    self.unit(UnitList::Groups, n).is_some_and(|g| {
                        matches!(g[unit::TYPE], 1 | 3) && g[unit::SYSTEM] == place.0 && g[unit::PLANET] == place.1
                    })
                });
                let name = self.body_of(exe, place).map(|b| self.place_name(place.0, b)).unwrap_or_default();
                let colony = format!("| Your colony is under attack by {} |  at {name} |", self.race_name(race));
                if defended {
                    if ground {
                        reports.push(Event::Message(colony));
                    } else {
                        let planet = self.place_name(place.0, place.1.into());
                        reports.push(Event::Message(format!(
                            " Your ships around {planet} |        is under attack by {} ",
                            self.race_name(race)
                        )));
                    }
                    attack = Some(Attack { place, ground, race });
                } else if ground {
                    reports.push(Event::Message(colony));
                    attack = Some(Attack { place, ground, race });
                }
                self.set_fleet_byte(id, 7, 0);
            }
        }
        (reports, attack)
    }

    /// After a battle at `place` (FUN_29b9_03d2 / 0487): the losers leave.
    /// Beaten alien fleets go home (or, at home, join the planet's
    /// defences); your beaten groups fly back to New Earth (or, there,
    /// join its base), and so do your allies' fleets.
    pub fn after_battle(&mut self, exe: &GameExe, (s, p, _): (u8, u8, u8), won: bool, random: Random) {
        let mut retreat = |state: &mut GameState, id: FleetId| {
            let race_at = Self::race_address(id.race);
            let home = (state.byte(race_at + 0x11).unwrap_or(0), state.byte(race_at + 0x12).unwrap_or(0));
            if (s, p) == home {
                // FUN_29b9_0a60: the fleet joins its planet's defences.
                let place = state.fleet_place(id);
                if let Some(body) = state.body_of(exe, place)
                    && state.body(s.into(), body).is_some_and(|r| r[0] == id.race)
                {
                    let forces: Vec<u16> = (1..=8).map(|k| state.fleet_forces(id, k)).collect();
                    if let Some(r) = state.planet_mut(s.into(), body) {
                        for (k, f) in forces.iter().enumerate() {
                            let at = 0x1b + 4 * k;
                            let v = i32::from_le_bytes([r[at], r[at + 1], r[at + 2], r[at + 3]]) + i32::from(*f);
                            r[at..at + 4].copy_from_slice(&v.to_le_bytes());
                        }
                    }
                }
                state.set_fleet_byte(id, 2, 0);
            } else {
                state.move_fleet(id, (home.0, home.1, 0), random);
            }
        };
        let losers = if won { AT_WAR } else { ALLIED };
        for id in self.fleets() {
            let (fs, fp, _) = self.fleet_place(id);
            if self.standing(id.race) == losers
                && self.fleet_byte(id, 2) == 1
                && self.fleet_byte(id, 0) > 1
                && (fs, fp) == (s, p)
            {
                retreat(self, id);
            }
        }
        if !won {
            let mut n = 1;
            while n <= self.unit_count(UnitList::Groups) {
                let Some(g) = self.unit(UnitList::Groups, n).map(<[u8]>::to_vec) else { break };
                if g[unit::STATUS] < 3 && g[unit::SYSTEM] == s && g[unit::PLANET] == p {
                    if (s, p) == (1, 5) {
                        self.disband_unit(UnitList::Groups, n);
                        continue;
                    }
                    self.start_travel(n, (1, 5, 0), &mut *random);
                }
                n += 1;
            }
        }
    }
}

/// Something in orbit on the galactic map's moons view (FUN_357b_18cc).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MapObject {
    Group(usize),
    Fleet(FleetId),
}

/// A place in the map's list of what's in orbit: the object, its icon in
/// HATTER4 (unit type 1-4; 3 + race for aliens), its moon (0: the planet).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MapEntry {
    pub object: MapObject,
    pub icon: u8,
    pub moon: u8,
}

pub const MAP_SLOTS: usize = 18;

impl GameState {
    /// FUN_357b_18cc: what's in orbit around planet `planet` of `system`, in
    /// its 18 places (two columns): your groups, then the aliens' fleets
    /// (each race from the left column) if you can see them: with a colony,
    /// droids or a spy ship there, or ships of yours or your allies'.
    pub fn map_objects(&self, exe: &GameExe, system: u8, planet: u8) -> Vec<Option<MapEntry>> {
        let mut slots: Vec<Option<MapEntry>> = Vec::new();
        for n in 1..=self.unit_count(UnitList::Groups) {
            let Some(g) = self.unit(UnitList::Groups, n) else { continue };
            if g[unit::SYSTEM] == system && g[unit::PLANET] == planet && matches!(g[unit::STATUS], 1..=3) && slots.len() < MAP_SLOTS {
                slots.push(Some(MapEntry { object: MapObject::Group(n), icon: g[unit::TYPE], moon: g[unit::MOON] }));
            }
        }
        let moons = exe
            .star_systems()
            .ok()
            .and_then(|l| l.get(usize::from(system).wrapping_sub(1)).and_then(|l| l.moons.get(usize::from(planet).wrapping_sub(1)).cloned()))
            .unwrap_or_default();
        let watched = std::iter::once(usize::from(planet)).chain(moons.iter().map(|&b| usize::from(b))).any(|body| {
            self.body(system.into(), body).is_some_and(|r| {
                let (owner, colony) = (r[0], r[6] != 0);
                (owner > 1 && colony && self.standing(owner) == ALLIED)
                    || (owner == 1 && colony)
                    || (owner == 1 && r[0x0a] != 0)
                    || r[9] != 0
            })
        });
        let here = |state: &GameState, id: FleetId| {
            let (s, p, _) = state.fleet_place(id);
            s == system && p == planet && state.fleet_byte(id, 2) == 1
        };
        let ships = !slots.is_empty()
            || self.fleets().into_iter().any(|id| self.standing(id.race) == ALLIED && here(self, id));
        if watched || ships {
            for race in FIRST_RACE..=LAST_RACE {
                if slots.len() % 2 == 1 {
                    slots.push(None);
                }
                for number in 1..=self.fleet_count(race) {
                    let id = FleetId { race, number };
                    if here(self, id) && self.fleet_byte(id, 0) > 1 && slots.len() < MAP_SLOTS {
                        slots.push(Some(MapEntry { object: MapObject::Fleet(id), icon: race + 3, moon: self.fleet_byte(id, 10) }));
                    }
                }
            }
        }
        slots.truncate(MAP_SLOTS);
        slots
    }

    /// The name the map gives a fleet: "Jaanosian Fleet 1" (DS:0x5aa8).
    pub fn fleet_name(&self, exe: &GameExe, id: FleetId) -> String {
        let race = exe.ds_string(0x5aa8 + 10 * u16::from(id.race)).unwrap_or_default();
        format!("{race} Fleet {}", id.number)
    }
}
