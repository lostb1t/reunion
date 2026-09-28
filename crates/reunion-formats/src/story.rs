//! The story's timeline (the second part of REUNION.PRG FUN_1b8a_3d07):
//! hour counters set by earlier events (first contacts, conversations,
//! inventions) that start the next ones: the Jaanosians' calls and their
//! end, the Morgrul invasions, the Kalls' turn, the Antares supernova, the
//! League's and the Earthlings' attacks on New Earth.
//!
//! Each counter is a word in the data segment with a byte beside it that
//! marks it done. What happens is reported as [`Event`]s: message boxes,
//! story pictures (screen 33, PICS/PIC<n>) and conversations with aliens
//! (screen 34).

use crate::aliens::{AT_WAR, FIRST_RACE, FleetId, LAST_RACE};
use crate::exe::GameExe;
use crate::sim::{Random, Texts};
use crate::state::{GameState, UnitList, unit};

/// Something the story shows.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Event {
    /// A message box (also in the log).
    Message(String),
    /// A story picture, PICS/PIC<n> (FUN_2e4b_0000).
    Scene(u8),
    /// A conversation with aliens, TEXT/KERDES<n>.AT (FUN_2e4b_03a5).
    Talk(u8),
}

const NEW_EARTH: (u8, u8, u8) = (1, 5, 0);
/// A ground attack (FUN_29b9_0316's "how").
const GROUND: u8 = 2;

impl GameState {
    fn story_word(&self, at: u16) -> i16 {
        self.word(at).unwrap_or(0) as i16
    }

    fn flag(&self, at: u16) -> bool {
        self.byte(at).unwrap_or(0) != 0
    }

    /// A counter at `at`, done when the byte at `done` is set: counts down
    /// while `going` and returns true when it reaches 0.
    fn countdown(&mut self, at: u16, done: Option<u16>, going: bool) -> bool {
        if done.is_some_and(|d| self.flag(d)) || self.story_word(at) <= 0 || !going {
            return false;
        }
        let left = self.story_word(at) - 1;
        self.set_word(at, left as u16);
        left == 0
    }

    /// Sets a counter unless its event is done.
    fn arm(&mut self, at: u16, done: u16, hours: u16) {
        if !self.flag(done) {
            self.set_word(at, hours);
        }
    }

    fn invention_done(&self, n: u16) -> bool {
        self.word(0x5d77 + 0x35 * n + 0x11) == Some(5)
    }

    /// FUN_29b9_13ab: an invention you're given, ready to produce.
    pub fn give_invention(&mut self, n: u16) {
        let at = 0x5d77 + 0x35 * n;
        if self.word(at + 0x11) == Some(0) {
            self.set_word(at + 0x15, 0);
            self.set_word(at + 0x11, 5);
        }
    }

    fn fleet(race: u8, number: u8) -> FleetId {
        FleetId { race, number }
    }

    /// FUN_29b9_03b3: an attack on New Earth by `race`'s fleet `number` in
    /// `hours` (DS:0x5d40-0x5d46).
    fn schedule_attack(&mut self, race: u8, number: u8, hours: u16, how: u8) {
        self.set_word(0x5d42, race.into());
        self.set_word(0x5d44, number.into());
        self.set_word(0x5d40, hours);
        self.set_word(0x5d46, how.into());
    }

    /// FUN_29b9_0f53: a fleet is gone.
    fn remove_fleet(&mut self, id: FleetId) {
        for field in 0..27 {
            self.set_byte(Self::fleet_address(id, field), 0);
        }
    }

    /// FUN_29b9_0fe4: a race is wiped out: its planets are empty and its
    /// fleets gone.
    fn destroy_race(&mut self, exe: &GameExe, race: u8) {
        let at = 0x6a06 + 0xe4 * u16::from(race);
        for i in 0x0f..=0x12 {
            self.set_byte(at + i, 0);
        }
        self.set_byte(at + 0x1b, 0xff);
        self.set_word(at + 0x1c, 0);
        self.set_word(at + 0x1e, 0);
        for i in [0x20, 0x22, 0x24] {
            self.set_word(at + i, 0xffff);
        }
        for place in self.all_places(exe) {
            if self.body(place.0.into(), place.1).is_some_and(|r| r[0] == race) {
                self.clear_planet(place.0, place.1);
            }
        }
        for number in 1..=self.fleet_count(race) {
            self.remove_fleet(Self::fleet(race, number));
        }
        self.set_byte(at + 0x26, 0);
    }

