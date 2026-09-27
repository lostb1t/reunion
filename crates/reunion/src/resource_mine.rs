//! RESOURCE-MINE (screen 4): mining on a planet, from one of its mines.
//!
//! From REUNION.PRG FUN_2527_001f (logic) and FUN_2527_01e5 (drawing):
//! GRAFIKA/MINER at row 49, and from GRAFIKA/SZAMOK:
//!
//! - the droids working (the planet record's byte 0x0a) as a big digit at
//!   (34, 144), one of ten 48x35 frames;
//! - droids in stock (New Earth: the Miner droid invention's stock; else the
//!   base's word 0x9d) in big digits at (112, 144) and (128, 144);
//! - mines there (finished mines and miner stations) in small digits at
//!   (113, 113) and (126, 113);
//! - the ore in stock at (59, 57), 8 apart, and on your working colonies
//!   what the droids dig per ore (droids x ore byte 0x3b + n / 10).
//!
//! ADD DROIDS puts a droid from stock to work, one per mine and nine at
//! most. Anywhere else goes back to the surface.

use bevy::prelude::*;
use bevy::sprite::Anchor;
use reunion_formats::colony::{MINE, MINER_STATION, building};
use reunion_formats::state::UnitList;

use crate::focus::{Activated, Hover, hotspot};
use crate::game::Game;
use crate::game_data::{GameData, GameDataHandle};
use crate::hud::{ActionUsed, CONTENT_Y, HoverLabel, YELLOW_TEXT};
use crate::screen::{GameScreen, picture, place};
use crate::text::{Label, label};
use crate::transition::GoTo;

pub struct ResourceMinePlugin;

impl Plugin for ResourceMinePlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Update, spawn_view.run_if(in_state(GameScreen::ResourceMine)))
            .add_observer(back)
            .add_observer(add_droid);
    }
}

const ADD_DROIDS: u8 = 25;
const NEW_EARTH_DROIDS: u16 = 0x5dfc;
const MAX_DROIDS: u8 = 9;

#[derive(Component, Clone)]
struct ViewPart;

#[derive(Component)]
struct Surface;

/// What the screen looks at.
struct Mining {
    place: (u8, u8, u8),
    body: usize,
    record: Vec<u8>,
    stock: u16,
    mines: usize,
    ores: [u32; 6],
}

impl Mining {
    fn of(game: &Game, data: &GameData) -> Option<Self> {
        let (s, p, m) = game.selection();
        let body = game.selected_body(&data.star_systems)?;
        let record = game.0.body(s as usize, body)?.to_vec();
        let place = (s as u8, p as u8, m as u8);
        let long = |b: &[u8], at: usize| u32::from_le_bytes([b[at], b[at + 1], b[at + 2], b[at + 3]]);
        let (stock, ores) = if place == (1, 5, 0) {
            let ores = std::array::from_fn(|i| {
                let at = 0x95c6 + 4 * i as u16;
                u32::from(game.0.word(at).unwrap_or(0)) | u32::from(game.0.word(at + 2).unwrap_or(0)) << 16
            });
            (game.0.word(NEW_EARTH_DROIDS).unwrap_or(0), ores)
        } else {
            let stock = if record[6] != 0 {
                game.0
                    .base_at(place.0, place.1, place.2)
                    .and_then(|b| game.0.unit(UnitList::Bases, b))
                    .map_or(0, |b| u16::from_le_bytes([b[0x9d], b[0x9e]]))
            } else {
                0
            };
            (stock, std::array::from_fn(|i| long(&record, 0x1b + 4 * i)))
        };
        let mines = game
            .0
            .buildings_at(place)
            .iter()
            .filter(|&&n| {
                let b = &game.0.buildings()[n - 1];
                matches!(b[0], MINE | MINER_STATION) && b[building::CONSTRUCTION] == 0
            })
            .count();
        Some(Self {
            place,
            body,
            record,
            stock,
            mines,
            ores,
        })
    }
}

