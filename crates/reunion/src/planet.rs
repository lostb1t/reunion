//! PLANET MAIN (screen 20): the surface of the selected planet.
//!
//! From REUNION.PRG FUN_2ef2_004a, _03e7 and _1924: DESIGNER.PIC at row 49 is
//! the frame (BUILD/DEMOLISH panel on the left). The map shows 16x16 tiles
//! from x 93, y 53, 14 columns by 9 rows, starting at column width/2 - 7 and
//! row height/2 - 4. Tiles below the terrain's static count come from
//! FELSZ<terrain>, the rest from FANIM<terrain>, whose three stacked frames
//! cycle every 10 VGA frames. The map file is MAP<type>_<variant>, where type
//! and variant are bytes 0x15 and 0x16 of the planet record.
//!
//! Widescreen: the screen stays the original 4:3, centred; only the HUD is wide.
//! A wider map view can come back once the frame has art to match.

use bevy::prelude::*;
use bevy::sprite::Anchor;
use bevy_enhanced_input::prelude::*;
use reunion_formats::map::{TILE_SIZE, TILES_PER_ROW};

use crate::game::Game;
use crate::game_data::{GameData, GameDataHandle, PlanetMap, SharedPictures};
use crate::hud::CONTENT_Y;
use crate::input::Scroll;
use crate::screen::{GameScreen, place};

pub struct PlanetPlugin;

impl Plugin for PlanetPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(OnEnter(GameScreen::PlanetMain), enter)
            .add_systems(OnExit(GameScreen::PlanetMain), |mut commands: Commands| {
                commands.remove_resource::<Surface>();
            })
            .add_systems(
                Update,
                (animate, layout_view, update_tiles)
                    .chain()
                    .run_if(in_state(GameScreen::PlanetMain).and_then(resource_exists::<Surface>)),
            )
            .add_observer(scroll);
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
const RECORD_TYPE: usize = 0x15;
const RECORD_VARIANT: usize = 0x16;

/// The surface being shown.
#[derive(Resource)]
struct Surface {
    map: Handle<PlanetMap>,
    felsz: Handle<Image>,
    fanim: Option<Handle<Image>>,
    static_tiles: u16,
    /// Height of one FANIM animation frame.
    fanim_frame_rows: u16,
    /// Top-left map cell in view; set once the map has loaded.
    scroll: Option<IVec2>,
    columns: usize,
    frame: u16,
    vga_frames: f64,
}

#[derive(Component)]
struct Frame;

#[derive(Component)]
struct Tile(usize, usize);

