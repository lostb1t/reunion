//! Main menu (GRAFIKA/OPTIONS.PIC) and the hero selection placeholder.

use bevy::prelude::*;
use bevy_enhanced_input::prelude::*;

use crate::focus::{Activated, DefaultFocus, Hover, hotspot};
use crate::input::Back;
use crate::screen::{GameScreen, picture};

pub struct MenuPlugin;

impl Plugin for MenuPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(OnEnter(GameScreen::MainMenu), spawn_main_menu)
            .add_systems(OnEnter(GameScreen::ChooseHero), spawn_choose_hero)
            .add_observer(on_activated)
            .add_observer(on_back);
    }
}

#[derive(Component, Clone, Copy, PartialEq, Debug)]
enum MenuChoice {
    NewGame,
    LoadGame,
    ExitToDos,
}

// Click areas from the original menu loop (REUNION.PRG FUN_321d_0155):
// 0x4a < x < 0xfb, and y in (0x2f, 0x4c), (0x4b, 0x69), (0x68, 0x85), exclusive.
const MENU_LEFT: f32 = 75.0;
const MENU_RIGHT: f32 = 251.0;
const MENU_ITEMS: [(MenuChoice, f32, f32); 3] = [
    (MenuChoice::NewGame, 48.0, 76.0),
    (MenuChoice::LoadGame, 76.0, 105.0),
    (MenuChoice::ExitToDos, 105.0, 133.0),
];

fn spawn_main_menu(mut commands: Commands, asset_server: Res<AssetServer>) {
    let background: Handle<Image> = asset_server.load("GRAFIKA/OPTIONS.PIC");
    commands.spawn((picture(background.clone(), Vec2::ZERO), DespawnOnExit(GameScreen::MainMenu)));
    for (choice, top, bottom) in MENU_ITEMS {
        let rect = Rect::new(MENU_LEFT, top, MENU_RIGHT, bottom);
        let mut item = commands.spawn((
            hotspot(rect, Hover::Brighten(background.clone())),
            choice,
            DespawnOnExit(GameScreen::MainMenu),
        ));
        if choice == MenuChoice::NewGame {
            item.insert(DefaultFocus);
        }
    }
}

fn spawn_choose_hero(mut commands: Commands, asset_server: Res<AssetServer>) {
    commands.spawn((
        picture(asset_server.load("GRAFIKA/CHOISE.PIC"), Vec2::ZERO),
        DespawnOnExit(GameScreen::ChooseHero),
    ));
}

fn on_activated(
    activated: On<Activated>,
    choices: Query<&MenuChoice>,
    mut next: ResMut<NextState<GameScreen>>,
    mut exit: MessageWriter<AppExit>,
) {
    let Ok(choice) = choices.get(activated.0) else {
        return;
    };
    match choice {
        MenuChoice::NewGame => next.set(GameScreen::ChooseHero),
        MenuChoice::LoadGame => info!("Load Game is not implemented yet"),
        MenuChoice::ExitToDos if cfg!(target_arch = "wasm32") => info!("No DOS to exit to in the browser"),
        MenuChoice::ExitToDos => {
            exit.write(AppExit::Success);
        }
    }
}

fn on_back(_: On<Start<Back>>, screen: Res<State<GameScreen>>, mut next: ResMut<NextState<GameScreen>>) {
    if *screen.get() == GameScreen::ChooseHero {
        next.set(GameScreen::MainMenu);
    }
}