    /// FUN_29b9_0fe4 for the ground war.
    pub(crate) fn destroy_race_public(&mut self, exe: &GameExe, race: u8) {
        self.destroy_race(exe, race);
    }

    /// Every planet and moon of every system: (system, body).
    fn all_places(&self, exe: &GameExe) -> Vec<(u8, usize)> {
        let Ok(systems) = exe.star_systems() else { return Vec::new() };
        let mut places = Vec::new();
        for (s, layout) in systems.iter().enumerate() {
            for (p, moons) in layout.moons.iter().enumerate() {
                places.push((s as u8 + 1, p + 1));
                places.extend(moons.iter().map(|&b| (s as u8 + 1, usize::from(b))));
            }
        }
        places
    }

    /// FUN_29b9_0217: a planet or moon without owner, colony or forces.
    fn clear_planet(&mut self, system: u8, body: usize) {
        if let Some(r) = self.planet_mut(system.into(), body) {
            r[0] = 0;
            r[1] = 0;
            r[4..6].fill(0);
            r[6] = 0;
            r[7] = 0;
            r[0x0a] = 0;
            r[0x0b] = 0;
            r[0x0d..0x14].fill(0);
            r[0x1b..0x3b].fill(0);
        }
    }

    /// FUN_29b9_06d4: race `race` takes a planet or moon, with a colony and
    /// defences from its tables (DS:0x88f).
    fn take_planet(&mut self, exe: &GameExe, (s, p, m): (u8, u8, u8), race: u8, random: Random) {
        let body = if m == 0 {
            usize::from(p)
        } else {
            let Some(b) = exe
                .star_systems()
                .ok()
                .and_then(|l| l.get(usize::from(s) - 1)?.moons.get(usize::from(p) - 1)?.get(usize::from(m) - 1).copied())
            else {
                return;
            };
            b.into()
        };
        self.clear_planet(s, body);
        let table = |at: u16, k: u16| u16::from(exe.ds_bytes(at + k, 1).map_or(0, |b| b[0]));
        let second_invasion_done = self.flag(0x5d6c);
        let mut defences = [0i32; 8];
        match race {
            3 => {
                let base = if second_invasion_done { 0x88f } else { 0x897 };
                for k in 1..=8u16 {
                    defences[usize::from(k) - 1] = i32::from(table(base, k) + random(table(0x89f, k)));
                }
            }
            7 => {
                for k in 1..=8u16 {
                    defences[usize::from(k) - 1] = i32::from(table(0x8a7, k) + random(table(0x8af, k)));
                }
            }
            // The original writes only the first defence here, eight times.
            8 => defences[0] = i32::from(table(0x8b7, 8) + random(table(0x8bf, 8))),
            12 => defences[0] = i32::from(table(0x8c7, 8) + random(table(0x8cf, 8))),
            _ => {}
        }
        let settlers = i32::from(random(3000)) + 2000;
        if let Some(r) = self.planet_mut(s.into(), body) {
            r[0] = race;
            r[6] = 1;
            r[8] = 0;
            r[9] = 0;
            r[0x0d..0x11].copy_from_slice(&settlers.to_le_bytes());
            r[0x11] = 0x14;
            r[0x12] = 3;
            r[0x13] = 0x1e;
            if matches!(race, 3 | 7 | 8 | 12) {
                for (k, d) in defences.iter().enumerate() {
                    r[0x1b + 4 * k..0x1f + 4 * k].copy_from_slice(&d.to_le_bytes());
                }
            }
        }
    }

