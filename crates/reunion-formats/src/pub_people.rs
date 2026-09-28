//! The people of the pub (DS:0x11f + 27 * n, n 1-10) and their missions
//! (REUNION.PRG FUN_1b8a_006e): someone you hired is away for some hours
//! (the word at 0x10 of their record), and when they're back:
//!
//! - the Stranger (2) reports on a race (FUN_29b9_1c49): its colonies, its
//!   fleets, its garrisons or its weapons;
//! - the Eran (5) has turned the Lisonians or the Undorlings to your side;
//! - the bounty hunter (7) has killed the Earthlings' leader;
//! - the pilot (9) is back from raiding a convoy (see [`TradeOffer`]).
//!
//! The pilot's jobs are ten convoys (DS:0x40c + 24 * k, TEXT/PIRATE.TXT):
//! each can be raided between two dates, which start out as offsets from
//! whenever you first ask the pilot about work. When a convoy's time comes
//! its intercepted message (line k) goes to the log, and in the pub the
//! pilot pitches it (line k + 10).

use crate::exe::GameExe;
use crate::sim::{Random, Texts};
use crate::state::GameState;
use crate::story::Event;

const PEOPLE: u16 = 0x11f;
const OFFERS: u16 = 0x40c;
const OFFER_LEN: u16 = 24;
/// The pilot, in the pub.
pub const PILOT: u16 = 9;
const PERSON_LEN: u16 = 27;
/// The Stranger's mission: 20 * kind + race.
pub const STRANGER_MISSION: u16 = 0x167;

fn person(n: u16, field: u16) -> u16 {
    PEOPLE + PERSON_LEN * n + field
}

/// A convoy the pilot can raid (DS:0x40c + 24 * k, k 1-10).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TradeOffer {
    /// Ore (1-9) or invention (10 + n) per cargo slot, 0 for none.
    pub goods: [u8; 4],
    pub amounts: [u16; 4],
    /// The ship class needed: paying 10,000, 50,000 or 100,000 buys a
    /// better one (levels below 2, 3 or 4).
    pub level: u8,
    /// Chance of success in percent.
    pub chance: u8,
    /// When it can be raided: year, month, day, hour.
    pub from: [u16; 4],
    pub until: [u16; 4],
}

fn offer_date(state: &GameState, at: u16) -> [u16; 4] {
    [
        state.word(at).unwrap_or(0),
        u16::from(state.byte(at + 2).unwrap_or(0)),
        u16::from(state.byte(at + 3).unwrap_or(0)),
        u16::from(state.byte(at + 4).unwrap_or(0)),
    ]
}

impl GameState {
    pub fn trade_offer(&self, k: u16) -> TradeOffer {
        let at = OFFERS + OFFER_LEN * k;
        let byte = |i: u16| self.byte(at + i).unwrap_or(0);
        TradeOffer {
            goods: std::array::from_fn(|i| byte(i as u16)),
            amounts: std::array::from_fn(|i| self.word(at + 4 + 2 * i as u16).unwrap_or(0)),
            level: byte(0xc),
            chance: byte(0xd),
            from: offer_date(self, at + 0xe),
            until: offer_date(self, at + 0x13),
        }
    }

    /// FUN_1fd2_0d75: the convoy that can be raided now (the last of those),
    /// 0 for none.
    pub fn current_offer(&self) -> u16 {
        let now = self.date();
        (1..=10)
            .rev()
            .find(|&k| {
                let offer = self.trade_offer(k);
                offer.from < now && now < offer.until
            })
            .unwrap_or(0)
    }

    /// FUN_1fd2_0c95: the offers' dates count from now (asking the pilot
    /// about work).
    pub fn start_offers(&mut self) {
        let now = self.date();
        for k in 1..=10 {
            let at = OFFERS + OFFER_LEN * k;
            for date in [at + 0xe, at + 0x13] {
                let [mut year, mut month, mut day, mut hour] = offer_date(self, date);
                year = year.wrapping_add(now[0]);
                month += now[1];
                day += now[2];
                hour += now[3];
                if hour > 23 {
                    day += 1;
                    hour -= 24;
                }
                if day > 30 {
                    month += 1;
                    day -= 30;
                }
                if month > 12 {
                    year = year.wrapping_add(1);
                    month -= 12;
                }
                self.set_word(date, year);
                self.set_byte(date + 2, month as u8);
                self.set_byte(date + 3, day as u8);
                self.set_byte(date + 4, hour as u8);
            }
        }
    }

