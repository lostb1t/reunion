//! Message boxes (REUNION.PRG FUN_34b0_0237): text split into lines at `|`,
//! in a box of palette color 2 with a color 4 frame, centered on (160, 124),
//! until a button is pressed. Game events (FUN_34b0_011a) show line n of
//! TEXT/MESSAGE.TXT this way and also put it in the message log.
//!
//! While a box is up, hotspots don't take input.

use bevy::prelude::*;
use bevy::sprite::Anchor;
use bevy_enhanced_input::prelude::*;

use crate::audio::Sfx;
use crate::game::Game;
use crate::hud::YELLOW_TEXT;
use crate::input::{Back, Click, Confirm};
use crate::screen::place;
use crate::text::{Label, label};

pub struct PopupPlugin;

impl Plugin for PopupPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Waiting>()
            .add_observer(open)
            .add_observer(close::<Confirm>)
            .add_observer(close::<Back>)
            .add_observer(close::<Click>);
    }
}

/// ICONMAIN palette entries 2 and 4.
const BOX: Color = Color::srgb_u8(160, 160, 192);
const FRAME: Color = Color::srgb_u8(96, 96, 128);
const Z: f32 = 50.0;

/// Shows a message box; `log` also adds it to the message log.
#[derive(Event, Clone)]
pub struct ShowMessage {
    pub text: String,
    pub log: bool,
}

impl ShowMessage {
    pub fn new(text: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            log: false,
        }
    }
}

/// A message box is up.
#[derive(Resource)]
pub struct PopupOpen {
    /// The frame it opened in: the press that closes one box doesn't also
    /// close the next.
    opened: u32,
}

/// Boxes waiting for the one that's up to close.
#[derive(Resource, Default)]
struct Waiting(std::collections::VecDeque<String>);

#[derive(Component)]
struct PopupPart;

fn open(
    show: On<ShowMessage>,
    open: Option<Res<PopupOpen>>,
    game: Option<ResMut<Game>>,
    mut waiting: ResMut<Waiting>,
    frame: Res<bevy::diagnostic::FrameCount>,
    mut commands: Commands,
) {
    if show.log
        && let Some(mut game) = game
    {
        // The log keeps it on one line, without the layout spaces.
        let flat: String = show.text.split('|').map(str::trim).collect::<Vec<_>>().join(" ");
        game.0.add_message(1, &flat);
    }
    if open.is_some() {
        waiting.0.push_back(show.text.clone());
        return;
    }
    let lines: Vec<&str> = show.text.split('|').collect();
    let columns = lines.iter().map(|l| l.len()).max().unwrap_or(0);
    let width = columns as f32 * 6.0;
    let height = lines.len() as f32 * 9.0;
    let x = 160.0 - (columns * 3) as f32;
    let y = 124.0 - (lines.len() * 9 / 2) as f32;
    let rect = |commands: &mut Commands, min: Vec2, size: Vec2, color: Color, z: f32| {
        commands.spawn((
            PopupPart,
            Sprite::from_color(color, size),
            Anchor::TOP_LEFT,
            place(min, z),
        ));
    };
    rect(&mut commands, Vec2::new(x - 3.0, y - 3.0), Vec2::new(width + 6.0, height + 6.0), BOX, Z);
    // One-pixel frame from (x - 2, y - 2) to (x + width + 3, y + height + 3).
    let (x0, y0, x1, y1) = (x - 2.0, y - 2.0, x + width + 3.0, y + height + 3.0);
    for (min, size) in [
        (Vec2::new(x0, y0), Vec2::new(x1 - x0, 1.0)),
        (Vec2::new(x0, y1 - 1.0), Vec2::new(x1 - x0, 1.0)),
        (Vec2::new(x0, y0), Vec2::new(1.0, y1 - y0)),
        (Vec2::new(x1 - 1.0, y0), Vec2::new(1.0, y1 - y0)),
    ] {
        rect(&mut commands, min, size, FRAME, Z + 0.1);
    }
    for (k, line) in lines.iter().enumerate() {
        let mut text = commands.spawn((
            PopupPart,
            label(
                Label::new(*line, columns, YELLOW_TEXT),
                Vec2::new(x, y + 1.0 + 9.0 * k as f32),
            ),
        ));
        text.insert(place(Vec2::new(x, y + 1.0 + 9.0 * k as f32), Z + 0.2));
    }
    // FUN_34b0_011a's boxes (nearly all of them) say "attention".
    commands.trigger(Sfx::named("attentio"));
    commands.insert_resource(PopupOpen { opened: frame.0 });
}

fn close<A: InputAction>(
    _: On<Start<A>>,
    open: Option<Res<PopupOpen>>,
    parts: Query<Entity, With<PopupPart>>,
    mut waiting: ResMut<Waiting>,
    frame: Res<bevy::diagnostic::FrameCount>,
    mut commands: Commands,
) {
    if open.is_none_or(|o| o.opened == frame.0) {
        return;
    }
    for part in &parts {
        commands.entity(part).despawn();
    }
    commands.remove_resource::<PopupOpen>();
    if let Some(text) = waiting.0.pop_front() {
        commands.trigger(ShowMessage::new(text));
    }
}
