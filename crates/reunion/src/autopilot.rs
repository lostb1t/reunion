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
//! - `click <x> <y>` - activate the hotspot at game coordinates, like a
//!   mouse click
//! - `alt <x> <y>` - the other use of the hotspot there (see `Alternate`)
//! - `type <text>` - finish the text being typed with `text`
//! - `byte <address> <value>` / `word <address> <value>` - set game state
//!   at a data segment address (hex, like `a2d4`), e.g. to unlock things
//! - `message <text>` / `alert <text>` - add a normal / highlighted message
//!   to the log, like the game does
//! - `hours <n>` - let n game hours pass at once (message boxes queue up)
//! - `troops <system> <planet> <moon> <troopers> <tanks>` - a new army group
//!   on the ground there
//! - `ground <system> <planet> <moon> <race>` - a ground war there, against
//!   that race
//! - `army <system> <planet> <moon> <n>` - a new army group in orbit there
//!   with n hunters and lasers
//! - `battle <system> <planet> <moon> <race>` - a space battle there,
//!   against that race
//! - `talk <n>` / `scene <n>` - start a conversation with aliens / show a
//!   story picture, as the story does
//! - `cutscene <credits|intro|victory|death>` - play one of the original's
//!   cutscene programs
//! - `upscale <n>` - switch to upscaling mode n (see `UpscaleMode::ALL`)
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

