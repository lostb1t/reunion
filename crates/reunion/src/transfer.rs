//! TRANSFER (screen 13): moving ore and goods between a trade ship and the
//! planet it's docked at.
//!
//! From REUNION.PRG FUN_2583_0029 (logic), FUN_2583_0494 (set-up), 07a6
//! (hotspots), 08b1 (names) and 09c6 (numbers): GRAFIKA/TRANSFER at row 49,
//! the six ores and the goods you make (DS:0x807 + k: the invention of good
//! k) in the middle, the planet's amounts left and the ship's right.
//!
//! - The ship holds its merchant ships' capacity (DS:0x846 + 4 * kind each);
//!   ore takes one unit of room, goods their weight (DS:0x812 + 4 * k).
//! - The planet stores 1000 of each ore per storage point of its finished
//!   buildings (FUN_357b_3644: types 0x0b 20, 4 and 0x19 10, 5 five). New
//!   Earth keeps its ore in the common stock and goods as the inventions'
//!   stock; elsewhere ore is in the planet's record (+0x1b) and goods in
//!   its base (+0x83).
//! - Only a ship docked at your colony can load or unload. Confirming moves
//!   all (the other button, see `Alternate`, 100); goods one at a time and
//!   only with a spaceport there.

use bevy::prelude::*;
use reunion_formats::state::{UnitList, unit};

use crate::focus::{Activated, AlternateUse, DefaultFocus, Focus, Hover, hotspot};
use crate::game::Game;
use crate::game_data::{GameData, GameDataHandle};
use crate::hud::{CONTENT_Y, HoverLabel, RED_TEXT, YELLOW_TEXT};
use crate::popup::ShowMessage;
use crate::screen::{GameScreen, picture};
use crate::ship_info::SELECTED;
use crate::text::{Label, label};

pub struct TransferPlugin;

impl Plugin for TransferPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Update, spawn_view.run_if(in_state(GameScreen::Transfer)))
            .add_observer(move_all)
            .add_observer(move_some);
    }
}

const GOODS: u16 = 13;
const ORES: usize = 6;
/// The common ore stock (New Earth's): six longs.
const ORE_STOCK: u16 = 0x95c6;
/// Ore in a ship: six longs from its record + 0x6d.
const SHIP_ORE: usize = 0x6d;
/// Goods in a ship or base: words from + 0x85.
const GOODS_AT: usize = 0x83;
const SPACEPORT: u8 = 0x0c;
const WITH_OTHER_BUTTON: u32 = 100;

#[derive(Component, Clone)]
struct ViewPart;

/// Row `k` (1-6 ore, then goods) toward the planet or the ship.
#[derive(Component, Clone, Copy, PartialEq)]
enum Button {
    ToPlanet(usize),
    ToShip(usize),
}

/// Where the ship is and what's there.
struct Place {
    group: usize,
    record: Vec<u8>,
    location: (u8, u8, u8),
    new_earth: bool,
    base: Option<usize>,
    /// The planet or moon's record number in its system.
    body: Option<usize>,
    /// Goods you make: (good number, invention).
    goods: Vec<(u16, u8)>,
    capacity: u32,
    used: u32,
    storage: u32,
    can_load: bool,
}

fn long(bytes: &[u8], at: usize) -> u32 {
    u32::from_le_bytes([bytes[at], bytes[at + 1], bytes[at + 2], bytes[at + 3]])
}

fn word(bytes: &[u8], at: usize) -> u16 {
    u16::from_le_bytes([bytes[at], bytes[at + 1]])
}

fn exe_long(data: &GameData, at: u16) -> u32 {
    data.exe.ds_bytes(at, 4).map_or(0, |b| u32::from_le_bytes([b[0], b[1], b[2], b[3]]))
}

