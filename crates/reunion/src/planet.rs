//! PLANET MAIN (screen 20): the surface of the selected planet or moon, and
//! building on your colonies.
//!
//! From REUNION.PRG FUN_2ef2_* and the screen's code in the main loop:
//!
//! - GRAFIKA/DESIGNER at row 49 is the frame. The map shows 16x16 tiles from
//!   x 93, y 53, 14 columns by 9 rows. Terrain tiles below the terrain's
//!   static count come from FELSZ<terrain>, the rest from FANIM<terrain>
//!   (three stacked frames, every 10 VGA frames). The map file is
//!   MAP<type>_<variant> (planet record bytes 0x15 and 0x16).
//! - Buildings (FUN_2ef2_062f / 25fd) are stamped on the map from their
//!   types' tiles (PLANETS/EPUL<terrain>, EPULT<terrain> on alien planets);
//!   buildings under construction show scaffolding (tile 0xdb + phase).
//!   They're seen on your colonies and where you have satellites or spies.
//!   Finished buildings that are switched off blink a frame.
//! - Your colonies: the left panel picks a building type (the arrows; types
//!   you've invented, your builders can make and the planet allows), shows
//!   its name and a picture, and its info (Cost, Production, Workers,
//!   Energy). BUILD then a place on the map builds it; DEMOLISH then a
//!   building removes it for 2000. Clicking a building shows its info; a
//!   finished mine opens RESOURCE-MINE.
//!
//! Not yet: the radar map.

use std::collections::HashMap;

use bevy::prelude::*;
use bevy::sprite::Anchor;
use bevy_enhanced_input::prelude::*;
use reunion_formats::colony::{
    self, BUILDING_TYPES, BuildingType, COMMAND_CENTRE, DERRICK, HOUSING, MINE, MINER_STATION,
    building,
};
use reunion_formats::map::{TILE_SIZE, TILES_PER_ROW};

use crate::focus::{Activated, Hover, hotspot};
use crate::game::{Game, random};
use crate::game_data::{GameData, GameDataHandle, PlanetMap, SharedPictures};
use crate::hud::{ActionUsed, CONTENT_Y, ExtraActions, HoverLabel, RED_TEXT, YELLOW_TEXT};
use crate::input::Scroll;
use crate::popup::ShowMessage;
use crate::screen::{GameScreen, place};
use crate::text::{Label, label};
use crate::transition::GoTo;

pub struct PlanetPlugin;

impl Plugin for PlanetPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(OnEnter(GameScreen::PlanetMain), enter.after(crate::hud::spawn_hud))
            .add_systems(OnExit(GameScreen::PlanetMain), |mut commands: Commands| {
                commands.remove_resource::<Surface>();
            })
            .add_systems(
                Update,
                (
                    animate,
                    layout_view,
                    stamp_buildings,
                    track_hover,
                    update_tiles,
                    update_panel,
                    blink,
                )
                    .chain()
                    .run_if(in_state(GameScreen::PlanetMain).and_then(resource_exists::<Surface>)),
            )
            .add_observer(scroll)
            .add_observer(activate)
            .add_observer(use_action);
    }
}

const VIEW_X: f32 = 93.0;
const VIEW_Y: f32 = 53.0;
const VIEW_ROWS: usize = 9;
const VIEW_COLUMNS: usize = 14;
const TILE: f32 = TILE_SIZE as f32;

const VGA_REFRESH_HZ: f64 = 70.086;
const ANIMATION_FRAMES: u16 = 3;
const VGA_FRAMES_PER_ANIMATION_STEP: f64 = 10.0;

/// Byte offsets in a 65-byte planet record.
const RECORD_OWNER: usize = 0x00;
const RECORD_MINERALS_KNOWN: usize = 0x03;
const RECORD_COLONY: usize = 0x06;
const RECORD_SATELLITES: usize = 0x08;
const RECORD_SPY: usize = 0x09;
const RECORD_MINERS: usize = 0x0a;
const RECORD_POPULATION: usize = 0x0d;
const RECORD_TYPE: usize = 0x15;
const RECORD_VARIANT: usize = 0x16;
const RECORD_ORE: usize = 0x3b;

const PLANET_FORCES: u8 = 0x42;
const SPACEPORT: u8 = 0x2b;
const CONTROL_PANEL: u8 = 0x10;
const DEMOLISH_COST: u32 = 2000;
/// Builders' level (hired builder's level) and hired builder (DS:0x95a6, 0x95b8).
const BUILDER_LEVEL: u16 = 0x95a6;
const BUILDER: u16 = 0x95b8;

/// What clicking the map does (DS:0x78da).
#[derive(Clone, Copy, PartialEq, Debug)]
enum Mode {
    Look,
    /// Placing the chosen type.
    Build,
    /// The chosen type's info.
    TypeInfo,
    /// A building's info.
    BuildingInfo(usize),
    Demolish,
}

