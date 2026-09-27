//! MESSAGES (screen 11): the message log.
//!
//! From REUNION.PRG FUN_34b0_0c14: UZENET.PIC (the screen table's background
//! for screen 11) at row 49, one message per row at x 10, y 48 + 9 * row, up
//! to 50 characters; kinds 1 and 99 in the highlight color, the rest yellow.

use bevy::prelude::*;
use bevy::sprite::Anchor;
use reunion_formats::state::MAX_MESSAGES;

use crate::game::Game;
use crate::game_data::{GameData, GameDataHandle};
use crate::hud::{CONTENT_Y, RED_TEXT, YELLOW_TEXT};
use crate::screen::{GameScreen, picture, place};

pub struct MessagesPlugin;

impl Plugin for MessagesPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(OnEnter(GameScreen::Messages), enter)
            .add_systems(Update, update_rows.run_if(in_state(GameScreen::Messages)));
    }
}

const ROW_X: f32 = 10.0;
const ROW_Y: f32 = 48.0;
const ROW_HEIGHT: f32 = 9.0;
const COLUMNS: usize = 50;
const HIGHLIGHTED: [u16; 2] = [1, 99];

#[derive(Component)]
struct Row(usize);

fn enter(mut commands: Commands, asset_server: Res<AssetServer>) {
    let scoped = DespawnOnExit(GameScreen::Messages);
    commands.spawn((
        picture(
            asset_server.load("GRAFIKA/UZENET.PIC"),
            Vec2::new(0.0, CONTENT_Y),
        ),
        scoped.clone(),
    ));
    for row in 0..MAX_MESSAGES {
        let y = ROW_Y + ROW_HEIGHT * (row + 1) as f32;
        commands.spawn((
            Row(row),
            Sprite::default(),
            Anchor::TOP_LEFT,
            place(Vec2::new(ROW_X, y), 1.0),
            // Hidden until it has a message: an image-less sprite draws a white dot.
            Visibility::Hidden,
            scoped.clone(),
        ));
    }
}

fn update_rows(
    game: Option<Res<Game>>,
    handle: Res<GameDataHandle>,
    data: Res<Assets<GameData>>,
    mut rows: Query<(&Row, &mut Sprite, &mut Visibility)>,
    added: Query<(), Added<Row>>,
    mut images: ResMut<Assets<Image>>,
) {
    let (Some(game), Some(data)) = (game, data.get(&handle.0)) else {
        return;
    };
    if !game.is_changed() && added.is_empty() {
        return;
    }
    let messages = game.0.messages();
    for (Row(row), mut sprite, mut visibility) in &mut rows {
        let Some((kind, text)) = messages.get(*row) else {
            *visibility = Visibility::Hidden;
            continue;
        };
        let colors = if HIGHLIGHTED.contains(kind) {
            RED_TEXT
        } else {
            YELLOW_TEXT
        };
        sprite.image = images.add(data.font.render(text, COLUMNS, colors));
        *visibility = Visibility::Inherited;
    }
}