impl Place {
    fn of(game: &Game, data: &GameData) -> Option<Self> {
        let group = game.0.word(SELECTED).unwrap_or(0) as usize;
        let record = game.0.unit(UnitList::Groups, group)?.to_vec();
        let location = (record[unit::SYSTEM], record[unit::PLANET], record[unit::MOON]);
        let (s, p, m) = location;
        let body = if m == 0 {
            Some(p as usize)
        } else {
            data.star_systems
                .get((s as usize).wrapping_sub(1))
                .and_then(|l| l.moons.get((p as usize).wrapping_sub(1)))
                .and_then(|moons| moons.get(m as usize - 1))
                .map(|&b| b as usize)
        };
        let goods = (1..=GOODS)
            .filter_map(|k| {
                let invention = data.exe.ds_bytes(0x807 + k, 1)?[0];
                let known = (invention as i8) > 0
                    && game.0.word(0x5d77 + 0x35 * u16::from(invention) + 0x11) == Some(5);
                known.then_some((k, invention))
            })
            .collect();
        // FUN_2583_0494: room from the merchant ships, used by ore and goods.
        let capacity = (1..=4u16)
            .map(|k| u32::from(word(&record, unit::SLOTS + (k as usize - 1) * unit::KIND_LEN)) * exe_long(data, 0x846 + 4 * k))
            .sum();
        let used = (1..=GOODS)
            .map(|k| u32::from(word(&record, GOODS_AT + 2 * k as usize)) * exe_long(data, 0x812 + 4 * k))
            .sum::<u32>()
            + (0..ORES).map(|i| long(&record, SHIP_ORE + 4 * i)).sum::<u32>();
        // FUN_357b_3644: storage points of the finished buildings there.
        let storage = game
            .0
            .buildings()
            .iter()
            .filter(|b| (b[1], b[2], b[3]) == location && b[6] == 0)
            .map(|b| match b[0] {
                0x0b => 20,
                0x04 | 0x19 => 10,
                0x05 => 5,
                _ => 0,
            })
            .sum::<u32>()
            * 1000;
        let planet = body.and_then(|b| game.0.body(s as usize, b));
        let can_load = record[unit::STATUS] == 1
            && planet.is_some_and(|p| p[0] == 1 && (p[6] != 0 || p[10] != 0));
        Some(Self {
            group,
            new_earth: location == (1, 5, 0),
            base: game.0.base_at(s, p, m),
            body,
            record,
            location,
            goods,
            capacity,
            used,
            storage,
            can_load,
        })
    }

    fn planet_ore(&self, game: &Game, i: usize) -> u32 {
        if self.new_earth {
            let at = ORE_STOCK + 4 * i as u16;
            u32::from(game.0.word(at).unwrap_or(0)) | u32::from(game.0.word(at + 2).unwrap_or(0)) << 16
        } else {
            self.body
                .and_then(|b| game.0.body(self.location.0 as usize, b))
                .map_or(0, |r| long(r, 0x1b + 4 * i))
        }
    }

    fn set_planet_ore(&self, game: &mut Game, i: usize, value: u32) {
        if self.new_earth {
            let at = ORE_STOCK + 4 * i as u16;
            game.0.set_word(at, value as u16);
            game.0.set_word(at + 2, (value >> 16) as u16);
        } else if let Some(r) = self.body.and_then(|b| game.0.planet_mut(self.location.0 as usize, b)) {
            r[0x1b + 4 * i..0x1f + 4 * i].copy_from_slice(&value.to_le_bytes());
        }
    }

    fn planet_goods(&self, game: &Game, k: u16, invention: u8) -> i16 {
        if self.new_earth {
            game.0.word(0x5d77 + 0x35 * u16::from(invention) + 0x1b).unwrap_or(0) as i16
        } else {
            self.base
                .and_then(|b| game.0.unit(UnitList::Bases, b))
                .map_or(0, |r| word(r, GOODS_AT + 2 * k as usize) as i16)
        }
    }

