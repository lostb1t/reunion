//! Game time. The original counts VGA frames in its main loop (DS:0x95f6)
//! and advances an hour once the count passes 100, so an hour is 101 frames
//! of the 70.086 Hz VGA refresh, about 1.44 seconds.

use bevy::prelude::*;

use crate::game::Game;
use crate::screen::in_game;

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

fn tick(time: Res<Time>, mut frames: ResMut<Frames>, mut game: ResMut<Game>) {
    frames.0 += time.delta_secs_f64() * VGA_REFRESH_HZ;
    while frames.0 >= FRAMES_PER_HOUR {
        frames.0 -= FRAMES_PER_HOUR;
        game.0.advance_hour();
    }
}
