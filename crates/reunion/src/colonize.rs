//! COLONIZATION (screen 28): founding a colony on the planet or moon
//! selected in PLANET INFO.
//!
//! From REUNION.PRG FUN_2841_0060 / 062d: GRAFIKA/KOLONIZ at row 49, six
//! boxes for the buildings a colony may start with (DS:0x651e + 10 * k: the
//! type, the box's corner and its size in cells), each with its price above
//! it (or "Sorry" when your builders can't make it there), and "Cost of
//! colony:" with the total, 100000 plus the chosen buildings. A box toggles
//! its building. OK BUILD IT pays and lands the colony (the settlers arrive
//! in a day or two, the buildings go up over the next days); ABORT goes to
//! the planet's surface.

use bevy::prelude::*;
use bevy::sprite::Anchor;
use reunion_formats::deploy::COLONY_COST;

use crate::audio::Sfx;
use crate::focus::{Activated, Hover, hotspot};
use crate::game::{Game, random};
use crate::game_data::{GameData, GameDataHandle};
use crate::hud::{ActionUsed, CONTENT_Y, HoverLabel, YELLOW_TEXT};
use crate::screen::{GameScreen, picture, place};
use crate::text::{Label, label};
use crate::transition::GoTo;

pub struct ColonizePlugin;

impl Plugin for ColonizePlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(OnEnter(GameScreen::Colonize), enter)
            .add_systems(Update, show_choice.run_if(in_state(GameScreen::Colonize)))
            .add_observer(toggle)
            .add_observer(act);
    }
}

const OK_BUILD_IT: u8 = 59;
const ABORT: u8 = 58;
const TILE: f32 = 16.0;

/// The buildings chosen so far.
#[derive(Resource, Default)]
struct Choice([bool; 6]);

/// Box k (0-5).
#[derive(Component, Clone, Copy)]
struct BuildingBox(usize);

/// A building's tiles in its box.
#[derive(Component)]
struct BoxTile(usize);

#[derive(Component)]
struct Total;

fn enter(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    game: Option<Res<Game>>,
    handle: Res<GameDataHandle>,
    data: Res<Assets<GameData>>,
) {
    commands.insert_resource(Choice::default());
    let scoped = DespawnOnExit(GameScreen::Colonize);
    commands.spawn((picture(asset_server.load("GRAFIKA/KOLONIZ.PIC"), Vec2::new(0.0, CONTENT_Y)), scoped.clone()));
    let (Some(game), Some(data)) = (game, data.get(&handle.0)) else {
        return;
    };
    let (system, _, _) = game.selection();
    let Some(record) = game.selected_body(&data.star_systems).and_then(|b| game.0.body(system.into(), b)) else {
        return;
    };
    let planet_type = record[0x15];
    let terrain = data.terrain_for_type.get(usize::from(planet_type)).copied().unwrap_or(1);
    let sheet = asset_server.load::<Image>(format!("PLANETS/EPUL{terrain}.PIC"));
    let text = |commands: &mut Commands, s: String, columns: usize, pos: Vec2| {
        commands.spawn((label(Label::new(s, columns, YELLOW_TEXT), pos), scoped.clone())).id()
    };
    for (k, setup) in data.exe.setup_buildings().into_iter().enumerate() {
        let Ok(kind) = data.exe.building_type(setup.kind) else { continue };
        let corner = Vec2::new(f32::from(setup.x), f32::from(setup.y));
        let size = Vec2::new(f32::from(setup.width), f32::from(setup.height)) * TILE;
        let available = game.0.setup_building_available(&data.exe, setup.kind, planet_type);
        let price = if available { kind.cost.to_string() } else { "Sorry".into() };
        text(&mut commands, price, 5, corner + Vec2::new(2.0, -11.0));
        commands.spawn((
            BuildingBox(k),
            HoverLabel(kind.name.clone()),
            hotspot(Rect::from_corners(corner, corner + size), Hover::Outline),
            scoped.clone(),
        ));
        for dy in 0..kind.height {
            for dx in 0..kind.width {
                let Some(tile) = kind.tile(dx, dy) else { continue };
                let min = Vec2::new(f32::from(u16::from(tile) % 20), f32::from(u16::from(tile) / 20)) * TILE;
                commands.spawn((
                    BoxTile(k),
                    Sprite {
                        image: sheet.clone(),
                        rect: Some(Rect::from_corners(min, min + TILE)),
                        ..default()
                    },
                    Anchor::TOP_LEFT,
                    place(corner + Vec2::new(f32::from(dx), f32::from(dy)) * TILE, 0.5),
                    Visibility::Hidden,
                    scoped.clone(),
                ));
            }
        }
    }
    text(&mut commands, "Cost of colony:".into(), 15, Vec2::new(180.0, 185.0));
    let total = text(&mut commands, COLONY_COST.to_string(), 6, Vec2::new(276.0, 185.0));
    commands.entity(total).insert(Total);
}

