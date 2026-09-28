//! MAIN COMPUTER (screen 37): lists of planets in the discovered star systems.
//!
//! From REUNION.PRG FUN_1390_*: COLINFO.PIC (the screen table's background)
//! at row 49; YOUR PLANETS, USEFUL PLANETS and ALIEN PLANETS pick the list
//! (DS:0x7616). Each discovered system's planets and their moons are
//! classified (FUN_1390_0047); a line is a red label at x 10 and the yellow
//! name after it, 15 lines from y 57, 9 apart (FUN_1390_04cc). A moon gets
//! "<system> system planet <planet>" and a second line "moon <moon>".
//! Scrolling moves the first line shown, like the original's scroll bar.

use bevy::prelude::*;
use bevy::sprite::Anchor;
use bevy_enhanced_input::prelude::*;

use crate::audio::Sfx;
use crate::game::Game;
use crate::game_data::{GameData, GameDataHandle};
use crate::hud::{ActionUsed, CONTENT_Y, RED_TEXT, YELLOW_TEXT};
use crate::input::Scroll;
use crate::screen::{GameScreen, picture, place};

pub struct MainComputerPlugin;

impl Plugin for MainComputerPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(OnEnter(GameScreen::MainComputer), enter)
            .add_systems(
                Update,
                update_rows.run_if(in_state(GameScreen::MainComputer)),
            )
            .add_observer(pick_list)
            .add_observer(scroll);
    }
}

const YOUR_PLANETS: u8 = 75;
const USEFUL_PLANETS: u8 = 76;
const ALIEN_PLANETS: u8 = 77;

const ROWS: usize = 15;
const ROW_X: f32 = 10.0;
const ROW_Y: f32 = 48.0;
const ROW_HEIGHT: f32 = 9.0;
/// Names end before x 307 (0x133).
const TEXT_RIGHT: f32 = 307.0;
const GLYPH: f32 = 6.0;

/// Record bytes used to classify (see planet_info for the full list).
const OWNER: usize = 0x00;
const HAS_LIFE: usize = 0x02;
const MINERALS_KNOWN: usize = 0x03;
const COLONY: usize = 0x06;
const COLONISTS: usize = 0x07;
const MINERS: usize = 0x0a;
const SURVEY: usize = 0x0c;
const TYPE: usize = 0x15;
/// A race's relation to you: byte 0x1b of its record.
const RACE_RELATION: usize = 0x1b;
/// Races you have met: DS:0x6517 + owner.
const RACE_KNOWN: u16 = 0x6517;

#[derive(Clone, Copy, PartialEq, Debug)]
enum List {
    Yours,
    Useful,
    Alien,
}

/// The list shown and its first visible line (0-based).
#[derive(Resource)]
struct Listing {
    list: List,
    first: usize,
}

/// A line: system, planet, moon body (0 for the planet itself) and kind
/// (1-3 from the classification, 4 for a moon's second line).
#[derive(Clone, Copy, PartialEq, Debug)]
struct Entry {
    system: usize,
    planet: usize,
    moon: usize,
    kind: u8,
}

#[derive(Component)]
struct RowLabel(usize);

#[derive(Component)]
struct RowName(usize);

fn enter(mut commands: Commands, asset_server: Res<AssetServer>) {
    let scoped = DespawnOnExit(GameScreen::MainComputer);
    commands.insert_resource(Listing {
        list: List::Yours,
        first: 0,
    });
    commands.spawn((
        picture(
            asset_server.load("GRAFIKA/COLINFO.PIC"),
            Vec2::new(0.0, CONTENT_Y),
        ),
        scoped.clone(),
    ));
    for row in 0..ROWS {
        let y = ROW_Y + ROW_HEIGHT * (row + 1) as f32;
        for label in [true, false] {
            let mut entity = commands.spawn((
                Sprite::default(),
                Anchor::TOP_LEFT,
                place(Vec2::new(ROW_X, y), 1.0),
                Visibility::Hidden,
                scoped.clone(),
            ));
            if label {
                entity.insert(RowLabel(row));
            } else {
                entity.insert(RowName(row));
            }
        }
    }
}

