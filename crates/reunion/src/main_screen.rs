//! The main screen (screen 1): the control room, MAIN.PIC at row 49, with
//! the fixed hotspots from FUN_3abd_08e0. The icon bar and text strip come
//! from the HUD.
//!
//! Widescreen: extension art from `art://widescreen/` on both sides.

use bevy::prelude::*;
use bevy::sprite::Anchor;

use crate::focus::{Hover, hotspot};
use crate::game_data::{GameData, GameDataHandle};
use crate::hud::{ActionByLabel, CONTENT_Y, HoverLabel};
use crate::screen::{GAME_WIDTH, GameScreen, picture, place};

pub struct MainScreenPlugin;

impl Plugin for MainScreenPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(OnEnter(GameScreen::MainScreen), spawn_room)
            .add_systems(
                Update,
                spawn_room_hotspots.run_if(in_state(GameScreen::MainScreen)),
            );
    }
}

/// Room spotlights dim everything below the text strip, extensions included.
const ROOM_AREA: Rect = Rect {
    min: Vec2::new(-1000.0, CONTENT_Y),
    max: Vec2::new(GAME_WIDTH + 1000.0, 200.0),
};

#[derive(Component)]
struct RoomHotspotsSpawned;

fn spawn_room(mut commands: Commands, asset_server: Res<AssetServer>) {
    let scoped = DespawnOnExit(GameScreen::MainScreen);
    commands.spawn((
        picture(
            asset_server.load("GRAFIKA/MAIN.PIC"),
            Vec2::new(0.0, CONTENT_Y),
        ),
        scoped.clone(),
    ));
    // Extension art beside the original: the left one ends at x 0, the right starts at 320.
    commands.spawn((
        Sprite::from_image(asset_server.load("art://widescreen/GRAFIKA/MAIN.left.png")),
        Anchor::TOP_RIGHT,
        place(Vec2::new(0.0, CONTENT_Y), 0.0),
        scoped.clone(),
    ));
    commands.spawn((
        Sprite::from_image(asset_server.load("art://widescreen/GRAFIKA/MAIN.right.png")),
        Anchor::TOP_LEFT,
        place(Vec2::new(GAME_WIDTH, CONTENT_Y), 0.0),
        scoped,
    ));
}

/// Room hotspots need the tables from the game data, which may still be loading.
fn spawn_room_hotspots(
    mut commands: Commands,
    handle: Res<GameDataHandle>,
    data: Res<Assets<GameData>>,
    spawned: Query<(), With<RoomHotspotsSpawned>>,
) {
    if !spawned.is_empty() {
        return;
    }
    let Some(data) = data.get(&handle.0) else {
        return;
    };
    let scoped = DespawnOnExit(GameScreen::MainScreen);
    commands.spawn((RoomHotspotsSpawned, scoped.clone()));
    for room in &data.main_room {
        let rect = Rect::new(
            room.x as f32,
            room.y as f32,
            (room.x + room.width) as f32,
            (room.y + room.height) as f32,
        );
        commands.spawn((
            ActionByLabel,
            HoverLabel(room.label.clone()),
            hotspot(rect, Hover::Spotlight { within: ROOM_AREA }),
            scoped.clone(),
        ));
    }
}
