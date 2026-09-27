//! Fade to black between screens, like the original's palette fades.

use bevy::prelude::*;

use crate::screen::GameScreen;

const FADE_SECONDS: f32 = 0.35;

pub struct TransitionPlugin;

impl Plugin for TransitionPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Fade>()
            .add_observer(go_to)
            .add_systems(Startup, spawn_overlay)
            .add_systems(Update, animate);
    }
}

/// Fade out, switch to the given screen, fade back in.
#[derive(Event)]
pub struct GoTo(pub GameScreen);

#[derive(Resource)]
enum Fade {
    Idle,
    Out { to: GameScreen, t: f32 },
    In { t: f32 },
}

impl Default for Fade {
    // The first screen fades in too.
    fn default() -> Self {
        Fade::In { t: 0.0 }
    }
}

#[derive(Component)]
struct Overlay;

fn spawn_overlay(mut commands: Commands) {
    // Big enough to cover any window shape; drawn above every screen.
    commands.spawn((
        Overlay,
        Sprite::from_color(Color::BLACK, Vec2::splat(10_000.0)),
        Transform::from_xyz(0.0, 0.0, 100.0),
    ));
}

fn go_to(go: On<GoTo>, mut fade: ResMut<Fade>) {
    // Ignore requests mid-fade, so a double press can't skip a screen.
    if matches!(*fade, Fade::Idle) {
        *fade = Fade::Out { to: go.0, t: 0.0 };
    }
}

fn animate(
    time: Res<Time>,
    mut fade: ResMut<Fade>,
    mut next: ResMut<NextState<GameScreen>>,
    mut overlay: Single<&mut Sprite, With<Overlay>>,
) {
    let step = time.delta_secs() / FADE_SECONDS;
    let (alpha, after) = match *fade {
        Fade::Idle => return,
        Fade::Out { to, t } => {
            let t = t + step;
            if t >= 1.0 {
                next.set(to);
                (1.0, Fade::In { t: 0.0 })
            } else {
                (t, Fade::Out { to, t })
            }
        }
        Fade::In { t } => {
            let t = t + step;
            if t >= 1.0 {
                (0.0, Fade::Idle)
            } else {
                (1.0 - t, Fade::In { t })
            }
        }
    };
    *fade = after;
    overlay.color = Color::BLACK.with_alpha(alpha);
}