fn label(list: List, kind: u8) -> &'static str {
    match (list, kind) {
        (List::Yours, 1) => "Colony on ",
        (List::Yours, 2) => "Miner station on ",
        (List::Useful, 1) => "Accomodable ",
        (List::Useful, 2) => "Mineable   ",
        (List::Alien, 1) => "Aliens on       ",
        (List::Alien, 2) => "Enemy colony on ",
        (List::Alien, 3) => "Friendly colony on ",
        _ => "",
    }
}

/// FUN_1390_0047: which line, if any, a body gets in a list.
fn classify(list: List, record: &[u8], habitable: bool, race_known: bool, relation: u8) -> u8 {
    let owner = record[OWNER];
    let survey = record[SURVEY] as i8;
    let empty = record[COLONY] == 0 && record[COLONISTS] == 0;
    match list {
        List::Yours => {
            if owner == 1 && record[COLONY] == 0 && record[MINERS] != 0 {
                2
            } else if owner == 1 && record[COLONY] != 0 {
                1
            } else {
                0
            }
        }
        List::Useful => {
            let accomodable =
                empty && survey > 29 && record[HAS_LIFE] != 0 && owner < 2 && habitable;
            let mineable = owner == 0
                && empty
                && record[MINERS] == 0
                && survey > 9
                && record[MINERALS_KNOWN] != 0
                && !accomodable;
            if mineable { 2 } else { u8::from(accomodable) }
        }
        List::Alien => {
            if owner < 2 {
                return 0;
            }
            let known = race_known || survey >= 40;
            if known && relation == 6 {
                3
            } else if known && relation == 2 {
                2
            } else if (known && relation == 4) || (!known && survey >= 30) {
                1
            } else {
                0
            }
        }
    }
}

/// FUN_1390_02b6: every line of a list, in system, planet, moon order.
fn entries(list: List, game: &Game, data: &GameData) -> Vec<Entry> {
    let state = &game.0;
    let races = state.races();
    let mut out = Vec::new();
    for (index, layout) in data.star_systems.iter().enumerate() {
        let system = index + 1;
        if !state.system_known(system) {
            continue;
        }
        for (planet_index, moons) in layout.moons.iter().enumerate() {
            let planet = planet_index + 1;
            let bodies =
                std::iter::once((planet, 0)).chain(moons.iter().map(|&m| (m as usize, m as usize)));
            for (body, moon) in bodies {
                let Some(record) = state.body(system, body) else {
                    continue;
                };
                let owner = record[OWNER];
                let habitable = data
                    .terrain_for_type
                    .get(record[TYPE] as usize)
                    .and_then(|t| data.terrains.get(*t as usize))
                    .is_some_and(|t| t.habitable);
                let race_known = state.byte(RACE_KNOWN + owner as u16).unwrap_or(0) != 0;
                let relation = (owner >= 2)
                    .then(|| races.get(owner as usize - 2).map(|r| r.data[RACE_RELATION]))
                    .flatten()
                    .unwrap_or(0);
                let kind = classify(list, record, habitable, race_known, relation);
                if kind != 0 {
                    out.push(Entry {
                        system,
                        planet,
                        moon,
                        kind,
                    });
                    if moon > 0 {
                        out.push(Entry {
                            system,
                            planet,
                            moon,
                            kind: 4,
                        });
                    }
                }
            }
        }
    }
    out
}