    /// Sends person `n` away for `hours` on mission `mission`.
    pub fn send_person(&mut self, n: u16, hours: u16, mission: u8) {
        self.set_word(person(n, 0x10), hours);
        self.set_byte(person(n, 0x12), mission);
    }

    /// FUN_1b8a_006e.
    pub fn pub_hour(&mut self, exe: &GameExe, texts: &Texts, random: Random) -> Vec<Event> {
        let message = |n: usize| Event::Message(texts.messages.get(n - 1).cloned().unwrap_or_default());
        let mut events = Vec::new();
        for n in 1..=10u16 {
            let status = self.byte(person(n, 0x0f)).unwrap_or(1);
            let hours = self.word(person(n, 0x10)).unwrap_or(0) as i16;
            if !matches!(status, 0 | 2) || hours <= 0 {
                continue;
            }
            self.set_word(person(n, 0x10), (hours - 1) as u16);
            if hours - 1 != 0 {
                continue;
            }
            let mission = self.byte(person(n, 0x12)).unwrap_or(0);
            match n {
                2 => {
                    let code = self.byte(STRANGER_MISSION).unwrap_or(0);
                    let (kind, race) = (code / 20, code % 20);
                    events.push(message(39));
                    events.extend(self.intelligence(exe, kind, race).into_iter().map(Event::Message));
                    let seen = 0x6517 + u16::from(race);
                    if kind == 1 && self.byte(seen) != Some(2) {
                        self.set_byte(seen, 1);
                    }
                    if kind == 3 {
                        self.set_byte(seen, 2);
                    }
                }
                5 => {
                    if mission == 1 {
                        self.set_byte(0x705d, 6);
                    }
                    if mission == 2 {
                        self.set_byte(0x7141, 6);
                    }
                    events.push(message(40));
                    if mission == 1 {
                        events.push(message(43));
                    }
                    if mission == 2 {
                        events.push(message(44));
                        self.give_invention(29);
                        self.make_known(26);
                        self.set_byte(0x17f, 2);
                    }
                }
                7 => {
                    self.set_byte(0x5d9a, 1);
                    self.set_byte(0x1eb, 1);
                    // The Earthlings' home planet loses much of its defence.
                    let losses = [20i32, 0, 1, 2, 0, 10, 10, 10];
                    for (k, loss) in losses.iter().enumerate() {
                        let at = 0x3f8f + 4 * (k as u16 + 1);
                        let v = i32::from(self.word(at).unwrap_or(0)) | i32::from(self.word(at + 2).unwrap_or(0)) << 16;
                        let v = (v - loss).max(0);
                        self.set_word(at, v as u16);
                        self.set_word(at + 2, (v >> 16) as u16);
                    }
                    events.push(message(41));
                }
                _ => {}
            }
            if n == PILOT {
                events.extend(self.pilot_back(exe, texts, mission, random));
            }
            self.set_byte(person(n, 0x12), 0);
        }
        // A convoy's time has come: its intercepted message.
        let now = self.date();
        for k in 1..=10 {
            let offer = self.trade_offer(k);
            if (offer.from[0] as i16) >= 0 && offer.from == now {
                let line = texts.pirate.get(usize::from(k) - 1).cloned().unwrap_or_default();
                events.push(Event::Message(line));
            }
        }
        events
    }

    /// The pilot is back from raiding convoy `mission` (above 10: the free
    /// trip, which brings twice as much).
    fn pilot_back(&mut self, exe: &GameExe, texts: &Texts, mission: u8, random: Random) -> Vec<Event> {
        let message = |n: usize| Event::Message(texts.messages.get(n - 1).cloned().unwrap_or_default());
        let (k, factor) = if mission > 10 { (mission - 10, 2) } else { (mission, 1) };
        let offer = self.trade_offer(u16::from(k));
        let mut events = Vec::new();
        if random(100) < u16::from(offer.chance) {
            events.push(message(0x2f));
            for (&good, &amount) in offer.goods.iter().zip(&offer.amounts) {
                if good == 0 {
                    continue;
                }
                let amount = u32::from(amount) * factor;
                if good < 10 {
                    let at = 0x95c2 + 4 * u16::from(good);
                    let have = u32::from(self.word(at).unwrap_or(0)) | u32::from(self.word(at + 2).unwrap_or(0)) << 16;
                    let total = have.saturating_add(amount);
                    self.set_word(at, total as u16);
                    self.set_word(at + 2, (total >> 16) as u16);
                    let ore = exe.ds_string(0x5a73 + 9 * u16::from(good)).unwrap_or_default();
                    events.push(Event::Message(format!("You got {amount} t of {}", ore.trim_end())));
                } else {
                    let record = 0x5d77 + 0x35 * u16::from(good - 10);
                    let stock = self.word(record + 0x1b).unwrap_or(0);
                    self.set_word(record + 0x1b, stock.wrapping_add(amount as u16));
                    let name = exe.ds_string(record).unwrap_or_default();
                    events.push(Event::Message(format!("You got {amount} pieces of {}s", name.trim_end())));
                }
            }
        } else {
            events.push(message(0x2a));
        }
        // The convoy is gone.
        self.set_word(OFFERS + OFFER_LEN * u16::from(k) + 0x13, 0);
        events
    }