fn spawn_view(
    mut commands: Commands,
    game: Option<Res<Game>>,
    handle: Res<GameDataHandle>,
    data: Res<Assets<GameData>>,
    asset_server: Res<AssetServer>,
    parts: Query<Entity, With<ViewPart>>,
    mut shown: Local<Option<(Vec<u8>, u16, usize, [u32; 6])>>,
) {
    let (Some(game), Some(data)) = (game, data.get(&handle.0)) else {
        return;
    };
    let Some(mining) = Mining::of(&game, data) else { return };
    let key = (mining.record.clone(), mining.stock, mining.mines, mining.ores);
    if !parts.is_empty() && shown.as_ref() == Some(&key) {
        return;
    }
    *shown = Some(key);
    for part in &parts {
        commands.entity(part).despawn();
    }
    let scoped = (ViewPart, DespawnOnExit(GameScreen::ResourceMine));
    commands.spawn((
        picture(asset_server.load("GRAFIKA/MINER.PIC"), Vec2::new(0.0, CONTENT_Y)),
        scoped.clone(),
    ));
    commands.spawn((
        Surface,
        HoverLabel("Back to surface".into()),
        hotspot(Rect::new(0.0, 49.0, 320.0, 200.0), Hover::Outline),
        scoped.clone(),
    ));
    let numbers = asset_server.load::<Image>("GRAFIKA/SZAMOK.PIC");
    let sprite = |rect: Rect, at: Vec2| {
        (
            Sprite {
                image: numbers.clone(),
                rect: Some(rect),
                ..default()
            },
            Anchor::TOP_LEFT,
            place(at, 0.5),
        )
    };
    let droids = mining.record[0x0a];
    let frame = Vec2::new(f32::from(droids % 5) * 64.0, f32::from(droids / 5) * 35.0);
    commands.spawn((sprite(Rect::from_corners(frame, frame + Vec2::new(48.0, 35.0)), Vec2::new(34.0, 144.0)), scoped.clone()));
    let stock = mining.stock.min(99);
    for (digit, x) in [(stock / 10, 112.0), (stock % 10, 128.0)] {
        let min = Vec2::new(f32::from(digit) * 16.0, 70.0);
        commands.spawn((sprite(Rect::from_corners(min, min + Vec2::new(16.0, 35.0)), Vec2::new(x, 144.0)), scoped.clone()));
    }
    let mines = mining.mines.min(99) as u16;
    for (digit, x) in [(mines / 10, 113.0), (mines % 10, 126.0)] {
        let min = Vec2::new(163.0 + f32::from(digit) * 13.0, 71.0);
        commands.spawn((sprite(Rect::from_corners(min, min + Vec2::new(13.0, 9.0)), Vec2::new(x, 113.0)), scoped.clone()));
    }
    let text = |commands: &mut Commands, s: String, columns: usize, pos: Vec2| {
        commands.spawn((label(Label::new(s, columns, YELLOW_TEXT), pos), scoped.clone()));
    };
    for (i, ore) in mining.ores.iter().enumerate() {
        let shown = if *ore == 0 { "     -".to_string() } else { format!("{ore:>6}") };
        text(&mut commands, shown, 6, Vec2::new(59.0, 57.0 + 8.0 * i as f32));
    }
    let r = &mining.record;
    if r[0] == 1 && (r[6] != 0 || r[0x0a] != 0) {
        for i in 1..=6 {
            let rich = r[0x3a + i];
            let shown = if i == 1 {
                "--".to_string()
            } else if rich < 10 {
                " -".to_string()
            } else {
                format!("{:>2}", u32::from(r[0x0a]) * u32::from(rich) / 10)
            };
            text(&mut commands, shown, 2, Vec2::new(113.0, 57.0 + 8.0 * (i - 1) as f32));
        }
    }
}

fn back(activated: On<Activated>, surface: Query<(), With<Surface>>, mut commands: Commands) {
    if surface.contains(activated.0) {
        commands.trigger(GoTo(GameScreen::PlanetMain));
    }
}

/// ADD DROIDS (FUN_2527_001f).
fn add_droid(
    action: On<ActionUsed>,
    screen: Res<State<GameScreen>>,
    game: Option<ResMut<Game>>,
    handle: Res<GameDataHandle>,
    data: Res<Assets<GameData>>,
) {
    if action.0 != ADD_DROIDS || *screen.get() != GameScreen::ResourceMine {
        return;
    }
    let (Some(mut game), Some(data)) = (game, data.get(&handle.0)) else {
        return;
    };
    let Some(mining) = Mining::of(&game, data) else { return };
    let r = &mining.record;
    let droids = r[0x0a];
    if mining.stock == 0 || usize::from(droids) >= mining.mines || r[6] == 0 || r[0] != 1 || droids >= MAX_DROIDS {
        return;
    }
    if let Some(record) = game.0.planet_mut(mining.place.0 as usize, mining.body) {
        record[0x0a] = droids + 1;
    }
    if mining.place == (1, 5, 0) {
        game.0.set_word(NEW_EARTH_DROIDS, mining.stock - 1);
    } else if let Some(b) = game.0.base_at(mining.place.0, mining.place.1, mining.place.2)
        && let Some(base) = game.0.unit_mut(UnitList::Bases, b)
    {
        base[0x9d..0x9f].copy_from_slice(&(mining.stock - 1).to_le_bytes());
    }
}