fn enter(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    game: Option<Res<Game>>,
    handle: Res<GameDataHandle>,
    data: Res<Assets<GameData>>,
) {
    let (Some(game), Some(data)) = (game, data.get(&handle.0)) else {
        warn!("planet screen opened without a game or game data");
        return;
    };
    let (system, planet, _moon) = game.selection();
    let systems = game.0.star_systems();
    let Some(body) = systems
        .get(system as usize - 1)
        .and_then(|s| s.bodies.get(planet as usize - 1))
    else {
        warn!("no planet {planet} in system {system}");
        return;
    };
    let (planet_type, variant) = (body.data[RECORD_TYPE], body.data[RECORD_VARIANT]);
    let terrain_number = data
        .terrain_for_type
        .get(planet_type as usize)
        .copied()
        .unwrap_or(1);
    let terrain = data
        .terrains
        .get(terrain_number as usize)
        .copied()
        .unwrap_or_default();
    info!(
        "{}: type {planet_type} variant {variant}, terrain {terrain_number}",
        body.name.trim_end()
    );

    commands.insert_resource(Surface {
        map: asset_server.load(format!("MAP/MAP{planet_type}_{variant}.MAP")),
        felsz: asset_server.load(format!("PLANETS/FELSZ{terrain_number}.PIC")),
        fanim: (terrain.fanim_rows > 0)
            .then(|| asset_server.load(format!("PLANETS/FANIM{terrain_number}.PIC"))),
        static_tiles: terrain.static_tiles,
        fanim_frame_rows: terrain.fanim_rows / ANIMATION_FRAMES,
        scroll: None,
        columns: 0,
        frame: 0,
        vga_frames: 0.0,
    });
    commands.spawn((
        Frame,
        Sprite::default(),
        Anchor::TOP_LEFT,
        place(Vec2::new(0.0, CONTENT_Y), 0.0),
        DespawnOnExit(GameScreen::PlanetMain),
    ));
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

/// Shows the frame and builds the grid of tile sprites once the map has loaded.
fn layout_view(
    mut commands: Commands,
    pictures: Res<SharedPictures>,
    maps: Res<Assets<PlanetMap>>,
    mut surface: ResMut<Surface>,
    mut frame: Single<&mut Sprite, With<Frame>>,
    tiles: Query<Entity, With<Tile>>,
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
    for row in 0..VIEW_ROWS {
        for column in 0..columns {
            let pos = Vec2::new(VIEW_X + column as f32 * TILE, VIEW_Y + row as f32 * TILE);
            commands.spawn((
                Tile(column, row),
                Sprite::default(),
                Anchor::TOP_LEFT,
                place(pos, 0.5),
                DespawnOnExit(GameScreen::PlanetMain),
            ));
        }
    }
    let (width, height) = (map.0.width as i32, map.0.height as i32);
    let start = surface.scroll.unwrap_or(IVec2::new(
        width / 2 - VIEW_COLUMNS as i32 / 2,
        height / 2 - 4,
    ));
    surface.scroll = Some(clamp_scroll(start, columns, width, height));
    surface.columns = columns;
}

fn clamp_scroll(scroll: IVec2, columns: usize, width: i32, height: i32) -> IVec2 {
    let max = IVec2::new(width - columns as i32, height - VIEW_ROWS as i32).max(IVec2::ZERO);
    scroll.clamp(IVec2::ZERO, max)
}

fn update_tiles(
    surface: Res<Surface>,
    maps: Res<Assets<PlanetMap>>,
    mut tiles: Query<(&Tile, &mut Sprite)>,
    added: Query<(), Added<Tile>>,
    mut shown: Local<Option<(IVec2, u16)>>,
) {
    let (Some(scroll), Some(map)) = (surface.scroll, maps.get(&surface.map)) else {
        return;
    };
    let key = (scroll, surface.frame);
    if *shown == Some(key) && added.is_empty() {
        return;
    }
    *shown = Some(key);
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
            (
                fanim.clone(),
                cell - surface.static_tiles,
                surface.frame * surface.fanim_frame_rows,
            )
        } else {
            (surface.felsz.clone(), 0, 0)
        };
        let per_row = TILES_PER_ROW as u16;
        let min = Vec2::new(
            ((index % per_row) * TILE_SIZE as u16) as f32,
            (top + (index / per_row) * TILE_SIZE as u16) as f32,
        );
        sprite.image = image;
        sprite.rect = Some(Rect::from_corners(min, min + TILE));
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
    let (Some(scroll), Some(map)) = (surface.scroll, maps.get(&surface.map)) else {
        return;
    };
    // Input y is up, map rows go down.
    let v = input.value;
    let step = if v.x.abs() > v.y.abs() {
        IVec2::new(v.x.signum() as i32, 0)
    } else {
        IVec2::new(0, -v.y.signum() as i32)
    };
    let columns = surface.columns;
    surface.scroll = Some(clamp_scroll(
        scroll + step,
        columns,
        map.0.width as i32,
        map.0.height as i32,
    ));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scroll_stays_inside_the_map() {
        assert_eq!(
            clamp_scroll(IVec2::new(-3, 50), 14, 48, 48),
            IVec2::new(0, 39)
        );
        assert_eq!(
            clamp_scroll(IVec2::new(40, 2), 14, 48, 48),
            IVec2::new(34, 2)
        );
    }
}
