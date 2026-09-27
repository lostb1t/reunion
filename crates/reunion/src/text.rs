//! Text in the game's font that redraws itself when it changes.

use bevy::input::keyboard::{Key, KeyboardInput};
use bevy::prelude::*;
use bevy::sprite::Anchor;

use crate::game_data::{GameData, GameDataHandle};
use crate::screen::place;

pub struct TextPlugin;

impl Plugin for TextPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(PreUpdate, type_text.after(bevy::input::InputSystems))
            .add_systems(PostUpdate, render_labels);
    }
}

/// A line of `columns` characters drawn with the original's text colors
/// (glyph background, then the two glyph shades).
#[derive(Component, Clone, PartialEq)]
pub struct Label {
    pub text: String,
    pub columns: usize,
    pub colors: [[u8; 4]; 3],
}

impl Label {
    pub fn new(text: impl Into<String>, columns: usize, colors: [[u8; 4]; 3]) -> Self {
        Self {
            text: text.into(),
            columns,
            colors,
        }
    }

    /// Changes the text, leaving change detection alone if it's the same.
    pub fn set(mut this: Mut<Self>, text: impl Into<String>) {
        let text = text.into();
        if this.text != text {
            this.text = text;
        }
    }
}

/// A label with its top-left corner at `pos` (game coordinates).
pub fn label(label: Label, pos: Vec2) -> impl Bundle {
    (
        label,
        Sprite::default(),
        Anchor::TOP_LEFT,
        place(pos, 1.0),
        // Until drawn: an image-less sprite shows as a white dot.
        Visibility::Hidden,
    )
}

/// Typing into a field (unit names, like the original's FUN_398f_0059):
/// keys add characters to `text` until Enter, Escape or the gamepad's confirm
/// button sets `done`. Confirm and back input are held off meanwhile. The
/// screen that started it removes it when done.
#[derive(Resource)]
pub struct TextEditing {
    pub text: String,
    pub max: usize,
    pub done: bool,
}

impl TextEditing {
    pub fn new(text: impl Into<String>, max: usize) -> Self {
        Self {
            text: text.into(),
            max,
            done: false,
        }
    }
}

fn type_text(
    editing: Option<ResMut<TextEditing>>,
    mut keys: MessageReader<KeyboardInput>,
    gamepads: Query<&Gamepad>,
) {
    let Some(mut editing) = editing else {
        keys.clear();
        return;
    };
    if editing.done {
        return;
    }
    for key in keys.read() {
        if !key.state.is_pressed() {
            continue;
        }
        match &key.logical_key {
            Key::Enter | Key::Escape => editing.done = true,
            Key::Backspace => {
                editing.text.pop();
            }
            Key::Character(c) => {
                for ch in c.chars().filter(|c| c.is_ascii() && !c.is_ascii_control()) {
                    if editing.text.len() < editing.max {
                        editing.text.push(ch);
                    }
                }
            }
            Key::Space if editing.text.len() < editing.max => editing.text.push(' '),
            _ => {}
        }
    }
    if gamepads
        .iter()
        .any(|g| g.just_pressed(GamepadButton::South) || g.just_pressed(GamepadButton::East))
    {
        editing.done = true;
    }
}

/// Labels not drawn yet (the font wasn't loaded) are retried.
#[derive(Component)]
struct Drawn;

fn render_labels(
    handle: Option<Res<GameDataHandle>>,
    data: Res<Assets<GameData>>,
    mut labels: Query<
        (Entity, &Label, &mut Sprite, &mut Visibility),
        Or<(Changed<Label>, Without<Drawn>)>,
    >,
    mut images: ResMut<Assets<Image>>,
    mut commands: Commands,
) {
    let Some(data) = handle.and_then(|h| data.get(&h.0)) else {
        return;
    };
    for (entity, label, mut sprite, mut visibility) in &mut labels {
        sprite.image = images.add(data.font.render(&label.text, label.columns, label.colors));
        if *visibility == Visibility::Hidden {
            *visibility = Visibility::Inherited;
        }
        commands.entity(entity).try_insert(Drawn);
    }
}