/// The surface being shown.
#[derive(Resource)]
struct Surface {
    map: Handle<PlanetMap>,
    felsz: Handle<Image>,
    fanim: Option<Handle<Image>>,
    buildings: Handle<Image>,
    static_tiles: u16,
    terrain: u16,
    /// Height of one FANIM animation frame.
    fanim_frame_rows: u16,
    /// Top-left map cell in view; set once the map has loaded.
    scroll: Option<IVec2>,
    columns: usize,
    frame: u16,
    vga_frames: f64,
    place: (u8, u8, u8),
    body: usize,
    planet_type: u8,
    /// Your colony: building allowed (DS:0x78ea).
    own: bool,
    /// Buildings shown at all.
    visible: bool,
    mode: Mode,
    /// The type in the panel (DS:0x78dc).
    kind: u8,
    /// Map cell -> (sheet tile, or None for blocked ground; building number).
    layer: HashMap<(usize, usize), (Option<u16>, usize)>,
    /// The buildings the layer was stamped from.
    stamped: Option<Vec<Vec<u8>>>,
    /// Focused map cell, for the ghost while building.
    hover: Option<(usize, usize)>,
}

#[derive(Component)]
struct Frame;

#[derive(Component)]
struct Tile(usize, usize);

#[derive(Component)]
struct BuildingTile(usize, usize);

/// A map cell hotspot.
#[derive(Component)]
struct Cell(usize, usize);

#[derive(Component, Clone, Copy, PartialEq)]
enum Button {
    Build,
    Demolish,
    TypeUp,
    TypeDown,
    TypeInfo,
    Left,
    Right,
    Up,
    Down,
}

/// Panel texts, redrawn on changes.
#[derive(Component, Clone)]
struct PanelPart;

#[derive(Component)]
struct Blink(Rect, Rect);

fn enter(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    game: Option<ResMut<Game>>,
    handle: Res<GameDataHandle>,
    data: Res<Assets<GameData>>,
    mut transitions: MessageReader<bevy::state::state::StateTransitionEvent<GameScreen>>,
    mut extra: ResMut<ExtraActions>,
) {
    let (Some(mut game), Some(data)) = (game, data.get(&handle.0)) else {
        warn!("planet screen opened without a game or game data");
        return;
    };
    let from = transitions.read().last().and_then(|t| t.exited);
    let (system, planet, moon) = game.selection();
    let Some(body) = game.selected_body(&data.star_systems) else {
        return;
    };
    let Some(record) = game.0.body(system as usize, body).map(<[u8]>::to_vec) else {
        warn!("no body {body} in system {system}");
        return;
    };
    let here = (system as u8, planet as u8, moon as u8);
    let (planet_type, variant) = (record[RECORD_TYPE], record[RECORD_VARIANT]);
    let terrain_number = data.terrain_for_type.get(planet_type as usize).copied().unwrap_or(1);
    let terrain = data.terrains.get(terrain_number as usize).copied().unwrap_or_default();
    let owner = record[RECORD_OWNER];
    let own = owner == 1 && record[RECORD_COLONY] != 0 && terrain.habitable;
    let visible = (owner == 1 && (record[RECORD_COLONY] != 0 || record[RECORD_MINERS] != 0))
        || (record[RECORD_SATELLITES] as i8) > 0
        || record[RECORD_SPY] != 0;
    let alien = if owner > 1 { "T" } else { "" };
    // FUN_357b_3cc0 runs when the colony is looked at.
    if owner == 1 {
        game.0.update_colony(&data.exe, here, body);
    }
    // The icon bar: PLANET FORCES on your colonies, CONTROL PANEL from there.
    if owner == 1 && record[RECORD_COLONY] != 0 {
        extra.0.push(PLANET_FORCES);
    }
    if from == Some(GameScreen::ControlPanel) {
        extra.0.push(CONTROL_PANEL);
    }
    commands.insert_resource(Surface {
        map: asset_server.load(format!("MAP/MAP{planet_type}_{variant}.MAP")),
        felsz: asset_server.load(format!("PLANETS/FELSZ{terrain_number}.PIC")),
        fanim: (terrain.fanim_rows > 0)
            .then(|| asset_server.load(format!("PLANETS/FANIM{terrain_number}.PIC"))),
        buildings: asset_server.load(format!("PLANETS/EPUL{alien}{terrain_number}.PIC")),
        static_tiles: terrain.static_tiles,
        terrain: terrain_number,
        fanim_frame_rows: terrain.fanim_rows / ANIMATION_FRAMES,
        scroll: None,
        columns: 0,
        frame: 0,
        vga_frames: 0.0,
        place: here,
        body,
        planet_type,
        own,
        visible,
        mode: Mode::Look,
        kind: first_kind(&game, data, &record).unwrap_or(COMMAND_CENTRE),
        layer: HashMap::new(),
        stamped: None,
        hover: None,
    });
    let scoped = DespawnOnExit(GameScreen::PlanetMain);
    commands.spawn((
        Frame,
        Sprite::default(),
        Anchor::TOP_LEFT,
        place(Vec2::new(0.0, CONTENT_Y), 0.0),
        scoped.clone(),
    ));
    let buttons = [
        (Button::Build, Rect::new(0.0, 50.0, 44.0, 63.0), "Build"),
        (Button::Demolish, Rect::new(45.0, 50.0, 89.0, 63.0), "Destroy"),
        (Button::TypeUp, Rect::new(0.0, 64.0, 11.0, 96.0), "Invention up "),
        (Button::TypeDown, Rect::new(0.0, 97.0, 11.0, 129.0), "Invention down"),
        (Button::TypeInfo, Rect::new(12.0, 66.0, 89.0, 125.0), "Invention Info"),
        (Button::Left, Rect::new(89.0, 53.0, 93.0, 197.0), "Left"),
        (Button::Right, Rect::new(317.0, 53.0, 320.0, 197.0), "Right"),
        (Button::Up, Rect::new(93.0, 49.0, 317.0, 53.0), "Up"),
        (Button::Down, Rect::new(93.0, 197.0, 317.0, 200.0), "Down"),
    ];
    for (button, rect, name) in buttons {
        let building_button = !matches!(button, Button::Left | Button::Right | Button::Up | Button::Down);
        if building_button && !own {
            continue;
        }
        commands.spawn((button, HoverLabel(name.into()), hotspot(rect, Hover::Outline), scoped.clone()));
    }
    // BUILD / DEMOLISH blink red while in use (FUN_2ef2_15de).
    for (normal, red, at) in [
        (Rect::new(106.0, 23.0, 150.0, 36.0), Rect::new(150.0, 23.0, 194.0, 36.0), 0.0),
        (Rect::new(106.0, 36.0, 150.0, 49.0), Rect::new(150.0, 36.0, 194.0, 49.0), 45.0),
    ] {
        commands.spawn((
            Blink(normal, red),
            Sprite {
                image: asset_server.load("GRAFIKA/DESIGNER.PIC"),
                rect: Some(normal),
                ..default()
            },
            Anchor::TOP_LEFT,
            place(Vec2::new(at, 50.0), 0.6),
            Visibility::Hidden,
            scoped.clone(),
        ));
    }
}

