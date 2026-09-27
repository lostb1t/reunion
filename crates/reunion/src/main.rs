// Bevy systems routinely take many parameters and nested query types.
#![allow(clippy::too_many_arguments, clippy::type_complexity)]

mod autopilot;
mod clock;
mod disk;
mod focus;
mod game;
mod game_data;
mod hero;
mod hud;
mod input;
mod main_computer;
mod main_screen;
mod menu;
mod messages;
mod pic;
mod planet;
mod planet_info;
mod screen;
mod transition;

use bevy::asset::AssetMetaCheck;
use bevy::asset::io::AssetSourceBuilder;
use bevy::prelude::*;
use bevy::window::WindowResolution;
use bevy_enhanced_input::prelude::*;

fn main() {
    let mut app = App::new();
    // Release builds log system errors instead of crashing the game.
    if !cfg!(debug_assertions) {
        app.set_error_handler(bevy::ecs::error::warn);
    }
    app.insert_resource(ClearColor(Color::BLACK))
        // Our own art (widescreen extensions), next to the player's game files.
        .register_asset_source("art", AssetSourceBuilder::platform_default("art", None))
        .add_plugins(
            DefaultPlugins
                .set(WindowPlugin {
                    primary_window: Some(Window {
                        title: "Reunion".into(),
                        resolution: WindowResolution::new(1280, 800),
                        // Web: render into the page's canvas instead of appending one.
                        canvas: Some("#bevy".into()),
                        fit_canvas_to_parent: true,
                        ..default()
                    }),
                    ..default()
                })
                // The original game files have no .meta sidecars; don't request them over HTTP.
                .set(AssetPlugin {
                    meta_check: AssetMetaCheck::Never,
                    ..default()
                })
                .set(ImagePlugin::default_nearest()),
        )
        .add_plugins(EnhancedInputPlugin)
        .add_plugins((
            pic::PicPlugin,
            screen::ScreenPlugin,
            input::InputPlugin,
            focus::FocusPlugin,
            menu::MenuPlugin,
            hero::HeroPlugin,
            transition::TransitionPlugin,
            game_data::GameDataPlugin,
        ))
        // Game screens.
        .add_plugins((
            main_screen::MainScreenPlugin,
            game::GamePlugin,
            hud::HudPlugin,
            planet::PlanetPlugin,
            clock::ClockPlugin,
            autopilot::AutopilotPlugin,
            disk::DiskPlugin,
            planet_info::PlanetInfoPlugin,
            messages::MessagesPlugin,
            main_computer::MainComputerPlugin,
        ))
        .run();
}