    /// FUN_29b9_1c49: what the Stranger found out about race `race`.
    fn intelligence(&self, exe: &GameExe, kind: u8, race: u8) -> Vec<String> {
        let name = self.race_name(race);
        let ship = |k: u16| exe.ds_string(0x59ea + 10 * k).unwrap_or_default().trim_end().to_string();
        let ground = |k: u16| exe.ds_string(0x5a13 + 9 * k).unwrap_or_default().trim_end().to_string();
        let mut reports = Vec::new();
        match kind {
            1 | 3 => {
                let Ok(systems) = exe.star_systems() else { return reports };
                for (s, layout) in systems.iter().enumerate() {
                    let s = s as u8 + 1;
                    let bodies = layout.moons.iter().enumerate().flat_map(|(p, moons)| {
                        std::iter::once(p + 1).chain(moons.iter().map(|&b| usize::from(b)))
                    });
                    for body in bodies {
                        let Some(r) = self.body(s.into(), body) else { continue };
                        if r[0] != race || r[6] == 0 {
                            continue;
                        }
                        let place = self.place_name(s, body);
                        if kind == 1 {
                            let what = if r[1] == 0 { "    Their colony is on| " } else { "  Their main planet is | " };
                            reports.push(format!("    Information on: {name}|{what}{place}"));
                        } else {
                            let mut text = format!("    Information on: {name}  Their garrison troops are on| {place}: ");
                            for k in 1..=8u16 {
                                if k % 2 == 1 {
                                    text.push('|');
                                }
                                let at = 0x17 + 4 * usize::from(k);
                                let n = i32::from_le_bytes([r[at], r[at + 1], r[at + 2], r[at + 3]]);
                                let label = if k <= 4 { ship(k) } else { ground(k - 4) };
                                text.push_str(&format!("{n:>3}  {label} "));
                            }
                            reports.push(text);
                        }
                    }
                }
            }
            2 => {
                for number in 1..=self.fleet_count(race) {
                    let id = crate::aliens::FleetId { race, number };
                    if self.fleet_byte(id, 0) == 0 || self.fleet_byte(id, 2) == 0 {
                        continue;
                    }
                    let mut text = format!(" Information on: {name} |  The {number}. army power is  ");
                    for k in 1..=8u16 {
                        if k % 2 == 1 {
                            text.push('|');
                        }
                        let label = if k <= 4 { ship(k) } else { ground(k - 4) };
                        text.push_str(&format!("{:>3}  {label}  ", self.fleet_forces(id, k)));
                    }
                    reports.push(text);
                }
            }
            4 => {
                let at = 0x6a06 + 0xe4 * u16::from(race);
                let mut text = format!(" Information on: {name}    Their weapons are |");
                for k in 1..=4u16 {
                    if self.byte(at + 0x12 + k).unwrap_or(0) != 0 {
                        text.push_str(&format!("  {} ", ship(k)));
                    }
                }
                text.push('|');
                for k in 1..=4u16 {
                    if self.byte(at + 0x16 + k).unwrap_or(0) != 0 {
                        text.push_str(&format!("  {}  ", ground(k)));
                    }
                }
                reports.push(text);
            }
            _ => {}
        }
        reports
    }
}

impl GameExe {
    /// FUN_1fd2_05fe: the Stranger's price for mission `reply` (lines
    /// 10-13 of VALASZ2.LOC) on race `race`.
    pub fn stranger_price(&self, race: u8, reply: u8) -> u32 {
        let factor = self.ds_bytes(0x407 + u16::from(race), 1).map_or(0, |b| u32::from(b[0]));
        let price = self
            .ds_bytes(0x3ec + 4 * u16::from(reply), 4)
            .map_or(0, |b| u32::from_le_bytes([b[0], b[1], b[2], b[3]]));
        factor * price
    }
}