fn cost(data: &GameData, choice: &Choice) -> u32 {
    COLONY_COST
        + data
            .exe
            .setup_buildings()
            .iter()
            .zip(choice.0)
            .filter(|(_, chosen)| *chosen)
            .filter_map(|(b, _)| data.exe.building_type(b.kind).ok())
            .map(|k| k.cost)
            .sum::<u32>()
}

fn show_choice(
    choice: Option<Res<Choice>>,
    handle: Res<GameDataHandle>,
    data: Res<Assets<GameData>>,
    mut tiles: Query<(&BoxTile, &mut Visibility)>,
    mut total: Query<&mut Label, With<Total>>,
) {
    let (Some(data), Some(choice)) = (data.get(&handle.0), choice) else { return };
    for (tile, mut visibility) in &mut tiles {
        *visibility = if choice.0[tile.0] { Visibility::Inherited } else { Visibility::Hidden };
    }
    for label in &mut total {
        Label::set(label, cost(data, &choice).to_string());
    }
}

/// A box: its building is in or out, if your builders can make it there.
fn toggle(
    activated: On<Activated>,
    boxes: Query<&BuildingBox>,
    choice: Option<ResMut<Choice>>,
    game: Option<Res<Game>>,
    handle: Res<GameDataHandle>,
    data: Res<Assets<GameData>>,
    mut commands: Commands,
) {
    let (Ok(&BuildingBox(k)), Some(mut choice), Some(game), Some(data)) =
        (boxes.get(activated.0), choice, game, data.get(&handle.0))
    else {
        return;
    };
    let (system, _, _) = game.selection();
    let Some(record) = game.selected_body(&data.star_systems).and_then(|b| game.0.body(system.into(), b)) else {
        return;
    };
    let Some(setup) = data.exe.setup_buildings().get(k).copied() else { return };
    if game.0.setup_building_available(&data.exe, setup.kind, record[0x15]) {
        choice.0[k] = !choice.0[k];
        commands.trigger(Sfx::named("x"));
    } else {
        commands.trigger(Sfx::named("hiba"));
    }
}

fn act(
    action: On<ActionUsed>,
    screen: Res<State<GameScreen>>,
    choice: Option<Res<Choice>>,
    game: Option<ResMut<Game>>,
    handle: Res<GameDataHandle>,
    data: Res<Assets<GameData>>,
    mut commands: Commands,
) {
    let (GameScreen::Colonize, Some(choice)) = (screen.get(), choice) else {
        return;
    };
    match action.0 {
        ABORT => commands.trigger(GoTo(GameScreen::PlanetMain)),
        OK_BUILD_IT => {
            let (Some(mut game), Some(data)) = (game, data.get(&handle.0)) else {
                return;
            };
            let (system, planet, moon) = game.selection();
            let Some(body) = game.selected_body(&data.star_systems) else { return };
            let place = (system as u8, planet as u8, moon as u8);
            // Not enough money: nothing happens but the error sound.
            if game.0.found_colony(&data.exe, place, body, &choice.0, &mut random) {
                commands.trigger(GoTo(GameScreen::PlanetMain));
            } else {
                commands.trigger(Sfx::named("hiba"));
            }
        }
        _ => {}
    }
}