/// The types that can be built here, in order (the arrows cycle through them).
fn buildable(game: &Game, data: &GameData, record: &[u8]) -> Vec<u8> {
    let level = game.0.word(BUILDER_LEVEL).unwrap_or(0);
    (1..=BUILDING_TYPES)
        .filter(|&t| {
            // Mines need known minerals; derricks some ore too.
            if t == MINE && record[RECORD_MINERALS_KNOWN] == 0 {
                return false;
            }
            if t == DERRICK && (record[RECORD_MINERALS_KNOWN] == 0 || record[RECORD_ORE] <= 9) {
                return false;
            }
            let Ok(kind) = data.exe.building_type(t) else {
                return false;
            };
            let known = if kind.invention == 0 {
                i32::from(kind.builder_level) <= i32::from(level)
            } else {
                game.0.word(0x5d77 + 0x35 * u16::from(kind.invention) + 0x11) == Some(5)
            };
            known && kind.terrain.get(record[RECORD_TYPE] as usize).is_some_and(|&v| v != 0)
        })
        .collect()
}

fn first_kind(game: &Game, data: &GameData, record: &[u8]) -> Option<u8> {
    buildable(game, data, record).first().copied()
}

fn animate(time: Res<Time>, mut surface: ResMut<Surface>) {
    if surface.fanim.is_none() {
        return;
    }
    surface.vga_frames += time.delta_secs_f64() * VGA_REFRESH_HZ;
    if surface.vga_frames >= VGA_FRAMES_PER_ANIMATION_STEP {
        surface.vga_frames %= VGA_FRAMES_PER_ANIMATION_STEP;
        surface.frame = (surface.frame + 1) % ANIMATION_FRAMES;
    }
}

/// Shows the frame and builds the grids of tile sprites once the map has loaded.
fn layout_view(
    mut commands: Commands,
    pictures: Res<SharedPictures>,
    maps: Res<Assets<PlanetMap>>,
    mut surface: ResMut<Surface>,
    mut frame: Single<&mut Sprite, With<Frame>>,
    tiles: Query<Entity, Or<(With<Tile>, With<BuildingTile>, With<Cell>)>>,
) {
    let columns = VIEW_COLUMNS;
    if surface.columns == columns {
        return;
    }
    let Some(map) = maps.get(&surface.map) else {
        return;
    };
    frame.image = pictures.planet_frame.clone();
    for tile in &tiles {
        commands.entity(tile).try_despawn();
    }
    let scoped = DespawnOnExit(GameScreen::PlanetMain);
    for row in 0..VIEW_ROWS {
        for column in 0..columns {
            let pos = Vec2::new(VIEW_X + column as f32 * TILE, VIEW_Y + row as f32 * TILE);
            commands.spawn((Tile(column, row), Sprite::default(), Anchor::TOP_LEFT, place(pos, 0.5), scoped.clone()));
            commands.spawn((
                BuildingTile(column, row),
                Sprite::default(),
                Anchor::TOP_LEFT,
                place(pos, 0.55),
                Visibility::Hidden,
                scoped.clone(),
            ));
            commands.spawn((
                Cell(column, row),
                HoverLabel("Terrain".into()),
                hotspot(Rect::from_corners(pos, pos + TILE), Hover::Outline),
                scoped.clone(),
            ));
        }
    }
    let (width, height) = (map.0.width as i32, map.0.height as i32);
    let start = surface.scroll.unwrap_or(IVec2::new(width / 2 - VIEW_COLUMNS as i32 / 2, height / 2 - 4));
    surface.scroll = Some(clamp_scroll(start, columns, width, height));
    surface.columns = columns;
}