    /// FUN_29b9_1179: the Antares supernova. The derricks of system 4 are
    /// gone, its planets and moons empty, the fleets and groups there lost
    /// (groups in orbit or on the ground; bases are disbanded).
    fn supernova(&mut self, exe: &GameExe) {
        let mut n = 1;
        while let Some(b) = self.buildings().get(n - 1).map(|b| b.to_vec()) {
            if b[1] == 4 {
                self.remove_building(n);
            } else {
                n += 1;
            }
        }
        for (s, body) in self.all_places(exe) {
            if s == 4 {
                self.clear_planet(s, body);
            }
        }
        for race in FIRST_RACE..=LAST_RACE {
            for number in 1..=self.fleet_count(race) {
                let id = Self::fleet(race, number);
                if self.fleet_byte(id, 8) == 4 {
                    self.remove_fleet(id);
                }
            }
        }
        let mut n = 1;
        while n <= self.unit_count(UnitList::Groups) {
            let Some(g) = self.unit(UnitList::Groups, n).map(<[u8]>::to_vec) else { break };
            if g[unit::SYSTEM] == 4 {
                if g[unit::STATUS] < 3 {
                    self.disband_unit(UnitList::Groups, n);
                    continue;
                }
                self.start_travel(n, NEW_EARTH, |_| 0);
            }
            n += 1;
        }
        let mut n = 1;
        while n <= self.unit_count(UnitList::Bases) {
            if self.unit(UnitList::Bases, n).is_some_and(|b| b[unit::SYSTEM] == 4) {
                self.disband_unit(UnitList::Bases, n);
            } else {
                n += 1;
            }
        }
    }

    /// FUN_22ca_0105: landing on or leaving certain places, the first time
    /// (while an invention hasn't come up): what's found there.
    pub fn docking_events(&mut self, texts: &Texts, place: (u8, u8, u8), random: Random) -> Vec<Event> {
        let message = |n: usize| Event::Message(texts.messages.get(n - 1).cloned().unwrap_or_default());
        let status = |state: &Self, n: u16| state.word(0x5d77 + 0x35 * n + 0x11).unwrap_or(0);
        let mut events = Vec::new();
        match place {
            // Jade, while the Jaanosians aren't there.
            (1, 7, 0) if self.body(1, 7).is_some_and(|r| r[0] != 2) && status(self, 12) == 0 => {
                events.push(message(0xd));
                self.make_known(0xc);
            }
            (3, 2, 1) if status(self, 22) == 0 => {
                events.push(message(0x15));
                self.make_known(0x16);
                self.make_known(0x17);
                events.push(Event::Scene(4));
            }
            (7, 1, 0) if status(self, 31) == 0 => {
                events.push(message(0x1f));
                self.make_known(0x1f);
                self.make_known(0x20);
                self.start_invention_timer(0x22, 0x14, 100, random);
                events.push(Event::Scene(6));
            }
            _ => {}
        }
        events
    }

