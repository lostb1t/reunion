// Bevy systems routinely take many parameters and nested query types.
#![allow(clippy::too_many_arguments, clippy::type_complexity)]

mod alien_talk;
mod anim_player;
mod audio;
mod autopilot;
mod clock;
mod colonize;
mod commanders;
mod control_panel;
mod create_unit;
mod credits;
mod cutscene;
mod disk;
mod focus;
mod galactic_map;
mod game;
mod game_data;
mod group;
mod ground_war;
mod hero;
mod hud;
mod info_buy;
mod input;
mod main_computer;
mod main_screen;
mod menu;
mod messages;
mod model_view;
mod pic;
mod planet;
mod planet_info;
mod popup;
mod research;
mod resource_mine;
mod pub_local;
mod screen;
mod settings;
mod space_battle;
mod story;
mod ship_info;
mod staff_talk;
mod text;
mod transfer;
mod transition;
mod upscale;

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
        // START.EXE: the company credits and the intro before the game.
        .insert_resource(cutscene::Cutscenes {
            scripts: vec![cutscene::company_credits(), cutscene::intro()],
            then: screen::GameScreen::MainMenu,
        })
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
            upscale::UpscalePlugin,
            audio::SoundPlugin,
            cutscene::CutscenePlugin,
            anim_player::AnimPlayerPlugin,
            settings::SettingsPlugin,
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
            research::ResearchPlugin,
            commanders::CommandersPlugin,
            galactic_map::GalacticMapPlugin,
            credits::CreditsPlugin,
        ))
        .add_plugins((
            text::TextPlugin,
            ship_info::ShipInfoPlugin,
            create_unit::CreateUnitPlugin,
            group::GroupPlugin,
            model_view::ModelViewPlugin,
            info_buy::InfoBuyPlugin,
            popup::PopupPlugin,
            control_panel::ControlPanelPlugin,
            transfer::TransferPlugin,
            staff_talk::StaffTalkPlugin,
            resource_mine::ResourceMinePlugin,
            colonize::ColonizePlugin,
            space_battle::SpaceBattlePlugin,
        ))
        .add_plugins((
            story::StoryPlugin,
            alien_talk::AlienTalkPlugin,
            pub_local::PubPlugin,
            ground_war::GroundWarPlugin,
        ))
        .run();
}