fn clamp_scroll(scroll: IVec2, columns: usize, width: i32, height: i32) -> IVec2 {
    let max = IVec2::new(width - columns as i32, height - VIEW_ROWS as i32).max(IVec2::ZERO);
    scroll.clamp(IVec2::ZERO, max)
}

/// FUN_2ef2_03e7 / 062f / 25fd: the building layer, whenever the buildings
/// here change.
fn stamp_buildings(
    mut surface: ResMut<Surface>,
    game: Option<ResMut<Game>>,
    handle: Res<GameDataHandle>,
    data: Res<Assets<GameData>>,
    maps: Res<Assets<PlanetMap>>,
) {
    let (Some(mut game), Some(data)) = (game, data.get(&handle.0)) else {
        return;
    };
    let Some(map) = maps.get(&surface.map) else { return };
    let numbers = game.0.buildings_at(surface.place);
    let here: Vec<Vec<u8>> = numbers.iter().map(|&n| game.0.buildings()[n - 1].to_vec()).collect();
    if surface.stamped.as_ref() == Some(&here) {
        return;
    }
    let mut layer = HashMap::new();
    // Ground that can't be built on.
    for y in 0..map.0.height {
        for x in 0..map.0.width {
            if let Some(tile) = map.0.tile(x, y)
                && data.exe.tile_blocked(surface.terrain, tile)
            {
                layer.insert((x, y), (None, 0));
            }
        }
    }
    // FUN_2ef2_27a5: buildings put up while nobody was looking (a colony
    // being set up) find their place now.
    let mut taken = layer.clone();
    let cells = |b: &[u8], kind: &BuildingType| {
        let mut cells = Vec::new();
        for dy in 0..kind.height {
            for dx in 0..kind.width {
                if kind.tile(dx, dy).is_some() {
                    cells.push((usize::from(b[building::X] + dx) - 1, usize::from(b[building::Y] + dy) - 1));
                }
            }
        }
        cells
    };
    for b in &here {
        if let Ok(kind) = data.exe.building_type(b[building::TYPE])
            && b[building::X] != 0xff
        {
            for cell in cells(b, &kind) {
                taken.insert(cell, (None, 0));
            }
        }
    }
    for (&n, b) in numbers.iter().zip(&here) {
        let Ok(kind) = data.exe.building_type(b[building::TYPE]) else { continue };
        if b[building::X] != 0xff {
            continue;
        }
        let free = |x: usize, y: usize| {
            (0..kind.height).all(|dy| {
                (0..kind.width).all(|dx| {
                    let cell = (x + usize::from(dx), y + usize::from(dy));
                    kind.tile(dx, dy).is_none()
                        || (cell.0 < map.0.width && cell.1 < map.0.height && !taken.contains_key(&cell))
                })
            })
        };
        // Rings around the middle of the map, top and bottom rows first.
        let (width, height) = (map.0.width as i32, map.0.height as i32);
        let cx = (width + 1 - i32::from(kind.width)) / 2;
        let cy = (height + 1 - i32::from(kind.height)) / 2;
        let fits = |x: i32, y: i32| x >= 1 && y >= 1 && free(x as usize - 1, y as usize - 1);
        let ring = |e: i32| {
            let rows = (cx - e..=cx + e).flat_map(move |x| [(x, cy - e), (x, cy + e)]);
            let columns = (cy - e..=cy + e).flat_map(move |y| [(cx - e, y), (cx + e, y)]);
            rows.chain(columns)
        };
        let Some(spot) = (0..cx.max(1)).flat_map(ring).find(|&(x, y)| fits(x, y)) else {
            continue;
        };
        let spot = (spot.0 as usize - 1, spot.1 as usize - 1);
        if let Some(record) = game.0.building_mut(n) {
            record[building::X] = spot.0 as u8 + 1;
            record[building::Y] = spot.1 as u8 + 1;
            for cell in cells(record, &kind) {
                taken.insert(cell, (None, 0));
            }
        }
    }
    let here: Vec<Vec<u8>> = numbers.iter().map(|&n| game.0.buildings()[n - 1].to_vec()).collect();
    if surface.visible {
        for (&n, b) in numbers.iter().zip(&here) {
            let Ok(kind) = data.exe.building_type(b[building::TYPE]) else { continue };
            if b[building::X] == 0xff {
                continue;
            }
            // FUN_2ef2_062f: scaffolding in three phases while being built.
            let phase = u16::from(b[building::CONSTRUCTION]).div_ceil(40);
            let phase = phase.min(3);
            for dy in 0..kind.height {
                for dx in 0..kind.width {
                    let Some(tile) = kind.tile(dx, dy) else { continue };
                    let sheet = if phase == 0 { u16::from(tile) } else { 0xdb + phase };
                    let cell = (usize::from(b[building::X] + dx) - 1, usize::from(b[building::Y] + dy) - 1);
                    layer.insert(cell, (Some(sheet), n));
                }
            }
        }
    }
    surface.layer = layer;
    surface.stamped = Some(here);
}

