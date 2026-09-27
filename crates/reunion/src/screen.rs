//! The original 320x200 screen, mapped into Bevy's world space.
//!
//! Game code uses the original coordinates (origin top-left, y down) so values
//! from the decompiled game can be used as-is. The picture is stretched 1.2x
//! vertically like a 4:3 CRT did, and a wider window shows extra space at the
//! sides instead of distorting it.

use bevy::camera::ScalingMode;
use bevy::prelude::*;
use bevy::sprite::Anchor;

pub const GAME_WIDTH: f32 = 320.0;
pub const GAME_HEIGHT: f32 = 200.0;
/// VGA mode 13h pixels were displayed 1.2x taller than wide on 4:3 monitors.
pub const PIXEL_ASPECT: f32 = 1.2;

#[derive(States, Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum GameScreen {
    #[default]
    MainMenu,
    ChooseHero,
}

pub struct ScreenPlugin;

impl Plugin for ScreenPlugin {
    fn build(&self, app: &mut App) {
        app.init_state::<GameScreen>()
            .add_systems(Startup, spawn_camera);
    }
}

fn spawn_camera(mut commands: Commands) {
    commands.spawn((
        Camera2d,
        Projection::Orthographic(OrthographicProjection {
            scaling_mode: ScalingMode::AutoMin {
                min_width: GAME_WIDTH,
                min_height: GAME_HEIGHT * PIXEL_ASPECT,
            },
            ..OrthographicProjection::default_2d()
        }),
    ));
}

pub fn game_to_world(pos: Vec2) -> Vec2 {
    Vec2::new(
        pos.x - GAME_WIDTH / 2.0,
        (GAME_HEIGHT / 2.0 - pos.y) * PIXEL_ASPECT,
    )
}

pub fn world_to_game(pos: Vec2) -> Vec2 {
    Vec2::new(
        pos.x + GAME_WIDTH / 2.0,
        GAME_HEIGHT / 2.0 - pos.y / PIXEL_ASPECT,
    )
}

/// Transform that places a top-left anchored sprite at `pos` in game coordinates.
pub fn place(pos: Vec2, z: f32) -> Transform {
    Transform::from_translation(game_to_world(pos).extend(z))
        .with_scale(Vec3::new(1.0, PIXEL_ASPECT, 1.0))
}

/// An original picture drawn at `pos` in game coordinates.
pub fn picture(image: Handle<Image>, pos: Vec2) -> impl Bundle {
    (Sprite::from_image(image), Anchor::TOP_LEFT, place(pos, 0.0))
}