fn update_rows(
    game: Option<Res<Game>>,
    listing: Option<Res<Listing>>,
    handle: Res<GameDataHandle>,
    data: Res<Assets<GameData>>,
    mut labels: Query<(&RowLabel, &mut Sprite, &mut Visibility), Without<RowName>>,
    mut names: Query<(&RowName, &mut Sprite, &mut Visibility, &mut Transform), Without<RowLabel>>,
    added: Query<(), Added<RowLabel>>,
    mut images: ResMut<Assets<Image>>,
) {
    let (Some(game), Some(listing), Some(data)) = (game, listing, data.get(&handle.0)) else {
        return;
    };
    if !game.is_changed() && !listing.is_changed() && added.is_empty() {
        return;
    }
    let all = entries(listing.list, &game, data);
    let body_name = |system: usize, body: usize| {
        game.0
            .star_systems()
            .get(system - 1)
            .and_then(|s| s.bodies.get(body - 1))
            .map(|b| b.name.trim_end().to_string())
            .unwrap_or_default()
    };
    for (RowLabel(row), mut sprite, mut visibility) in &mut labels {
        match all.get(listing.first + row).filter(|e| e.kind < 4) {
            Some(entry) => {
                let text = label(listing.list, entry.kind);
                sprite.image = images.add(data.font.render(text, text.len(), RED_TEXT));
                *visibility = Visibility::Inherited;
            }
            None => *visibility = Visibility::Hidden,
        }
    }
    for (RowName(row), mut sprite, mut visibility, mut transform) in &mut names {
        let index = listing.first + row;
        let Some(entry) = all.get(index) else {
            *visibility = Visibility::Hidden;
            continue;
        };
        // A moon's second line lines up with the name above it.
        let label_kind = if entry.kind == 4 {
            index
                .checked_sub(1)
                .and_then(|i| all.get(i))
                .map_or(1, |e| e.kind)
        } else {
            entry.kind
        };
        let x = label(listing.list, label_kind).len() as f32 * GLYPH + ROW_X;
        let system_name = data
            .star_systems
            .get(entry.system - 1)
            .map_or("", |s| s.name.as_str());
        let text = match (entry.kind, entry.moon) {
            (4, moon) => format!("moon {}", body_name(entry.system, moon)),
            (_, 0) => format!(
                "{system_name} system {}",
                body_name(entry.system, entry.planet)
            ),
            _ => format!(
                "{system_name} system planet {}",
                body_name(entry.system, entry.planet)
            ),
        };
        let columns = ((TEXT_RIGHT - x) / GLYPH) as usize;
        sprite.image = images.add(data.font.render(&text, columns, YELLOW_TEXT));
        let y = ROW_Y + ROW_HEIGHT * (row + 1) as f32;
        *transform = place(Vec2::new(x, y), 1.0);
        *visibility = Visibility::Inherited;
    }
}

fn pick_list(action: On<ActionUsed>, listing: Option<ResMut<Listing>>, mut commands: Commands) {
    let Some(mut listing) = listing else { return };
    let (list, sound) = match action.0 {
        YOUR_PLANETS => (List::Yours, "plyour"),
        USEFUL_PLANETS => (List::Useful, "pluseful"),
        ALIEN_PLANETS => (List::Alien, "plalien"),
        _ => return,
    };
    // The entry's voice for the list, or a click when it's already showing.
    commands.trigger(Sfx::named(if listing.list == list { "x" } else { sound }));
    listing.list = list;
    listing.first = 0;
}

fn scroll(
    input: On<Fire<Scroll>>,
    screen: Res<State<GameScreen>>,
    listing: Option<ResMut<Listing>>,
    game: Option<Res<Game>>,
    handle: Res<GameDataHandle>,
    data: Res<Assets<GameData>>,
) {
    let (GameScreen::MainComputer, Some(mut listing), Some(game), Some(data)) =
        (screen.get(), listing, game, data.get(&handle.0))
    else {
        return;
    };
    let count = entries(listing.list, &game, data).len();
    let last_first = count.saturating_sub(ROWS - 1);
    if input.value.y > 0.0 {
        listing.first = listing.first.saturating_sub(1);
    } else if input.value.y < 0.0 {
        listing.first = (listing.first + 1).min(last_first);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn record(owner: u8, colony: u8, miners: u8, survey: u8) -> [u8; 65] {
        let mut r = [0u8; 65];
        r[OWNER] = owner;
        r[COLONY] = colony;
        r[MINERS] = miners;
        r[SURVEY] = survey;
        r
    }

    #[test]
    fn your_planets() {
        assert_eq!(
            classify(List::Yours, &record(1, 1, 2, 100), true, false, 0),
            1
        );
        assert_eq!(
            classify(List::Yours, &record(1, 0, 2, 100), true, false, 0),
            2
        );
        assert_eq!(
            classify(List::Yours, &record(2, 1, 0, 100), true, false, 0),
            0
        );
    }

    #[test]
    fn alien_planets_depend_on_what_you_know() {
        let alien = record(2, 1, 0, 35);
        assert_eq!(classify(List::Alien, &alien, true, false, 0), 1);
        assert_eq!(classify(List::Alien, &alien, true, true, 2), 2);
        assert_eq!(classify(List::Alien, &alien, true, true, 6), 3);
        assert_eq!(
            classify(List::Alien, &record(2, 1, 0, 10), true, false, 0),
            0
        );
    }
}