fn update_tiles(
    surface: Res<Surface>,
    maps: Res<Assets<PlanetMap>>,
    mut tiles: Query<(&Tile, &mut Sprite), Without<BuildingTile>>,
    mut building_tiles: Query<(&BuildingTile, &mut Sprite, &mut Visibility)>,
    handle: Res<GameDataHandle>,
    data: Res<Assets<GameData>>,
) {
    let (Some(scroll), Some(map), Some(data)) = (surface.scroll, maps.get(&surface.map), data.get(&handle.0)) else {
        return;
    };
    if !surface.is_changed() {
        return;
    }
    let per_row = TILES_PER_ROW as u16;
    let rect = |index: u16, top: u16| {
        let min = Vec2::new(
            ((index % per_row) * TILE_SIZE as u16) as f32,
            (top + (index / per_row) * TILE_SIZE as u16) as f32,
        );
        Rect::from_corners(min, min + TILE)
    };
    for (Tile(column, row), mut sprite) in &mut tiles {
        let (x, y) = (scroll.x as usize + column, scroll.y as usize + row);
        let Some(cell) = map.0.tile(x, y) else {
            sprite.image = Handle::default();
            continue;
        };
        let cell = cell as u16;
        let (image, index, top) = if cell < surface.static_tiles {
            (surface.felsz.clone(), cell, 0)
        } else if let Some(fanim) = &surface.fanim {
            (fanim.clone(), cell - surface.static_tiles, surface.frame * surface.fanim_frame_rows)
        } else {
            (surface.felsz.clone(), 0, 0)
        };
        sprite.image = image;
        sprite.rect = Some(rect(index, top));
    }
    // While building: the chosen type where it would go.
    let ghost = (surface.mode == Mode::Build)
        .then_some(surface.hover)
        .flatten()
        .and_then(|origin| Some((origin, data.exe.building_type(surface.kind).ok()?)));
    for (BuildingTile(column, row), mut sprite, mut visibility) in &mut building_tiles {
        let (x, y) = (scroll.x as usize + column, scroll.y as usize + row);
        let ghost_tile = ghost.as_ref().and_then(|((gx, gy), kind)| {
            let (dx, dy) = (x.checked_sub(*gx)?, y.checked_sub(*gy)?);
            kind.tile(u8::try_from(dx).ok()?, u8::try_from(dy).ok()?)
        });
        let tile = ghost_tile
            .map(u16::from)
            .or_else(|| surface.layer.get(&(x, y)).and_then(|(t, _)| *t));
        match tile {
            Some(t) => {
                sprite.image = surface.buildings.clone();
                sprite.rect = Some(rect(t, 0));
                sprite.color = if ghost_tile.is_some() {
                    Color::srgba(1.0, 1.0, 1.0, 0.7)
                } else {
                    Color::WHITE
                };
                *visibility = Visibility::Inherited;
            }
            None => *visibility = Visibility::Hidden,
        }
    }
}

