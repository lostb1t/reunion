//! The main screen (screen 1): the control room, MAIN.PIC at row 49, with
//! the fixed hotspots from FUN_3abd_08e0. The icon bar and text strip come
//! from the HUD.
//!
//! The people: you (GRAFIKA/HEROES, 67x59 at (131, 99), FUN_3abd_163f) and
//! the commanders you hired (GRAFIKA/MAINFACE, FUN_3abd_16bc: per category
//! a place in the room, DS:0x54cc x, 0x54d4 y, 0x550c width, 0x5514 height,
//! and per candidate a frame, DS:0x54d6 / 0x54ee + 6 * category + 2 *
//! candidate). Commanders away at university aren't there. Clicking one
//! talks to them (screen 24).
//!
//! Widescreen: extension art from `art://widescreen/` on both sides.

use bevy::prelude::*;
use bevy::sprite::Anchor;

use crate::anim_player::{AnimPlayer, Delay, Segment, anim_player};
use crate::audio::ClickSound;
use crate::focus::{Activated, Hover, hotspot};
use crate::game::Game;
use crate::game_data::{GameData, GameDataHandle};
use crate::pic::{MASKED, PALETTE};
use crate::staff_talk::TalkTo;
use crate::transition::GoTo;
use crate::hud::{ActionByLabel, CONTENT_Y, HoverLabel};
use crate::screen::{GAME_WIDTH, GameScreen, picture, place};

pub struct MainScreenPlugin;

impl Plugin for MainScreenPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(OnEnter(GameScreen::MainScreen), spawn_room)
            .add_systems(
                Update,
                (spawn_room_hotspots, window_view).run_if(in_state(GameScreen::MainScreen)),
            )
            .add_observer(talk);
    }
}

/// Room spotlights dim everything below the text strip, extensions included.
const ROOM_AREA: Rect = Rect {
    min: Vec2::new(-1000.0, CONTENT_Y),
    max: Vec2::new(GAME_WIDTH + 1000.0, 200.0),
};

#[derive(Component)]
struct RoomHotspotsSpawned;

/// A commander in the room (category 1-4).
#[derive(Component)]
struct Person(u16);

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
        scoped.clone(),
    ));
    // The round window: ships flying past outside (ANIM/MAIN1-6 at (83, 49)).
    commands.spawn((
        WindowView { wait: 0.0 },
        anim_player(
            AnimPlayer::new(Vec::new(), asset_server.load(format!("GRAFIKA/MAIN.PIC#{PALETTE}"))).with_crop(Rect::new(1.0, 1.0, 43.0, 65.0)),
            Vec2::new(84.0, CONTENT_Y + 1.0),
            0.2,
        ),
        scoped,
    ));
}

/// Seconds until the window's next animation.
#[derive(Component)]
struct WindowView {
    wait: f32,
}

/// The original's main loop steps (FUN_3abd_0c70 runs once a step).
const ROOM_STEP: f32 = 1.0 / 18.2;

/// FUN_3abd_0c70: one of the six animations a frame a step, then a pause of
/// 100-299 steps before the next.
fn window_view(
    time: Res<Time>,
    asset_server: Res<AssetServer>,
    mut views: Query<(&mut WindowView, &mut AnimPlayer)>,
) {
    for (mut view, mut player) in &mut views {
        if !player.segments.is_empty() {
            continue;
        }
        view.wait -= time.delta_secs();
        if view.wait > 0.0 {
            continue;
        }
        let n = crate::game::random(6) + 1;
        player.segments.push_back(Segment {
            animation: asset_server.load(format!("ANIM/MAIN{n}.ANI")),
            from: 2,
            to: usize::MAX,
            delay: Delay::Fixed(4),
            sounds: Vec::new(),
            end_sound: None,
            looping: false,
        });
        view.wait = f32::from(crate::game::random(200) + 100) * ROOM_STEP;
    }
}

