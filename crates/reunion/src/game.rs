//! The game being played: created when the first in-game screen appears
//! after choosing a hero, dropped when a new game is started.

use bevy::prelude::*;
use reunion_formats::exe::SystemLayout;
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

/// The game's Random(n): 0 to n - 1 (0 for 0).
pub fn random(n: u16) -> u16 {
    use std::hash::BuildHasher;
    (std::collections::hash_map::RandomState::new().hash_one(n) % u64::from(n.max(1))) as u16
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

    /// Number of the selected planet or moon in its system (planets first,
    /// then moons), for [`GameState::body`].
    /// Number of the planet or moon at `(system, planet, moon)` in its system.
    pub fn body_number(&self, layouts: &[SystemLayout], (system, planet, moon): (u8, u8, u8)) -> Option<usize> {
        if moon == 0 {
            return Some(planet.into());
        }
        let moons = layouts.get(usize::from(system).checked_sub(1)?)?.moons.get(usize::from(planet).checked_sub(1)?)?;
        moons.get(usize::from(moon) - 1).map(|&b| b as usize)
    }

    pub fn selected_body(&self, layouts: &[SystemLayout]) -> Option<usize> {
        let (system, planet, moon) = self.selection();
        if moon == 0 {
            return Some(planet as usize);
        }
        let moons = layouts
            .get((system as usize).checked_sub(1)?)?
            .moons
            .get((planet as usize).checked_sub(1)?)?;
        moons.get(moon as usize - 1).map(|&body| body as usize)
    }

    /// Name of a planet (moon 0) or moon, like FUN_357b_332c.
    pub fn body_name(&self, layouts: &[SystemLayout], system: u16, planet: u16, moon: u16) -> String {
        let body = if moon == 0 {
            Some(planet as usize)
        } else {
            layouts
                .get((system as usize).wrapping_sub(1))
                .and_then(|l| l.moons.get((planet as usize).wrapping_sub(1)))
                .and_then(|m| m.get(moon as usize - 1))
                .map(|&b| b as usize)
        };
        body.and_then(|b| {
            self.0
                .star_systems()
                .into_iter()
                .nth((system as usize).wrapping_sub(1))?
                .bodies
                .into_iter()
                .nth(b.wrapping_sub(1))
        })
        .map(|b| b.name.trim_end().to_string())
        .unwrap_or_default()
    }

    pub fn select(&mut self, system: u16, planet: u16, moon: u16) {
        let state = &mut self.0;
        state.set_word(CURRENT_SYSTEM, system);
        state.set_word(CURRENT_PLANET + system * 2, planet);
        state.set_word(CURRENT_MOON + system * 2, moon);
    }
}