    fn add_planet_goods(&self, game: &mut Game, k: u16, invention: u8, delta: i16) {
        if self.new_earth {
            let at = 0x5d77 + 0x35 * u16::from(invention) + 0x1b;
            let v = game.0.word(at).unwrap_or(0) as i16;
            game.0.set_word(at, v.wrapping_add(delta) as u16);
        } else if let Some(r) = self.base.and_then(|b| game.0.unit_mut(UnitList::Bases, b)) {
            let at = GOODS_AT + 2 * k as usize;
            let v = word(r, at) as i16;
            r[at..at + 2].copy_from_slice(&v.wrapping_add(delta).to_le_bytes());
        }
    }
}

fn spawn_view(
    mut commands: Commands,
    game: Option<Res<Game>>,
    handle: Res<GameDataHandle>,
    data: Res<Assets<GameData>>,
    asset_server: Res<AssetServer>,
    parts: Query<Entity, With<ViewPart>>,
    focus: Res<Focus>,
    buttons: Query<&Button>,
    mut shown: Local<Option<Vec<u8>>>,
) {
    let (Some(game), Some(data)) = (game, data.get(&handle.0)) else {
        return;
    };
    let Some(place) = Place::of(&game, data) else {
        return;
    };
    // Redraw when anything shown changes.
    let mut key = place.record.clone();
    key.extend((0..ORES).flat_map(|i| place.planet_ore(&game, i).to_le_bytes()));
    key.extend(place.goods.iter().flat_map(|&(k, inv)| place.planet_goods(&game, k, inv).to_le_bytes()));
    if parts.iter().next().is_some() && shown.as_ref() == Some(&key) {
        return;
    }
    *shown = Some(key);
    let focused = focus.0.and_then(|e| buttons.get(e).ok()).copied();
    for part in &parts {
        commands.entity(part).despawn();
    }
    let scoped = (ViewPart, DespawnOnExit(GameScreen::Transfer));
    commands.spawn((
        picture(asset_server.load("GRAFIKA/TRANSFER.PIC"), Vec2::new(0.0, CONTENT_Y)),
        scoped.clone(),
    ));
    let text = |commands: &mut Commands, s: String, columns: usize, colors, pos: Vec2| {
        commands.spawn((label(Label::new(s, columns, colors), pos), scoped.clone()));
    };
    let mut rows: Vec<(String, u32, u32)> = (0..ORES)
        .map(|i| {
            let name = data.exe.ds_string(0x5a73 + 9 * (i as u16 + 1)).unwrap_or_default();
            (name, place.planet_ore(&game, i), long(&place.record, SHIP_ORE + 4 * i))
        })
        .collect();
    for &(k, invention) in &place.goods {
        let name = data.exe.ds_string(0x5d77 + 0x35 * u16::from(invention)).unwrap_or_default();
        let planet = place.planet_goods(&game, k, invention).max(0) as u32;
        rows.push((name, planet, u32::from(word(&place.record, GOODS_AT + 2 * k as usize))));
    }
    for (row, (name, planet, ship)) in rows.iter().enumerate() {
        let y = 61.0 + 8.0 * row as f32;
        text(&mut commands, name.clone(), 16, YELLOW_TEXT, Vec2::new(112.0, y));
        text(&mut commands, format!("{planet:>6}"), 6, YELLOW_TEXT, Vec2::new(12.0, y));
        text(&mut commands, format!("{ship:>6}"), 6, YELLOW_TEXT, Vec2::new(270.0, y));
        if place.can_load {
            for (button, x, name) in [
                (Button::ToPlanet(row + 1), 68.0, "Transfer to planet"),
                (Button::ToShip(row + 1), 235.0, "Transfer to ship"),
            ] {
                let mut entity = commands.spawn((
                    button,
                    HoverLabel(name.into()),
                    hotspot(Rect::new(x, y, x + 16.0, y + 7.0), Hover::Outline),
                    scoped.clone(),
                ));
                if focused == Some(button) {
                    entity.insert(DefaultFocus);
                }
            }
        }
    }
    text(
        &mut commands,
        format!("Space: {:>6}/{}", place.used, place.capacity),
        25,
        YELLOW_TEXT,
        Vec2::new(5.0, 193.0),
    );
    text(
        &mut commands,
        format!("Storage space on planet: {}", place.storage),
        31,
        if place.can_load { YELLOW_TEXT } else { RED_TEXT },
        Vec2::new(128.0, 193.0),
    );
}

