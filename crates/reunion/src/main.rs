mod focus;
mod input;
mod menu;
mod pic;
mod screen;

use bevy::asset::AssetMetaCheck;
use bevy::prelude::*;
use bevy::window::WindowResolution;
use bevy_enhanced_input::prelude::*;

fn main() {
    App::new()
        .insert_resource(ClearColor(Color::BLACK))
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
        ))
        .run();
}
