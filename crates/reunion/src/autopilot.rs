//! Debug autopilot: with `REUNION_AUTOPILOT=<dir>` the game skips the menus,
//! screenshots the main screen and PLANET MAIN into `<dir>`, and exits.
//! For checking what actually renders without playing through by hand.
//! The camera renders into an image instead of the window, so capturing works
//! even when the window isn't visible (macOS doesn't draw hidden windows).

use std::path::PathBuf;

use bevy::camera::RenderTarget;
use bevy::prelude::*;
use bevy::render::render_resource::TextureFormat;
use bevy::render::view::screenshot::{Screenshot, save_to_disk};

use crate::game::Game;
use crate::hero::Hero;
use crate::screen::GameScreen;
use crate::transition::GoTo;

pub struct AutopilotPlugin;

impl Plugin for AutopilotPlugin {
    fn build(&self, app: &mut App) {
        if let Ok(dir) = std::env::var("REUNION_AUTOPILOT") {
            app.insert_resource(Autopilot {
                dir: dir.into(),
                step: 0,
                wait: 1.5,
                target: Handle::default(),
            })
            .add_systems(PostStartup, render_to_image)
            .add_systems(Update, run);
        }
    }
}

#[derive(Resource)]
struct Autopilot {
    dir: PathBuf,
    step: u8,
    /// Seconds until the next step.
    wait: f32,
    /// What the camera renders into.
    target: Handle<Image>,
}

/// The Steam Deck's resolution.
const SIZE: UVec2 = UVec2::new(1280, 800);

fn render_to_image(
    mut commands: Commands,
    camera: Single<Entity, With<Camera2d>>,
    mut images: ResMut<Assets<Image>>,
    mut pilot: ResMut<Autopilot>,
) {
    let target = images.add(Image::new_target_texture(
        SIZE.x,
        SIZE.y,
        TextureFormat::Rgba8UnormSrgb,
        None,
    ));
    commands
        .entity(*camera)
        .insert(RenderTarget::Image(target.clone().into()));
    pilot.target = target;
}

fn run(
    time: Res<Time>,
    mut pilot: ResMut<Autopilot>,
    mut game: Option<ResMut<Game>>,
    mut exit: MessageWriter<AppExit>,
    mut commands: Commands,
) {
    pilot.wait -= time.delta_secs();
    if pilot.wait > 0.0 {
        return;
    }
    let target = pilot.target.clone();
    let screenshot = |commands: &mut Commands, name: &str, dir: &PathBuf| {
        let path = dir.join(name);
        info!("autopilot: screenshot {}", path.display());
        commands
            .spawn(Screenshot::image(target.clone()))
            .observe(save_to_disk(path));
    };
    let (next_wait, done) = match pilot.step {
        0 => {
            screenshot(&mut commands, "menu.png", &pilot.dir);
            (0.5, false)
        }
        1 => {
            commands.insert_resource(Hero(1));
            commands.trigger(GoTo(GameScreen::MainScreen));
            (3.0, false)
        }
        2 => {
            screenshot(&mut commands, "main.png", &pilot.dir);
            (1.0, false)
        }
        3 => {
            if let Some(game) = game.as_mut() {
                game.select(1, 5, 0);
            }
            commands.trigger(GoTo(GameScreen::PlanetMain));
            (3.0, false)
        }
        4 => {
            screenshot(&mut commands, "planet.png", &pilot.dir);
            (1.0, false)
        }
        _ => {
            exit.write(AppExit::Success);
            (f32::MAX, true)
        }
    };
    pilot.wait = next_wait;
    if !done {
        pilot.step += 1;
    }
}