use crate::focus::{Activated, AlternateUse, Focus, Hotspot};
use crate::game::Game;
use crate::hero::Hero;
use crate::input::{Back, Confirm, Navigate, Scroll};
use crate::screen::GameScreen;
use crate::text::TextEditing;
use crate::transition::GoTo;
use crate::upscale::UpscaleMode;

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
    camera: Single<Entity, With<crate::upscale::DisplayCamera>>,
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
        "load" => GameScreen::LoadGame,
        "research" => GameScreen::Research,
        "commanders" => GameScreen::Commanders,
        "computer" => GameScreen::MainComputer,
        "map" => GameScreen::GalacticMap,
        "credits" => GameScreen::Credits,
        "ships" => GameScreen::ShipInfo,
        "create" => GameScreen::CreateUnit,
        "group" => GameScreen::Group,
        "info" => GameScreen::InfoBuy,
        "mine" => GameScreen::ResourceMine,
        "pub" => GameScreen::SpaceLocal,
        "cockpit" => GameScreen::ControlPanel,
        "transfer" => GameScreen::Transfer,
        "settings" => GameScreen::Settings,
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
    hotspots: Query<(Entity, &Hotspot)>,
    mut focus: ResMut<Focus>,
    editing: Option<ResMut<TextEditing>>,
    handle: Res<crate::game_data::GameDataHandle>,
    data: Res<Assets<crate::game_data::GameData>>,
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
        "byte" | "word" => {
            let mut parts = arg.split_whitespace();
            let address = parts.next().and_then(|a| u16::from_str_radix(a, 16).ok());
            let value = parts.next().and_then(|v| v.parse::<u16>().ok());
            match (address, value, game.as_mut()) {
                (Some(address), Some(value), Some(game)) if command == "word" => {
                    game.0.set_word(address, value);
                }
                (Some(address), Some(value), Some(game)) => {
                    game.0.set_byte(address, value as u8);
                }
                _ => error!("autopilot: {command} needs an address, a value and a game"),
            }
        }
        "army" => {
            let v: Vec<u8> = arg.split_whitespace().filter_map(|n| n.parse().ok()).collect();
            let (Some(game), [s, p, m, hunters, ..]) = (game.as_mut(), v.as_slice()) else {
                error!("autopilot: army needs a game and system planet moon hunters");
                return;
            };
            let Some(n) = game.0.add_group() else { return };
            if let Some(g) = game.0.unit_mut(reunion_formats::state::UnitList::Groups, n) {
                (g[0x13], g[0x14], g[0x15], g[0x16]) = (*s, *p, *m, 2);
                g[0x1d..0x1f].copy_from_slice(&u16::from(*hunters).to_le_bytes());
                g[0x1f..0x21].copy_from_slice(&u16::from(*hunters).to_le_bytes());
            }
        }
        "talk" | "scene" => {
            let n = arg.parse().unwrap_or(2);
            let event = if command == "talk" {
                reunion_formats::story::Event::Talk(n)
            } else {
                reunion_formats::story::Event::Scene(n)
            };
            commands.trigger(crate::story::Tell(event));
        }
        "troops" => {
            let v: Vec<u8> = arg.split_whitespace().filter_map(|n| n.parse().ok()).collect();
            let (Some(game), [s, p, m, troopers, tanks, ..]) = (game.as_mut(), v.as_slice()) else {
                error!("autopilot: troops needs system planet moon troopers tanks");
                return;
            };
            let Some(n) = game.0.add_group() else { return };
            if let Some(g) = game.0.unit_mut(reunion_formats::state::UnitList::Groups, n) {
                (g[0x13], g[0x14], g[0x15], g[0x16]) = (*s, *p, *m, 1);
                for (k, count) in [(0, *troopers), (1, *tanks)] {
                    let at = 0x45 + 10 * k;
                    g[at..at + 2].copy_from_slice(&u16::from(count).to_le_bytes());
                    g[at + 2..at + 4].copy_from_slice(&u16::from(count).to_le_bytes());
                }
            }
        }
        "battle" => {
            let v: Vec<u8> = arg.split_whitespace().filter_map(|n| n.parse().ok()).collect();
            if let [s, p, m, race, ..] = v.as_slice() {
                if let Some(game) = game.as_mut() {
                    game.0.set_standing(*race, reunion_formats::aliens::AT_WAR);
                }
                commands.insert_resource(crate::space_battle::BattleStart { place: (*s, *p, *m), you_attack: true, ground: false });
                commands.trigger(GoTo(GameScreen::SpaceBattle));
            }
        }
        "ground" => {
            let v: Vec<u8> = arg.split_whitespace().filter_map(|n| n.parse().ok()).collect();
            if let [s, p, m, race, ..] = v.as_slice() {
                if let Some(game) = game.as_mut() {
                    game.0.set_standing(*race, reunion_formats::aliens::AT_WAR);
                }
                commands.insert_resource(crate::space_battle::BattleStart { place: (*s, *p, *m), you_attack: true, ground: true });
                commands.trigger(GoTo(GameScreen::GroundSetup));
            }
        }
        "hours" => {
            let (Some(game), Some(data)) = (game.as_mut(), data.get(&handle.0)) else {
                error!("autopilot: hours needs a game");
                return;
            };
            for _ in 0..arg.parse::<u32>().unwrap_or(1) {
                crate::clock::pass_hour(game, data, &mut commands);
            }
        }
        "type" => match editing {
            Some(mut editing) => {
                editing.text = arg.to_string();
                editing.done = true;
            }
            None => error!("autopilot: nothing is being typed"),
        },
        "click" | "alt" => {
            let mut numbers = arg.split_whitespace().filter_map(|n| n.parse::<f32>().ok());
            let (Some(x), Some(y)) = (numbers.next(), numbers.next()) else {
                error!("autopilot: click needs x and y");
                return;
            };
            match hotspots
                .iter()
                .filter(|(_, h)| h.rect.contains(Vec2::new(x, y)))
                .min_by(|(_, a), (_, b)| {
                    let area = |r: Rect| r.width() * r.height();
                    area(a.rect).total_cmp(&area(b.rect))
                })
            {
                Some((entity, _)) => {
                    focus.0 = Some(entity);
                    if command == "alt" {
                        commands.trigger(AlternateUse(entity));
                    } else {
                        commands.trigger(Activated(entity));
                    }
                }
                None => error!("autopilot: no hotspot at {x}, {y}"),
            }
        }
        "message" | "alert" => {
            if let Some(game) = game.as_mut() {
                game.0
                    .add_message(if command == "alert" { 1 } else { 0 }, arg);
            }
        }
        "cutscene" => {
            let script = match arg {
                "credits" => Some(crate::cutscene::company_credits()),
                "intro" => Some(crate::cutscene::intro()),
                "victory" => Some(crate::cutscene::victory()),
                "death" => Some(crate::cutscene::death(true, 2)),
                _ => None,
            };
            match script {
                Some(script) => {
                    commands.insert_resource(crate::cutscene::Cutscenes { scripts: vec![script], then: GameScreen::MainMenu });
                    commands.trigger(GoTo(GameScreen::Cutscene));
                }
                None => error!("autopilot: unknown cutscene {arg}"),
            }
        }
        "upscale" => {
            match arg.parse::<usize>().ok().and_then(|n| UpscaleMode::ALL.get(n)) {
                Some(&mode) => commands.insert_resource(mode),
                None => error!("autopilot: upscale needs a mode 0-{}", UpscaleMode::ALL.len() - 1),
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