/// The left panel: the chosen type's name and picture, and the info over
/// the map (FUN_2ef2_19f7, FUN_2ef2_0a9d).
fn update_panel(
    mut commands: Commands,
    surface: Res<Surface>,
    game: Option<Res<Game>>,
    handle: Res<GameDataHandle>,
    data: Res<Assets<GameData>>,
    parts: Query<Entity, With<PanelPart>>,
    mut shown: Local<Option<(u8, Mode, Vec<u8>)>>,
) {
    let (Some(game), Some(data)) = (game, data.get(&handle.0)) else {
        return;
    };
    let record = match surface.mode {
        Mode::BuildingInfo(n) => game.0.buildings().get(n - 1).map(|b| b.to_vec()).unwrap_or_default(),
        _ => Vec::new(),
    };
    let key = (surface.kind, surface.mode, record.clone());
    if shown.as_ref() == Some(&key) && !parts.is_empty() {
        return;
    }
    *shown = Some(key);
    for part in &parts {
        commands.entity(part).despawn();
    }
    if !surface.own {
        return;
    }
    let scoped = (PanelPart, DespawnOnExit(GameScreen::PlanetMain));
    let Ok(kind) = data.exe.building_type(surface.kind) else { return };
    let text = |commands: &mut Commands, s: String, columns: usize, colors, pos: Vec2| {
        commands.spawn((label(Label::new(s, columns, colors), pos), scoped.clone()));
    };
    text(&mut commands, kind.name.clone(), 14, YELLOW_TEXT, Vec2::new(3.0, 130.0));
    // The picture: the type's tiles in the middle of the box.
    let (w, h) = (f32::from(kind.width) * TILE, f32::from(kind.height) * TILE);
    let origin = Vec2::new(12.0 + (64.0 - w) / 2.0, 64.0 + (77.0 - h) / 2.0);
    for dy in 0..kind.height {
        for dx in 0..kind.width {
            let Some(tile) = kind.tile(dx, dy) else { continue };
            let min = Vec2::new(f32::from(u16::from(tile) % 20) * TILE, f32::from(u16::from(tile) / 20) * TILE);
            commands.spawn((
                Sprite {
                    image: surface.buildings.clone(),
                    rect: Some(Rect::from_corners(min, min + TILE)),
                    ..default()
                },
                Anchor::TOP_LEFT,
                place(origin + Vec2::new(f32::from(dx) * TILE, f32::from(dy) * TILE), 0.6),
                scoped.clone(),
            ));
        }
    }
    let lines = match surface.mode {
        Mode::TypeInfo => type_info(data, &kind),
        Mode::BuildingInfo(_) if !record.is_empty() => {
            let planet = game.0.body(surface.place.0 as usize, surface.body).unwrap_or_default();
            let Ok(kind) = data.exe.building_type(record[building::TYPE]) else { return };
            text(&mut commands, kind.name.clone(), 18, YELLOW_TEXT, Vec2::new(117.0, 59.0));
            building_info(data, &kind, &record, planet)
        }
        _ => return,
    };
    if let Mode::TypeInfo = surface.mode {
        text(&mut commands, kind.name.clone(), 18, YELLOW_TEXT, Vec2::new(117.0, 59.0));
    }
    let rows = lines.len();
    commands.spawn((
        Sprite::from_color(Color::BLACK, Vec2::new(130.0, 12.0 + 9.0 * rows as f32)),
        Anchor::TOP_LEFT,
        place(Vec2::new(96.0, 57.0), 0.9),
        scoped.clone(),
    ));
    for (k, line) in lines.into_iter().enumerate() {
        let mut t = commands.spawn((
            label(Label::new(line, 21, RED_TEXT), Vec2::new(98.0, 70.0 + 9.0 * k as f32)),
            scoped.clone(),
        ));
        t.insert(place(Vec2::new(98.0, 70.0 + 9.0 * k as f32), 1.0));
    }
}

/// Lines 1-4 of an info panel, by the category's rows (DS:0x183e).
fn rows(data: &GameData, kind: &BuildingType, first: String, production: String, workers: String, energy: String) -> Vec<String> {
    let [p, w, e] = data.exe.building_info_rows(kind.category);
    let mut lines = vec![first, String::new(), String::new(), String::new()];
    for (row, text) in [(p, production), (w, workers), (e, energy)] {
        if row > 0 && (row as usize) < lines.len() + 1 {
            lines[row as usize] = text;
        }
    }
    lines
}

fn type_info(data: &GameData, kind: &BuildingType) -> Vec<String> {
    let production = if matches!(kind.category, 2 | 9) {
        "Production : mine".to_string()
    } else {
        format!("Production : {} kwh", kind.production)
    };
    rows(
        data,
        kind,
        format!("Cost       : {}", kind.cost),
        production,
        format!("Workers    : {}", kind.workers),
        format!("Energy     : {} kwh", kind.energy),
    )
}

fn building_info(data: &GameData, kind: &BuildingType, b: &[u8], planet: &[u8]) -> Vec<String> {
    let active = b[building::ACTIVE] != 0;
    let working = u32::from(b[building::WORKING]);
    let production = if !active {
        "NONE".to_string()
    } else if kind.category == 2 {
        if planet.get(RECORD_ORE).copied().unwrap_or(0) < 10 {
            "NONE".into()
        } else {
            format!("{} t/ptp", planet[RECORD_ORE] / 10)
        }
    } else {
        let made = u32::from(kind.production) * u32::from(b[building::CONDITION]) / 100 * working / 100;
        format!("{made} kwh")
    };
    let word = |at: usize| u16::from_le_bytes([b[at], b[at + 1]]);
    let (workers, energy, percent) = if active {
        (word(building::WORKERS), word(building::ENERGY), working)
    } else {
        (0, 0, 0)
    };
    let mut lines = rows(
        data,
        kind,
        format!("Status     : {}", if active { "Active" } else { "Passive" }),
        format!("Production : {production}"),
        format!("Workers    : {workers}/{}", kind.workers),
        format!("Energy     : {energy} kwh"),
    );
    let [p, w, e] = data.exe.building_info_rows(kind.category);
    let last = p.max(w).max(e).max(0) as usize;
    let line = format!("Working    : {percent}%");
    if last + 1 < lines.len() {
        lines[last + 1] = line;
    } else {
        lines.push(line);
    }
    lines
}