    /// The story's hour (FUN_1b8a_3d07 after the invention timers).
    pub fn story_hour(&mut self, exe: &GameExe, texts: &Texts, random: Random) -> Vec<Event> {
        let mut events = Vec::new();
        let message = |n: usize| Event::Message(texts.messages.get(n - 1).cloned().unwrap_or_default());
        let developer_home = self.story_word(0x5d9c) == 0 || self.story_word(0x5d9e) != 4;
        let jaanosians = self.standing(2);
        let communicator = self.invention_done(8);

        // Satellites launched to another planet are shot down; your
        // developers set to work on a satellite carrier.
        if self.countdown(0x5d4a, Some(0x5d4c), true) {
            self.set_byte(0x5d4c, 1);
            self.start_invention_timer(4, 10, 10, random);
            events.push(message(1));
            events.push(Event::Scene(9));
        }
        // The Jaanosians call: their first offer.
        let going = self.word(0x95bc).unwrap_or(0) != 0 && developer_home && jaanosians > 2;
        if self.countdown(0x5d4e, Some(0x5d50), going) {
            events.push(if communicator { Event::Talk(2) } else { message(6) });
        }
        // Your scientist comes back from the Jaanosians with the trade ship.
        if self.countdown(0x5d52, None, true) {
            events.push(message(8));
            self.give_invention(9);
        }
        // Their second call.
        if self.countdown(0x5d54, Some(0x5d56), jaanosians > 2) {
            if communicator {
                events.push(message(9));
                events.push(Event::Talk(3));
            } else {
                events.push(message(6));
            }
            self.set_byte(0x5d56, 1);
        }
        // Their S.O.S.: the Morgruls come for them.
        if self.countdown(0x5d58, Some(0x5d5a), jaanosians == 0 || jaanosians > 2) {
            if communicator {
                events.push(message(10));
                events.push(Event::Talk(4));
            } else {
                events.push(message(6));
            }
            self.set_byte(0x5d5a, 1);
            self.arm(0x5d5c, 0x5d5e, 200 + random(50));
            self.move_fleet(Self::fleet(3, 1), (1, 7, 0), random);
        }
        // The Jaanosians are gone; the Morgruls take Jade and prepare.
        if self.countdown(0x5d5c, Some(0x5d5e), true) {
            if self.standing(2) as i8 > 0 {
                events.push(message(11));
                events.push(Event::Scene(1));
            }
            self.destroy_race(exe, 2);
            self.take_planet(exe, (1, 7, 0), 3, random);
            self.arm(0x5d64, 0x5d66, 600 + random(50));
            self.arm(0x5d60, 0x5d62, 300 + random(50));
            self.arm(0x5d6a, 0x5d6c, 1800 + random(200));
            self.set_byte(0x5d5e, 1);
            self.set_word(0x91ea, 2);
        }
        // The first Morgrul invasion.
        if self.countdown(0x5d64, Some(0x5d66), true) {
            self.send_fleet_to_attack(Self::fleet(3, 1), NEW_EARTH, GROUND, random);
            self.set_byte(0x5d66, 1);
        }
        // What you bought from the Jaanosians turns out to be a hyperdrive.
        let going = self.story_word(0x606e) > 0 && self.word(0x95aa).unwrap_or(0) != 0 && self.story_word(0x5d52) == 0
            && self.story_word(0x5d9e) != 4;
        if self.countdown(0x5d60, Some(0x5d62), going) {
            events.push(message(30));
            self.set_byte(0x5d62, 1);
        }
        // The second invasion.
        if self.countdown(0x5d6a, Some(0x5d6c), true) {
            self.send_fleet_to_attack(Self::fleet(3, 2), NEW_EARTH, GROUND, random);
            self.set_byte(0x5d6c, 1);
            self.set_byte(0x5d6d, 1);
            self.arm(0x5d72, 0x5d74, 1100 + random(50));
            self.arm(0x5d6e, 0x5d70, 100 + random(20));
        }
        // An S.O.S. decoded with the sub-space radio.
        if self.countdown(0x5d6e, Some(0x5d70), self.invention_done(12)) {
            events.push(message(17));
            self.set_byte(0x5d70, 1);
            self.set_byte(0x5d71, 1);
        }
        // The third invasion; the spy's report is coming, and another attack.
        if self.countdown(0x5d72, Some(0x5d74), true) {
            self.send_fleet_to_attack(Self::fleet(3, 3), NEW_EARTH, GROUND, random);
            self.set_byte(0x5d74, 1);
            self.set_byte(0x5d75, 1);
            self.arm(0x5d86, 0x5d88, 50 + random(50));
            let hours = 8000 + random(500);
            self.schedule_attack(3, 6, hours, GROUND);
        }
        // The Kalls move against the Phelonians.
        if self.countdown(0x5d76, Some(0x5d78), true) {
            self.set_byte(0x5d78, 1);
            self.set_byte(0x5d3e, 1);
            self.arm(0x5d7a, 0x5d7c, 4000 + random(50));
            self.move_fleet(Self::fleet(4, 2), (2, 5, 0), random);
        }
        // ... with the Morgruls: the Phelonians fall, and the Kalls turn on
        // you.
        if self.countdown(0x5d7a, Some(0x5d7c), true) {
            self.set_byte(0x5d7c, 1);
            self.set_byte(0x5d3e, 0);
            self.arm(0x5d7e, 0x5d80, 300 + random(50));
            self.set_standing(4, AT_WAR);
            self.move_fleet(Self::fleet(3, 4), (2, 5, 0), random);
            events.push(message(24));
            self.destroy_race(exe, 5);
        }
        if self.countdown(0x5d7e, Some(0x5d80), true) {
            self.set_byte(0x5d80, 1);
            self.send_fleet_to_attack(Self::fleet(3, 4), NEW_EARTH, GROUND, random);
            self.send_fleet_to_attack(Self::fleet(4, 2), NEW_EARTH, GROUND, random);
        }
        // The Phelonians want to trade.
        if self.countdown(0x5d82, Some(0x5d84), self.standing(5) > 2) {
            self.set_byte(0x5d84, 1);
            events.push(Event::Talk(7));
        }
        // The spy's report: a ship near Mirach's second planet.
        if self.countdown(0x5d86, Some(0x5d88), true) {
            self.set_byte(0x5d88, 1);
            events.push(message(20));
        }
        // A new solar system.
        if (self.byte(0x4818).unwrap_or(0) as i8) < 0 && self.countdown(0x5d8a, None, true) {
            self.set_byte(0x4818, 0);
            events.push(message(37));
        }
        // Antares will explode.
        if self.countdown(0x5d92, Some(0x5d94), self.story_word(0x4824) > 0) {
            self.set_byte(0x5d94, 1);
            self.set_byte(0x5d95, 1);
            events.push(message(25));
            self.arm(0x5d8e, 0x5d90, 1000 + random(100));
        }
        if self.countdown(0x5d8e, Some(0x5d90), true) {
            self.set_byte(0x5d90, 1);
            self.set_word(0x5d92, 0);
            events.push(message(27));
            events.push(Event::Scene(7));
            self.supernova(exe);
        }
        // The League attacks.
        if self.countdown(0x5d96, Some(0x5d98), true) {
            self.send_fleet_to_attack(Self::fleet(7, 1), NEW_EARTH, GROUND, random);
            self.set_byte(0x5d98, 1);
            let hours = 1000 + random(500);
            self.schedule_attack(8, 1, hours, GROUND);
        }
        // Scheduled attacks on New Earth, each setting up the next.
        if self.countdown(0x5d40, None, true) {
            let race = self.word(0x5d42).unwrap_or(0) as u8;
            let number = self.word(0x5d44).unwrap_or(0) as u8;
            if self.standing(race) == AT_WAR {
                let id = Self::fleet(race, number);
                self.send_fleet_to_attack(id, NEW_EARTH, GROUND, random);
                let how = self.fleet_byte(id, 7);
                self.set_fleet_byte(id, 0, how);
            }
            self.set_word(0x5d40, 0);
            let next = match (race, number) {
                (3, 6) => Some((3, 7, 4500 + random(100))),
                (7, 6) => Some((7, 7, 3000 + random(500))),
                (7, 5) => Some((7, 6, 1500 + random(500))),
                (8, 1) => Some((7, 5, 1000 + random(500))),
                (12, 4) => Some((12, 5, 2000 + random(500))),
                (12, 3) => Some((12, 4, 2000 + random(500))),
                _ => None,
            };
            if let Some((race, number, hours)) = next {
                self.schedule_attack(race, number, hours, GROUND);
            }
        }
        events
    }
}

