//! Debug autopilot: plays a script and saves screenshots, for checking what
//! actually renders and how screens respond to input, without playing by hand.
//!
//! `REUNION_AUTOPILOT=<dir>` turns it on; screenshots go to `<dir>`.
//! `REUNION_SCRIPT` holds `;`-separated commands (default: a main screen and
//! planet screen tour):
//!
//! - `new` - start a new game on the main screen (skips the menus)
//! - `goto <screen>` - switch screens like navigating there
//! - `press <button>` - `up`, `down`, `left`, `right`, `confirm`, `back`,
//!   `scroll-up`, `scroll-down`, `scroll-left`, `scroll-right`; goes through
//!   the input actions like a real button press
//! - `message <text>` / `alert <text>` - add a normal / highlighted message
//!   to the log, like the game does
//! - `wait <seconds>`
//! - `shot <name>` - saves `<dir>/<name>.png`
//!
//! The game exits at the end. The camera renders into an image instead of the
//! window, so capturing works even when the window isn't visible (macOS
//! doesn't draw hidden windows).

use std::collections::VecDeque;
use std::path::PathBuf;

use bevy::camera::RenderTarget;
use bevy::prelude::*;
use bevy::render::render_resource::TextureFormat;
use bevy::render::view::screenshot::{Screenshot, save_to_disk};
use bevy_enhanced_input::prelude::*;

use crate::game::Game;
use crate::hero::Hero;
use crate::input::{Back, Confirm, Navigate, Scroll};
use crate::screen::GameScreen;
use crate::transition::GoTo;

pub struct AutopilotPlugin;

const DEFAULT_SCRIPT: &str = "wait 1.5; new; wait 3; shot main; goto planet; wait 3; shot planet";
/// Pause after every command so its effect (and any fade) settles.
const STEP_PAUSE: f32 = 0.4;

impl Plugin for AutopilotPlugin {
    fn build(&self, app: &mut App) {
        let Ok(dir) = std::env::var("REUNION_AUTOPILOT") else {
            return;
        };
        let script = std::env::var("REUNION_SCRIPT").unwrap_or_else(|_| DEFAULT_SCRIPT.to_string());
        app.insert_resource(Autopilot {
            dir: dir.into(),
            commands: script
                .split(';')
                .map(|c| c.trim().to_string())
                .filter(|c| !c.is_empty())
                .collect(),
            wait: 0.0,
            target: Handle::default(),
        })
        .add_systems(PostStartup, render_to_image)
        .add_systems(Update, run);
    }
}

#[derive(Resource)]
struct Autopilot {
    dir: PathBuf,
    commands: VecDeque<String>,
    /// Seconds until the next command.
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

fn screen_named(name: &str) -> Option<GameScreen> {
    Some(match name {
        "menu" => GameScreen::MainMenu,
        "main" => GameScreen::MainScreen,
        "planet" => GameScreen::PlanetMain,
        "planet-info" => GameScreen::PlanetInfo,
        "messages" => GameScreen::Messages,
        "disk" => GameScreen::DiskOperations,
        "computer" => GameScreen::MainComputer,
        _ => return None,
    })
}

fn run(
    time: Res<Time>,
    mut pilot: ResMut<Autopilot>,
    mut game: Option<ResMut<Game>>,
    navigate: Single<Entity, With<Action<Navigate>>>,
    scroll: Single<Entity, With<Action<Scroll>>>,
    confirm: Single<Entity, With<Action<Confirm>>>,
    back: Single<Entity, With<Action<Back>>>,
    mut exit: MessageWriter<AppExit>,
    mut commands: Commands,
) {
    pilot.wait -= time.delta_secs();
    if pilot.wait > 0.0 {
        return;
    }
    let Some(line) = pilot.commands.pop_front() else {
        exit.write(AppExit::Success);
        pilot.wait = f32::MAX;
        return;
    };
    info!("autopilot: {line}");
    pilot.wait = STEP_PAUSE;
    let (command, arg) = line.split_once(' ').unwrap_or((&line, ""));
    let arg = arg.trim();
    match command {
        "new" => {
            commands.insert_resource(Hero(1));
            commands.trigger(GoTo(GameScreen::MainScreen));
        }
        "goto" => match screen_named(arg) {
            Some(screen) => {
                if screen == GameScreen::PlanetMain
                    && let Some(game) = game.as_mut()
                {
                    game.select(1, 5, 0);
                }
                commands.trigger(GoTo(screen));
            }
            None => error!("autopilot: unknown screen {arg}"),
        },
        "press" => {
            let (entity, value) = match arg {
                "confirm" => (*confirm, ActionValue::Bool(true)),
                "back" => (*back, ActionValue::Bool(true)),
                "up" => (*navigate, ActionValue::Axis2D(Vec2::Y)),
                "down" => (*navigate, ActionValue::Axis2D(Vec2::NEG_Y)),
                "left" => (*navigate, ActionValue::Axis2D(Vec2::NEG_X)),
                "right" => (*navigate, ActionValue::Axis2D(Vec2::X)),
                "scroll-up" => (*scroll, ActionValue::Axis2D(Vec2::Y)),
                "scroll-down" => (*scroll, ActionValue::Axis2D(Vec2::NEG_Y)),
                "scroll-left" => (*scroll, ActionValue::Axis2D(Vec2::NEG_X)),
                "scroll-right" => (*scroll, ActionValue::Axis2D(Vec2::X)),
                _ => {
                    error!("autopilot: unknown button {arg}");
                    return;
                }
            };
            commands
                .entity(entity)
                .insert(ActionMock::once(TriggerState::Fired, value));
        }
        "message" | "alert" => {
            if let Some(game) = game.as_mut() {
                game.0
                    .add_message(if command == "alert" { 1 } else { 0 }, arg);
            }
        }
        "wait" => pilot.wait = arg.parse().unwrap_or(1.0),
        "shot" => {
            let path = pilot.dir.join(format!("{arg}.png"));
            commands
                .spawn(Screenshot::image(pilot.target.clone()))
                .observe(save_to_disk(path));
        }
        _ => error!("autopilot: unknown command {command}"),
    }
}