fn blink(time: Res<Time>, surface: Res<Surface>, mut buttons: Query<(&Blink, &mut Sprite, &mut Visibility)>) {
    // Every 10 VGA frames, red and normal (FUN_2ef2_15de).
    let red = ((time.elapsed_secs() * 70.086 / 10.0) as u32).is_multiple_of(2);
    for (i, (blink, mut sprite, mut visibility)) in buttons.iter_mut().enumerate() {
        let on = matches!((i, surface.mode), (0, Mode::Build) | (1, Mode::Demolish));
        visibility.set_if_neq(if on { Visibility::Inherited } else { Visibility::Hidden });
        sprite.rect = Some(if red { blink.1 } else { blink.0 });
    }
}

fn scroll(
    input: On<Fire<Scroll>>,
    screen: Res<State<GameScreen>>,
    maps: Res<Assets<PlanetMap>>,
    surface: Option<ResMut<Surface>>,
) {
    let (GameScreen::PlanetMain, Some(mut surface)) = (screen.get(), surface) else {
        return;
    };
    // Input y is up, map rows go down.
    let v = input.value;
    let step = if v.x.abs() > v.y.abs() {
        IVec2::new(v.x.signum() as i32, 0)
    } else {
        IVec2::new(0, -v.y.signum() as i32)
    };
    scroll_by(&mut surface, &maps, step);
}

fn scroll_by(surface: &mut Surface, maps: &Assets<PlanetMap>, step: IVec2) {
    let (Some(scroll), Some(map)) = (surface.scroll, maps.get(&surface.map)) else {
        return;
    };
    let columns = surface.columns;
    surface.scroll = Some(clamp_scroll(scroll + step, columns, map.0.width as i32, map.0.height as i32));
}

/// Whether the chosen type fits with its top-left at `cell` (FUN_2ef2_22df).
fn fits(surface: &Surface, map: &PlanetMap, kind: &BuildingType, (x, y): (usize, usize)) -> bool {
    (0..kind.height).all(|dy| {
        (0..kind.width).all(|dx| {
            let cell = (x + usize::from(dx), y + usize::from(dy));
            kind.tile(dx, dy).is_none()
                || (cell.0 < map.0.width && cell.1 < map.0.height && !surface.layer.contains_key(&cell))
        })
    })
}

fn activate(
    activated: On<Activated>,
    cells: Query<&Cell>,
    buttons: Query<&Button>,
    surface: Option<ResMut<Surface>>,
    game: Option<ResMut<Game>>,
    handle: Res<GameDataHandle>,
    data: Res<Assets<GameData>>,
    maps: Res<Assets<PlanetMap>>,
    mut commands: Commands,
) {
    let (Some(mut surface), Some(mut game), Some(data)) = (surface, game, data.get(&handle.0)) else {
        return;
    };
    if let Ok(&Cell(column, row)) = cells.get(activated.0) {
        let Some(scroll) = surface.scroll else { return };
        let cell = (scroll.x as usize + column, scroll.y as usize + row);
        click_map(&mut surface, &mut game, data, &maps, cell, &mut commands);
        return;
    }
    let Ok(&button) = buttons.get(activated.0) else { return };
    let say = |commands: &mut Commands, text: &str| commands.trigger(ShowMessage::new(text));
    match button {
        Button::Left => scroll_by(&mut surface, &maps, IVec2::NEG_X),
        Button::Right => scroll_by(&mut surface, &maps, IVec2::X),
        Button::Up => scroll_by(&mut surface, &maps, IVec2::NEG_Y),
        Button::Down => scroll_by(&mut surface, &maps, IVec2::Y),
        Button::TypeUp | Button::TypeDown => {
            let Some(record) = game.0.body(surface.place.0 as usize, surface.body) else { return };
            let kinds = buildable(&game, data, record);
            let at = kinds.iter().position(|&k| k == surface.kind);
            let next = match (button, at) {
                (Button::TypeUp, Some(i)) if i > 0 => kinds.get(i - 1),
                (Button::TypeDown, Some(i)) => kinds.get(i + 1),
                (_, None) => kinds.first(),
                _ => None,
            };
            if let Some(&k) = next {
                surface.kind = k;
                if matches!(surface.mode, Mode::Build | Mode::Demolish) {
                    surface.mode = Mode::Look;
                }
            }
        }
        Button::TypeInfo => surface.mode = Mode::TypeInfo,
        Button::Build => {
            if surface.mode == Mode::Build {
                surface.mode = Mode::Look;
                return;
            }
            let Ok(kind) = data.exe.building_type(surface.kind) else { return };
            // The main loop's BUILD checks.
            let count = |t: u8| {
                game.0
                    .buildings_at(surface.place)
                    .iter()
                    .filter(|&&n| {
                        let b = &game.0.buildings()[n - 1];
                        b[0] == t && b[building::CONSTRUCTION] == 0 && b[building::ACTIVE] != 0
                    })
                    .count()
            };
            let builder = game.0.word(BUILDER).unwrap_or(0);
            if game.0.money() < kind.cost {
                say(&mut commands, " Not enough money! ");
            } else if game.0.word(BUILDER_LEVEL).unwrap_or(0) == 0 {
                say(&mut commands, " You have no builder! ");
            } else if surface.kind == COMMAND_CENTRE {
                say(&mut commands, " You cannot have two command centres built! ");
            } else if surface.kind == MINER_STATION {
                say(&mut commands, " You already have a colony! ");
            } else if kind.needs_builder_plant && count(colony::BUILDER_PLANT) == 0 {
                say(&mut commands, " This building needs a builder plant! ");
            } else if kind.needs_vehicle_plant && count(colony::VEHICLE_PLANT) == 0 {
                say(&mut commands, " This building needs a vehicle plant! ");
            } else if i32::from(kind.builder_level) > i32::from(builder) {
                let name = data.exe.ds_string(0x586a + 0x13 * builder).unwrap_or_default();
                say(&mut commands, &format!("{name} is not able to construct this building"));
            } else {
                surface.mode = Mode::Build;
            }
        }
        Button::Demolish => {
            surface.mode = if surface.mode == Mode::Demolish { Mode::Look } else { Mode::Demolish };
        }
    }
}

