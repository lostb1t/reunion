//! Game time. The original counts VGA frames in its main loop (DS:0x95f6)
//! and advances an hour once the count passes 100, so an hour is 101 frames
//! of the 70.086 Hz VGA refresh, about 1.44 seconds.

use bevy::prelude::*;

use reunion_formats::state::{TravelEvent, UnitList};

use crate::game::Game;
use crate::game_data::{GameData, GameDataHandle};
use crate::game::random;
use crate::popup::{PopupOpen, ShowMessage};
use crate::space_battle::BattleStart;
use crate::story::Tell;
use crate::transition::GoTo;
use crate::screen::{GameScreen, in_game};
use crate::ship_info::unit_name;

const VGA_REFRESH_HZ: f64 = 70.086;
const FRAMES_PER_HOUR: f64 = 101.0;

pub struct ClockPlugin;

impl Plugin for ClockPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Frames>().add_systems(
            Update,
            tick.run_if(in_game.and_then(resource_exists::<Game>)),
        );
    }
}

/// VGA frames counted towards the next hour.
#[derive(Resource, Default)]
struct Frames(f64);

fn tick(
    time: Res<Time>,
    screen: Res<State<GameScreen>>,
    mut frames: ResMut<Frames>,
    mut game: ResMut<Game>,
    handle: Res<GameDataHandle>,
    data: Res<Assets<GameData>>,
    popup: Option<Res<PopupOpen>>,
    mut commands: Commands,
) {
    // Message boxes stop the game until they're closed, like the original's.
    if screen.get().pauses_time() || popup.is_some() {
        return;
    }
    let Some(data) = data.get(&handle.0) else { return };
    frames.0 += time.delta_secs_f64() * VGA_REFRESH_HZ;
    while frames.0 >= FRAMES_PER_HOUR {
        frames.0 -= FRAMES_PER_HOUR;
        // One hour at a time while boxes are waiting.
        if pass_hour(&mut game, data, &mut commands) {
            frames.0 = 0.0;
            break;
        }
    }
}

/// An hour of the game: the clock, the simulation and travel. Returns
/// whether a message box came up.
pub fn pass_hour(game: &mut Game, data: &GameData, commands: &mut Commands) -> bool {
    let mut shown = false;
    game.0.advance_hour();
    for event in game.0.simulate_hour(&data.exe, &data.sim_texts, &mut random) {
        commands.trigger(Tell(event));
        shown = true;
    }
    let events = game.0.travel_hour();
    report(game, data, &events, commands);
    for event in &events {
        if let TravelEvent::Arrived(n) = *event {
            for event in game.0.arrival(&data.exe, &data.sim_texts, n, &mut random) {
                commands.trigger(Tell(event));
                shown = true;
            }
        }
    }
    // FUN_1b8a_006e: the people of the pub.
    for event in game.0.pub_hour(&data.exe, &data.sim_texts) {
        commands.trigger(Tell(event));
        shown = true;
    }
    // FUN_1b8a_3546: the aliens.
    let (reports, attack) = game.0.aliens_hour(&data.exe, &data.sim_texts, &mut random);
    for event in reports {
        commands.trigger(Tell(event));
        shown = true;
    }
    if let Some(attack) = attack {
        commands.insert_resource(BattleStart { place: attack.place, you_attack: false, ground: attack.ground });
        commands.trigger(GoTo(GameScreen::SpaceBattle));
        shown = true;
    }
    shown || events.iter().any(|e| matches!(e, TravelEvent::Explored(..)))
}

/// FUN_1b8a_2a0e's messages: "<group> arrived to <star> <planet>" in the
/// log, and a box when a group explored a system.
fn report(game: &mut Game, data: &GameData, events: &[TravelEvent], commands: &mut Commands) {
    for event in events {
        match *event {
            TravelEvent::Arrived(n) => {
                let Some(group) = game.0.unit(UnitList::Groups, n).map(<[u8]>::to_vec) else {
                    continue;
                };
                let (s, p, m) = (group[0x13], group[0x14], group[0x15]);
                let star = star_name(data, s);
                let mut place = game.body_name(&data.star_systems, s.into(), p.into(), 0);
                if m > 0 {
                    place = format!("{place} {}", game.body_name(&data.star_systems, s.into(), p.into(), m.into()));
                }
                let text = format!("{} arrived to {star} {place}", unit_name(&group));
                game.0.add_message(2, &text);
            }
            TravelEvent::Explored(_, system) => {
                let star = star_name(data, system);
                commands.trigger(ShowMessage::new(format!(
                    " Your ships explored {star} system |    They found new planets"
                )));
            }
        }
    }
}

fn star_name(data: &GameData, system: u8) -> String {
    data.star_systems
        .get((system as usize).wrapping_sub(1))
        .map(|s| s.name.trim_end().to_string())
        .unwrap_or_default()
}