impl GameState {
    /// What choosing line `line` of conversation `talk` does (REUNION.PRG's
    /// main loop, screen 34).
    pub fn talk_choice(&mut self, talk: u8, line: u8, random: Random) {
        let money = |state: &mut GameState, change: i64| {
            let money = (i64::from(state.money()) + change).clamp(0, i64::from(u32::MAX)) as u32;
            state.set_word(0x95be, money as u16);
            state.set_word(0x95c0, (money >> 16) as u16);
        };
        // Energon on New Earth, sold to the Phelonians.
        let energon = |state: &GameState| {
            i64::from(state.word(0x95ca).unwrap_or(0)) | i64::from(state.word(0x95cc).unwrap_or(0)) << 16
        };
        let set_energon = |state: &mut GameState, value: i64| {
            state.set_word(0x95ca, value as u16);
            state.set_word(0x95cc, (value >> 16) as u16);
        };
        match (talk, line) {
            // The Jaanosians take your scientist for a few days, or not.
            (2, 4) => {
                self.set_byte(0x5d50, 1);
                let hours = random(30) + 60;
                self.set_word(0x5d52, hours);
            }
            (2, 5 | 6) => {
                self.set_byte(0x5d50, 1);
                self.set_word(0x5d52, 0);
            }
            // Buying their "strange device".
            (3, 6) => {
                self.make_known(14);
                money(self, -16000);
            }
            // The Kalls' gifts.
            (4, 4 | 5) => {
                self.give_invention(10);
                self.give_invention(11);
                self.set_byte(0xa2d6, 1);
            }
            (5, 4) => {
                self.make_known(16);
                let v = self.word(0x60e2).unwrap_or(0).wrapping_add(10);
                self.set_word(0x60e2, v);
            }
            (6, 2) => {
                self.set_byte(0x5d3f, 1);
                self.set_word(0x5d7a, 0);
                self.move_fleet(FleetId { race: 4, number: 2 }, (3, 3, 0), random);
            }
            (6, 5) if !self.flag(0x5d74) && self.story_word(0x5d72) > 0 => {
                let v = self.story_word(0x5d72) + 250;
                self.set_word(0x5d72, v as u16);
            }
            (7, 5 | 8) => {
                let sold = energon(self).min(10_000);
                set_energon(self, energon(self) - sold);
                money(self, sold * if line == 5 { 10 } else { 12 });
            }
            (7, 9) => {
                set_energon(self, energon(self) - energon(self) / 2);
                self.give_invention(21);
            }
            (9, 3) => {
                self.set_byte(0x1b5, 2);
                self.set_byte(0x6f79, 6);
            }
            (10, 6) => {
                self.set_byte(0x481d, 0);
                self.give_invention(35);
                self.set_byte(0x1eb, 2);
            }
            _ => {}
        }
    }

