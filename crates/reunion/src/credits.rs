//! GAME CREDITS (screen 10), opened from DISK OPERATIONS.
//!
//! From REUNION.PRG FUN_264a_01b8: the picture area filled with color 0x40
//! (black: pictures are loaded with their colors moved up by 0x40, so 0x40
//! is a picture's color 0), then the credits from code segment 0x264a: the
//! title red at (53, 53), the rest yellow from x 50, 9 rows apart.

use bevy::prelude::*;
use bevy::sprite::Anchor;

use crate::game_data::{GameData, GameDataHandle};
use crate::hud::{CONTENT_Y, RED_TEXT, YELLOW_TEXT};
use crate::screen::{GAME_WIDTH, GAME_HEIGHT, GameScreen, place};

pub struct CreditsPlugin;

impl Plugin for CreditsPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(OnEnter(GameScreen::Credits), enter)
            .add_systems(Update, render.run_if(in_state(GameScreen::Credits)));
    }
}

/// Color 0x40: color 0 of the screens' pictures.
const BACKGROUND: Color = Color::BLACK;

/// y, x, columns, text; the first line is red.
const LINES: [(f32, f32, usize, &str); 14] = [
    (53.0, 53.0, 40, "             R E U N I O N"),
    (62.0, 50.0, 40, "                   by"),
    (71.0, 50.0, 40, "             Amnesty Design"),
    (80.0, 50.0, 40, ""),
    (89.0, 50.0, 40, "     PROGRAMMED BY:   ISTVAN KISS"),
    (98.0, 50.0, 40, "  AMIGA VERSION BY:   JANOS KISTAMAS"),
    (107.0, 50.0, 40, "                      KRISZTIAN JAMBOR"),
    (116.0, 50.0, 40, "         DESIGN BY:   CSABA GYARMATI"),
    (125.0, 50.0, 40, "        ARTWORK BY:   TAMAS FODOR"),
    (134.0, 50.0, 40, "          MUSIC BY:   TAMAS KREINER"),
    (152.0, 50.0, 40, "   PROGRAM MANAGER:   STEVE SARGENT"),
    (161.0, 50.0, 40, "                      GABOR FEHER"),
    (179.0, 50.0, 42, "COPYRIGHT (C) 1994 GRANDSLAM VIDEO LIMITED"),
    (191.0, 50.0, 40, "            ALL RIGHTS RESERVED"),
];

#[derive(Component)]
struct CreditLine(usize);

fn enter(mut commands: Commands) {
    let scoped = DespawnOnExit(GameScreen::Credits);
    commands.spawn((
        Sprite::from_color(BACKGROUND, Vec2::new(GAME_WIDTH, GAME_HEIGHT - CONTENT_Y)),
        Anchor::TOP_LEFT,
        place(Vec2::new(0.0, CONTENT_Y), 0.0),
        scoped.clone(),
    ));
    for (i, &(y, x, _, _)) in LINES.iter().enumerate() {
        commands.spawn((
            CreditLine(i),
            Sprite::default(),
            Anchor::TOP_LEFT,
            place(Vec2::new(x, y), 1.0),
            Visibility::Hidden,
            scoped.clone(),
        ));
    }
}

fn render(
    handle: Res<GameDataHandle>,
    data: Res<Assets<GameData>>,
    mut lines: Query<(&CreditLine, &mut Sprite, &mut Visibility)>,
    mut images: ResMut<Assets<Image>>,
) {
    let Some(data) = data.get(&handle.0) else {
        return;
    };
    for (CreditLine(i), mut sprite, mut visibility) in &mut lines {
        if *visibility != Visibility::Hidden {
            continue;
        }
        let (_, _, columns, text) = LINES[*i];
        let colors = if *i == 0 { RED_TEXT } else { YELLOW_TEXT };
        sprite.image = images.add(data.font.render(text, columns, colors));
        *visibility = Visibility::Inherited;
    }
}