/// Room hotspots need the tables from the game data, which may still be loading.
fn spawn_room_hotspots(
    mut commands: Commands,
    handle: Res<GameDataHandle>,
    data: Res<Assets<GameData>>,
    spawned: Query<(), With<RoomHotspotsSpawned>>,
    game: Option<Res<Game>>,
    asset_server: Res<AssetServer>,
) {
    if !spawned.is_empty() {
        return;
    }
    let (Some(data), Some(game)) = (data.get(&handle.0), game) else {
        return;
    };
    let scoped = DespawnOnExit(GameScreen::MainScreen);
    commands.spawn((RoomHotspotsSpawned, scoped.clone()));
    // FUN_3abd_005c: each part of the room has its sound (a door, a console...).
    const ROOM_SOUNDS: [&str; 8] = [
        "research", "messages", "door1", "door2", "starmap", "local", "door3", "surface",
    ];
    for (room, sound) in data.main_room.iter().zip(ROOM_SOUNDS) {
        let rect = Rect::new(
            room.x as f32,
            room.y as f32,
            (room.x + room.width) as f32,
            (room.y + room.height) as f32,
        );
        commands.spawn((
            ActionByLabel,
            ClickSound(sound),
            HoverLabel(room.label.clone()),
            hotspot(rect, Hover::Spotlight { within: ROOM_AREA }),
            scoped.clone(),
        ));
    }
    let word = |at: u16| {
        data.exe
            .ds_bytes(at, 2)
            .map_or(0.0, |b| f32::from(i16::from_le_bytes([b[0], b[1]])))
    };
    let sprite = |image: &Handle<Image>, rect: Rect, at: Vec2| {
        (
            Sprite {
                image: image.clone(),
                rect: Some(rect),
                ..default()
            },
            Anchor::TOP_LEFT,
            place(at, 0.3),
        )
    };
    // You.
    let heroes = asset_server.load::<Image>(format!("GRAFIKA/HEROES.PIC#{MASKED}"));
    let x = f32::from(game.0.word(0x9278).unwrap_or(1));
    commands.spawn((
        sprite(&heroes, Rect::new(x, 1.0, x + 67.0, 60.0), Vec2::new(131.0, 99.0)),
        scoped.clone(),
    ));
    // The commanders you hired, in the drawing order of DS:0x551c.
    let faces = asset_server.load::<Image>(format!("GRAFIKA/MAINFACE.PIC#{MASKED}"));
    for slot in 1..=4u16 {
        let p = word(0x551c + 2 * slot) as u16;
        let hired = game.0.word(0x95b4 + 2 * p).unwrap_or(0);
        let away = game.0.word(0x5d9e) == Some(p) || (p == 4 && game.0.word(0x5d52) != Some(0));
        if hired == 0 || hired > 3 || away {
            continue;
        }
        let at = Vec2::new(word(0x54cc + 2 * p) + 1.0, word(0x54d4 + 2 * p) + CONTENT_Y + 1.0);
        let size = Vec2::new(word(0x550c + 2 * p) - 2.0, word(0x5514 + 2 * p) - 2.0);
        let from = Vec2::new(
            word(0x54d6 + 6 * p + 2 * hired) + 1.0,
            word(0x54ee + 6 * p + 2 * hired) + 1.0,
        );
        commands.spawn((
            sprite(&faces, Rect::from_corners(from, from + size), at),
            scoped.clone(),
        ));
        let name = data
            .exe
            .ds_string(0x57f8 + 0x39 * p + 0x13 * hired)
            .unwrap_or_default();
        commands.spawn((
            Person(p),
            ClickSound(["pilots", "builders", "fighters", "develope"][usize::from(p.clamp(1, 4)) - 1]),
            HoverLabel(name),
            hotspot(
                Rect::from_corners(at - 1.0, at + size + 1.0),
                Hover::Spotlight { within: ROOM_AREA },
            ),
            scoped.clone(),
        ));
    }
}

/// FUN_3abd_005c: a commander was clicked; talk to them.
fn talk(activated: On<Activated>, people: Query<&Person>, mut commands: Commands) {
    if let Ok(&Person(p)) = people.get(activated.0) {
        commands.insert_resource(TalkTo(p));
        commands.trigger(GoTo(GameScreen::StaffTalk));
    }
}