    /// Messages when conversation `talk` ends.
    pub fn talk_end(&self, talk: u8, texts: &Texts) -> Vec<Event> {
        let message = |n: usize| Event::Message(texts.messages.get(n - 1).cloned().unwrap_or_default());
        match talk {
            9 if self.byte(0x1b5) == Some(2) => vec![message(46)],
            6 => vec![message(48)],
            _ => Vec::new(),
        }
    }
}

impl GameExe {
    /// The lines you may say in state `state` of conversation `talk`
    /// (DS:0xcde + 4 * talk: a pointer to 4-byte entries, a count and up to
    /// three line numbers of TEXT/KERDES<talk>.AT).
    pub fn talk_choices(&self, talk: u8, state: u8) -> Vec<u8> {
        let Some(pointer) = self.ds_bytes(0xcde + 4 * u16::from(talk), 4) else {
            return Vec::new();
        };
        let at = u16::from_le_bytes([pointer[0], pointer[1]]);
        if pointer[2..4] == [0, 0] && at == 0 {
            return Vec::new();
        }
        let Some(entry) = self.ds_bytes(at + 4 * u16::from(state), 4) else {
            return Vec::new();
        };
        entry[1..=usize::from(entry[0]).min(3)].to_vec()
    }

    /// Which alien (ALIEN/ALIEN<n>, PICS/SZEK<n>) conversation `talk` is with
    /// (DS:0xd0d + talk).
    pub fn talk_picture(&self, talk: u8) -> u8 {
        self.ds_bytes(0xd0d + u16::from(talk), 1).map_or(1, |b| b[0])
    }
}
