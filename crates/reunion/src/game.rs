//! The game being played: created when the first in-game screen appears
//! after choosing a hero, dropped when a new game is started.

use bevy::prelude::*;
use reunion_formats::state::GameState;

use crate::game_data::{GameData, GameDataHandle};
use crate::hero::Hero;
use crate::screen::{GameScreen, in_game};

pub struct GamePlugin;

impl Plugin for GamePlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(OnEnter(GameScreen::ChooseHero), |mut commands: Commands| {
            commands.remove_resource::<Game>();
        })
        .add_systems(
            Update,
            start_game.run_if(in_game.and_then(not(resource_exists::<Game>))),
        );
    }
}

#[derive(Resource)]
pub struct Game(pub GameState);

/// DS addresses of the selection the planet screens work on.
pub const CURRENT_SYSTEM: u16 = 0x7ae4;
/// Words per star system: selected planet (0x7ae6 + 2 * system) and moon (0x7af6 + 2 * system).
pub const CURRENT_PLANET: u16 = 0x7ae4;
pub const CURRENT_MOON: u16 = 0x7af4;

/// A new game starts from the executable's initial state plus the chosen hero.
fn start_game(
    mut commands: Commands,
    hero: Option<Res<Hero>>,
    handle: Res<GameDataHandle>,
    data: Res<Assets<GameData>>,
) {
    let Some(data) = data.get(&handle.0) else {
        return;
    };
    let mut state = data.new_game.clone();
    // FUN_321d_000d stores the hero at 0x9276 and 0x45 or 1 at 0x9278.
    let hero = hero.map_or(1, |h| h.0);
    state.set_word(0x9276, hero as u16);
    state.set_word(0x9278, if hero == 1 { 0x45 } else { 1 });
    commands.insert_resource(Game(state));
}

impl Game {
    /// Star system, planet and moon (0 = the planet itself) being looked at.
    pub fn selection(&self) -> (u16, u16, u16) {
        let state = &self.0;
        let system = state.word(CURRENT_SYSTEM).unwrap_or(1);
        let planet = state.word(CURRENT_PLANET + system * 2).unwrap_or(1);
        let moon = state.word(CURRENT_MOON + system * 2).unwrap_or(0);
        (system, planet, moon)
    }

    pub fn select(&mut self, system: u16, planet: u16, moon: u16) {
        let state = &mut self.0;
        state.set_word(CURRENT_SYSTEM, system);
        state.set_word(CURRENT_PLANET + system * 2, planet);
        state.set_word(CURRENT_MOON + system * 2, moon);
    }
}