fn move_all(
    activated: On<Activated>,
    buttons: Query<&Button>,
    game: Option<ResMut<Game>>,
    handle: Res<GameDataHandle>,
    data: Res<Assets<GameData>>,
    mut commands: Commands,
) {
    if let (Ok(&button), Some(mut game), Some(data)) = (buttons.get(activated.0), game, data.get(&handle.0)) {
        transfer(&mut game, data, button, u32::MAX, &mut commands);
    }
}

fn move_some(
    used: On<AlternateUse>,
    buttons: Query<&Button>,
    game: Option<ResMut<Game>>,
    handle: Res<GameDataHandle>,
    data: Res<Assets<GameData>>,
    mut commands: Commands,
) {
    if let (Ok(&button), Some(mut game), Some(data)) = (buttons.get(used.0), game, data.get(&handle.0)) {
        transfer(&mut game, data, button, WITH_OTHER_BUTTON, &mut commands);
    }
}

/// FUN_2583_0029: moves up to `most` of a row.
fn transfer(game: &mut Game, data: &GameData, button: Button, most: u32, commands: &mut Commands) {
    let Some(place) = Place::of(game, data) else { return };
    let (row, to_ship) = match button {
        Button::ToPlanet(r) => (r, false),
        Button::ToShip(r) => (r, true),
    };
    let set_ship_long = |game: &mut Game, at: usize, value: u32| {
        if let Some(r) = game.0.unit_mut(UnitList::Groups, place.group) {
            r[at..at + 4].copy_from_slice(&value.to_le_bytes());
        }
    };
    if row <= ORES {
        let i = row - 1;
        let at = SHIP_ORE + 4 * i;
        let ship = long(&place.record, at);
        let planet = place.planet_ore(game, i);
        // Both sides are also limited by the planet's storage.
        let amount = if to_ship {
            planet
                .min(most)
                .min(place.capacity.saturating_sub(place.used))
                .min(place.storage.saturating_sub(ship))
        } else {
            ship.min(most).min(place.storage.saturating_sub(planet))
        };
        if to_ship {
            set_ship_long(game, at, ship + amount);
            place.set_planet_ore(game, i, planet - amount);
        } else {
            set_ship_long(game, at, ship - amount);
            place.set_planet_ore(game, i, planet + amount);
        }
        return;
    }
    let Some(&(k, invention)) = place.goods.get(row - ORES - 1) else {
        return;
    };
    let (s, p, m) = place.location;
    let spaceports = game
        .0
        .buildings()
        .iter()
        .filter(|b| b[0] == SPACEPORT && (b[1], b[2], b[3]) == (s, p, m) && b[6] == 0)
        .count();
    if spaceports == 0 {
        commands.trigger(ShowMessage::new(" You need a spaceport to transfer items "));
        return;
    }
    let at = GOODS_AT + 2 * k as usize;
    let ship = word(&place.record, at);
    let weight = exe_long(data, 0x812 + 4 * k);
    let set_ship = |game: &mut Game, value: u16| {
        if let Some(r) = game.0.unit_mut(UnitList::Groups, place.group) {
            r[at..at + 2].copy_from_slice(&value.to_le_bytes());
        }
    };
    if to_ship {
        if place.planet_goods(game, k, invention) > 0 && weight <= place.capacity.saturating_sub(place.used) {
            set_ship(game, ship + 1);
            place.add_planet_goods(game, k, invention, -1);
        }
    } else if ship > 0 {
        set_ship(game, ship - 1);
        place.add_planet_goods(game, k, invention, 1);
    }
}