fn click_map(
    surface: &mut Surface,
    game: &mut Game,
    data: &GameData,
    maps: &Assets<PlanetMap>,
    cell: (usize, usize),
    commands: &mut Commands,
) {
    let building_here = surface.layer.get(&cell).map(|&(_, n)| n).filter(|&n| n > 0);
    match surface.mode {
        Mode::TypeInfo | Mode::BuildingInfo(_) => surface.mode = Mode::Look,
        Mode::Look => {
            let Some(n) = building_here else { return };
            if !surface.visible {
                return;
            }
            let b = game.0.buildings()[n - 1].to_vec();
            // A finished mine opens RESOURCE-MINE.
            if surface.own && matches!(b[0], MINE | MINER_STATION) && b[building::CONSTRUCTION] == 0 {
                commands.trigger(GoTo(GameScreen::ResourceMine));
                return;
            }
            surface.mode = Mode::BuildingInfo(n);
        }
        Mode::Build => {
            let (Ok(kind), Some(map)) = (data.exe.building_type(surface.kind), maps.get(&surface.map)) else {
                return;
            };
            if !fits(surface, map, &kind, cell) {
                surface.mode = Mode::Look;
                return;
            }
            let (x, y) = ((cell.0 + 1) as u8, (cell.1 + 1) as u8);
            let Some(n) = game.0.add_building(&kind, surface.kind, surface.place, (x, y), surface.planet_type, random)
            else {
                return;
            };
            let money = game.0.money() - kind.cost;
            game.0.set_word(0x95be, money as u16);
            game.0.set_word(0x95c0, (money >> 16) as u16);
            // Housing brings people.
            if surface.kind == HOUSING
                && let Some(r) = game.0.planet_mut(surface.place.0 as usize, surface.body)
            {
                let people = u32::from_le_bytes([r[0xd], r[0xe], r[0xf], r[0x10]]) + 800 + u32::from(random(200));
                r[RECORD_POPULATION..RECORD_POPULATION + 4].copy_from_slice(&people.to_le_bytes());
            }
            game.0.update_colony(&data.exe, surface.place, surface.body);
            surface.mode = Mode::BuildingInfo(n);
        }
        Mode::Demolish => {
            surface.mode = Mode::Look;
            let Some(n) = building_here else { return };
            let b = &game.0.buildings()[n - 1];
            if b[0] == COMMAND_CENTRE || b[building::CONSTRUCTION] != 0 || game.0.money() < DEMOLISH_COST {
                return;
            }
            game.0.remove_building(n);
            let money = game.0.money() - DEMOLISH_COST;
            game.0.set_word(0x95be, money as u16);
            game.0.set_word(0x95c0, (money >> 16) as u16);
            game.0.update_colony(&data.exe, surface.place, surface.body);
        }
    }
}

/// Tracks the focused map cell for the building ghost.
fn track_hover(focus: Res<crate::focus::Focus>, cells: Query<&Cell>, surface: Option<ResMut<Surface>>) {
    let Some(mut surface) = surface else { return };
    let Some(scroll) = surface.scroll else { return };
    let hover = focus
        .0
        .and_then(|e| cells.get(e).ok())
        .map(|Cell(c, r)| (scroll.x as usize + c, scroll.y as usize + r));
    if surface.hover != hover {
        surface.hover = hover;
    }
}

/// SPACEPORT: the bases; PLANET FORCES: this planet's base.
fn use_action(action: On<ActionUsed>, screen: Res<State<GameScreen>>, mut commands: Commands) {
    if *screen.get() != GameScreen::PlanetMain {
        return;
    }
    match action.0 {
        SPACEPORT => commands.trigger(GoTo(GameScreen::ShipInfo)),
        PLANET_FORCES => commands.trigger(GoTo(GameScreen::Group)),
        _ => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scroll_stays_inside_the_map() {
        assert_eq!(clamp_scroll(IVec2::new(-3, 50), 14, 48, 48), IVec2::new(0, 39));
        assert_eq!(clamp_scroll(IVec2::new(40, 2), 14, 48, 48), IVec2::new(34, 2));
    }
}
