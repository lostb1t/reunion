//! Talking with aliens (screen 34), from REUNION.PRG FUN_2e4b_0400 - 092e
//! and the main loop.
//!
//! PICS/ATVEZETO at row 49 with you (PICS/SAJAT<hero>, 65x80 at (23, 50)),
//! the seat (PICS/SZEK<n>, 106x80 at (115, 50)) and the alien
//! (ALIEN/ALIEN<n>, 95x149 at (224, 50)), n from DS:0xd0d. Below, from
//! (6, 134), 36 columns: what you may say (TEXT/KERDES<talk>.AT, lines
//! "NN text" chosen by the conversation's state, DS:0xcde), then the
//! alien's reply (TEXT/VALASZ<talk>.AT line NN, itself "NN text": the next
//! state, 00 ending the talk). Some of your lines change the story.

use bevy::prelude::*;
use bevy::sprite::Anchor;

use crate::focus::{Activated, DefaultFocus, Hover, hotspot};
use crate::game::{Game, random};
use crate::game_data::{GameData, GameDataHandle};
use crate::hud::{CONTENT_Y, HoverLabel, RED_TEXT, YELLOW_TEXT};
use crate::screen::{GameScreen, picture, place};
use crate::story::{Showing, Tell};
use crate::text::{Label, label};
use crate::transition::GoTo;

pub struct AlienTalkPlugin;

impl Plugin for AlienTalkPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(OnEnter(GameScreen::AlienTalk), enter)
            .add_systems(Update, spawn_text.run_if(in_state(GameScreen::AlienTalk)))
            .add_observer(choose);
    }
}

#[derive(Resource, Clone, PartialEq)]
struct Talk {
    number: u8,
    state: u8,
    /// The alien's reply being shown (then a click goes on).
    reply: Option<Vec<String>>,
}

#[derive(Component, Clone)]
struct TextPart;

/// One of your lines (its KERDES line number), or the reply (0).
#[derive(Component, Clone, Copy)]
struct Choice(u8);

const TEXT_TOP: f32 = 134.0;
const COLUMNS: usize = 36;

fn enter(
    mut commands: Commands,
    showing: Option<Res<Showing>>,
    game: Option<Res<Game>>,
    handle: Res<GameDataHandle>,
    data: Res<Assets<GameData>>,
    asset_server: Res<AssetServer>,
) {
    let number = showing.map_or(2, |s| s.0);
    commands.insert_resource(Talk { number, state: 1, reply: None });
    let scoped = DespawnOnExit(GameScreen::AlienTalk);
    commands.spawn((picture(asset_server.load("PICS/ATVEZETO.PIC"), Vec2::new(0.0, CONTENT_Y)), scoped.clone()));
    let (Some(game), Some(data)) = (game, data.get(&handle.0)) else { return };
    let alien = data.exe.talk_picture(number);
    let hero = game.0.word(0x9276).unwrap_or(1);
    for (path, rect, at) in [
        (format!("PICS/SAJAT{hero}.PIC"), Rect::new(1.0, 1.0, 66.0, 81.0), Vec2::new(23.0, 50.0)),
        (format!("PICS/SZEK{alien}.PIC"), Rect::new(1.0, 1.0, 107.0, 81.0), Vec2::new(115.0, 50.0)),
        (format!("ALIEN/ALIEN{alien}.PIC"), Rect::new(2.0, 1.0, 97.0, 150.0), Vec2::new(224.0, 50.0)),
    ] {
        commands.spawn((
            Sprite { image: asset_server.load(path), rect: Some(rect), ..default() },
            Anchor::TOP_LEFT,
            place(at, 0.5),
            scoped.clone(),
        ));
    }
}

/// "NN text": the number and the text's lines.
fn split(line: &str) -> (u8, Vec<String>) {
    let next = line.get(..2).and_then(|n| n.trim().parse().ok()).unwrap_or(0);
    let text = line.get(3..).unwrap_or("");
    (next, text.split('|').map(|l| l.trim_end().to_string()).collect())
}

fn line(lines: &[String], n: u8) -> String {
    lines.get(usize::from(n).wrapping_sub(1)).cloned().unwrap_or_default()
}

fn spawn_text(
    mut commands: Commands,
    talk: Option<Res<Talk>>,
    handle: Res<GameDataHandle>,
    data: Res<Assets<GameData>>,
    parts: Query<Entity, With<TextPart>>,
) {
    let (Some(talk), Some(data)) = (talk, data.get(&handle.0)) else { return };
    if !talk.is_changed() && !parts.is_empty() {
        return;
    }
    for part in &parts {
        commands.entity(part).despawn();
    }
    let scoped = (TextPart, DespawnOnExit(GameScreen::AlienTalk));
    let (questions, answers) = data.alien_talks.get(usize::from(talk.number)).cloned().unwrap_or_default();
    let _ = answers;
    let mut row = 0usize;
    let text = |commands: &mut Commands, s: &str, colors, row: usize| {
        let at = Vec2::new(6.0, TEXT_TOP + 9.0 * row as f32);
        commands.spawn((label(Label::new(s, COLUMNS, colors), at), scoped.clone()));
    };
    match &talk.reply {
        Some(reply) => {
            for l in reply.iter().take(7) {
                text(&mut commands, l, YELLOW_TEXT, row);
                row += 1;
            }
            commands.spawn((
                Choice(0),
                HoverLabel("Talking".into()),
                hotspot(Rect::new(6.0, TEXT_TOP, 222.0, TEXT_TOP + 63.0), Hover::Outline),
                DefaultFocus,
                scoped.clone(),
            ));
        }
        None => {
            for (k, q) in data.exe.talk_choices(talk.number, talk.state).into_iter().enumerate() {
                let (_, lines) = split(&line(&questions, q));
                let top = row;
                for l in &lines {
                    if row >= 7 {
                        break;
                    }
                    text(&mut commands, l, RED_TEXT, row);
                    row += 1;
                }
                let rect = Rect::new(6.0, TEXT_TOP + 9.0 * top as f32, 222.0, TEXT_TOP + 9.0 * row as f32);
                let mut e = commands.spawn((Choice(q), HoverLabel("Talking".into()), hotspot(rect, Hover::Outline), scoped.clone()));
                if k == 0 {
                    e.insert(DefaultFocus);
                }
            }
        }
    }
}

fn choose(
    activated: On<Activated>,
    choices: Query<&Choice>,
    talk: Option<ResMut<Talk>>,
    game: Option<ResMut<Game>>,
    handle: Res<GameDataHandle>,
    data: Res<Assets<GameData>>,
    mut commands: Commands,
) {
    let (Ok(&Choice(q)), Some(mut talk), Some(mut game), Some(data)) =
        (choices.get(activated.0), talk, game, data.get(&handle.0))
    else {
        return;
    };
    let (questions, answers) = data.alien_talks.get(usize::from(talk.number)).cloned().unwrap_or_default();
    if q == 0 {
        // After the reply: the next choices, or the end.
        if talk.state == 0 {
            for event in game.0.talk_end(talk.number, &data.sim_texts) {
                commands.trigger(Tell(event));
            }
            commands.trigger(GoTo(GameScreen::MainScreen));
        } else {
            talk.reply = None;
        }
        return;
    }
    let (reply_line, _) = split(&line(&questions, q));
    game.0.talk_choice(talk.number, q, &mut random);
    let (next, reply) = split(&line(&answers, reply_line));
    talk.state = next;
    talk.reply = Some(reply);
}
