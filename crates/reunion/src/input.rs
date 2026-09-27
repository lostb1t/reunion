//! Menu input actions, shared by mouse, keyboard and gamepad.

use bevy::prelude::*;
use bevy_enhanced_input::prelude::*;

pub struct InputPlugin;

impl Plugin for InputPlugin {
    fn build(&self, app: &mut App) {
        app.add_input_context::<UiInput>()
            .add_systems(Startup, spawn_ui_input);
    }
}

#[derive(Component)]
pub struct UiInput;

/// Move focus to the next hotspot in a direction. Repeats while held.
#[derive(InputAction)]
#[action_output(Vec2)]
pub struct Navigate;

/// Scroll a map (planet surface). Repeats while held.
#[derive(InputAction)]
#[action_output(Vec2)]
pub struct Scroll;

/// Activate the focused hotspot.
#[derive(InputAction)]
#[action_output(bool)]
pub struct Confirm;

/// Leave the current screen. The original uses the right mouse button for this.
#[derive(InputAction)]
#[action_output(bool)]
pub struct Back;

/// The other use of a hotspot, like taking one away where confirming adds one.
#[derive(InputAction)]
#[action_output(bool)]
pub struct Alternate;

/// Left mouse button: activates the hotspot under the cursor.
#[derive(InputAction)]
#[action_output(bool)]
pub struct Click;

fn spawn_ui_input(mut commands: Commands) {
    commands.spawn((
        UiInput,
        actions!(UiInput[
            (
                Action::<Navigate>::new(),
                DeadZone::default(),
                Pulse::new(0.12).with_initial_delay(0.35),
                Bindings::spawn((Cardinal::arrows(), Cardinal::dpad(), Axial::left_stick())),
            ),
            (
                Action::<Scroll>::new(),
                DeadZone::default(),
                Pulse::new(0.08).with_initial_delay(0.25),
                Bindings::spawn((Cardinal::wasd_keys(), Axial::right_stick())),
            ),
            (
                Action::<Confirm>::new(),
                bindings![KeyCode::Enter, KeyCode::NumpadEnter, KeyCode::Space, GamepadButton::South],
            ),
            (
                Action::<Back>::new(),
                bindings![KeyCode::Escape, KeyCode::Backspace, GamepadButton::East, MouseButton::Right],
            ),
            (
                Action::<Alternate>::new(),
                bindings![KeyCode::Delete, KeyCode::Minus, KeyCode::NumpadSubtract, GamepadButton::West, MouseButton::Middle],
            ),
            (
                Action::<Click>::new(),
                bindings![MouseButton::Left],
            ),
        ]),
    ));
}
