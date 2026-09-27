//! Talking to your staff (screen 24), from the people in the control room.
//!
//! From REUNION.PRG FUN_1991_0000 and the rest of segment 1991: GRAFIKA/DUMA
//! at row 49, the person (FACES<category>, the hired candidate's 98x109
//! frame) at (5, 90) and you (GRAFIKA/YOU) at (217, 90). The conversation
//! has states: each offers up to five questions (DS:0xbc + 6 * state: a
//! count, then question numbers; short forms from TEXT/RKERDES1.SP at
//! (112, 93), 9 apart). Asking shows the question (TEXT/KERDES1.SP) and an
//! answer (TEXT/VALASZ1.SP: "NN text|second line", NN the next state, 00
//! ending the talk). Some answers act:
//!
//! - "What do you require" / "shall we develop" / "shall we produce":
//!   inventions this person cares about (DS:0x64ec: person, invention),
//!   with a reason from TEXT/AJANLAS.SP;
//! - university: it costs (random 0-9 + the person's level) x 1000; paying
//!   sends them away for 50-69 hours (DS:0x5d9e, 0x5d9c). The developer
//!   picks a course (DS:0x5da0) instead, unless already skilled enough.

use bevy::prelude::*;
use bevy::sprite::Anchor;

use crate::focus::{Activated, DefaultFocus, Hover, hotspot};
use crate::game::Game;
use crate::game_data::{GameData, GameDataHandle};
use crate::hud::{CONTENT_Y, HoverLabel, RED_TEXT, YELLOW_TEXT};
use crate::screen::{GameScreen, picture, place};
use crate::text::{Label, label};
use crate::transition::GoTo;

pub struct StaffTalkPlugin;

impl Plugin for StaffTalkPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(OnEnter(GameScreen::StaffTalk), enter)
            .add_systems(Update, spawn_view.run_if(in_state(GameScreen::StaffTalk)))
            .add_observer(ask);
    }
}

/// Who is being talked to: commander category 1-4 (pilot, builder,
/// fighter, developer).
#[derive(Resource, Clone, Copy)]
pub struct TalkTo(pub u16);

/// The conversation: its state, and the last question and answer lines.
#[derive(Resource, Clone, PartialEq)]
struct Talk {
    state: u8,
    asked: Option<u8>,
    answer: [String; 2],
}

#[derive(Component, Clone)]
struct ViewPart;

#[derive(Component, Clone, Copy)]
struct Question(u8);

const HIRED: u16 = 0x95b4;
const LEVEL: u16 = 0x95a2;
const AT_UNIVERSITY: u16 = 0x5d9e;
const UNIVERSITY_HOURS: u16 = 0x5d9c;
const COURSE: u16 = 0x5da0;
const COST: u16 = 0x5da2;
const DEVELOPER: u16 = 4;

fn enter(mut commands: Commands) {
    commands.insert_resource(Talk {
        state: 1,
        asked: None,
        answer: Default::default(),
    });
}

fn line(lines: &[String], n: u8) -> String {
    lines.get(usize::from(n).wrapping_sub(1)).cloned().unwrap_or_default()
}

fn spawn_view(
    mut commands: Commands,
    talk: Option<Res<Talk>>,
    who: Option<Res<TalkTo>>,
    game: Option<Res<Game>>,
    handle: Res<GameDataHandle>,
    data: Res<Assets<GameData>>,
    asset_server: Res<AssetServer>,
    parts: Query<Entity, With<ViewPart>>,
) {
    let (Some(talk), Some(who), Some(game), Some(data)) = (talk, who, game, data.get(&handle.0))
    else {
        return;
    };
    if !talk.is_changed() {
        return;
    }
    for part in &parts {
        commands.entity(part).despawn();
    }
    let scoped = (ViewPart, DespawnOnExit(GameScreen::StaffTalk));
    commands.spawn((
        picture(asset_server.load("GRAFIKA/DUMA.PIC"), Vec2::new(0.0, CONTENT_Y)),
        scoped.clone(),
    ));
    // You and them.
    let hero = game.0.word(0x9276).unwrap_or(1);
    let you = if hero == 2 { 1.0 } else { 101.0 };
    let hired = game.0.word(HIRED + 2 * who.0).unwrap_or(1).max(1);
    let face = 5.0 + 106.0 * f32::from(hired - 1);
    for (path, rect, at) in [
        ("GRAFIKA/YOU.PIC".to_string(), Rect::new(you, 1.0, you + 100.0, 110.0), 217.0),
        (
            format!("GRAFIKA/FACES{}.PIC", who.0),
            Rect::new(face, 1.0, face + 98.0, 110.0),
            5.0,
        ),
    ] {
        commands.spawn((
            Sprite {
                image: asset_server.load(path),
                rect: Some(rect),
                ..default()
            },
            Anchor::TOP_LEFT,
            place(Vec2::new(at, 90.0), 0.5),
            scoped.clone(),
        ));
    }
    let text = |commands: &mut Commands, s: String, columns: usize, colors, pos: Vec2| {
        commands.spawn((label(Label::new(s, columns, colors), pos), scoped.clone()));
    };
    if let Some(q) = talk.asked {
        text(&mut commands, line(&data.talk.questions, q), 52, RED_TEXT, Vec2::new(6.0, 54.0));
    }
    for (k, answer) in talk.answer.iter().enumerate() {
        text(&mut commands, answer.clone(), 51, YELLOW_TEXT, Vec2::new(12.0, 62.0 + 8.0 * k as f32));
    }
    // FUN_1991_08de / 0946: the questions of this state.
    let set = data
        .exe
        .ds_bytes(0xbc + 6 * u16::from(talk.state), 6)
        .unwrap_or(&[0; 6]);
    for (k, &q) in set[1..=usize::from(set[0]).min(5)].iter().enumerate() {
        let y = 93.0 + 9.0 * k as f32;
        let short = line(&data.talk.short_questions, q);
        text(&mut commands, short.clone(), 16, YELLOW_TEXT, Vec2::new(112.0, y));
        let mut entity = commands.spawn((
            Question(q),
            HoverLabel("Talking".into()),
            hotspot(Rect::new(112.0, y, 208.0, y + 9.0), Hover::Outline),
            scoped.clone(),
        ));
        if k == 0 {
            entity.insert(DefaultFocus);
        }
    }
}

/// Inventions this person has an opinion on (DS:0x64ec, nine of them):
/// the last one being developed (`done` false) or finished (`done` true),
/// and its index for TEXT/AJANLAS.SP.
fn opinion(game: &Game, data: &GameData, person: u16, done: bool) -> Option<(usize, u8)> {
    (1..=9u16)
        .filter_map(|i| {
            let entry = data.exe.ds_bytes(0x64ec + 4 * i, 4)?;
            let (p, invention) = (u16::from_le_bytes([entry[0], entry[1]]), entry[2]);
            let status = game.0.word(0x5d77 + 0x35 * u16::from(invention) + 0x11).unwrap_or(0);
            let wanted = if done { status == 5 } else { (1..=4).contains(&status) };
            (p == person && wanted).then_some((i as usize, invention))
        })
        .next_back()
}

fn random(n: u32) -> u32 {
    use std::hash::{BuildHasher, RandomState};
    (RandomState::new().hash_one(n) % u64::from(n.max(1))) as u32
}

fn ask(
    activated: On<Activated>,
    questions: Query<&Question>,
    talk: Option<ResMut<Talk>>,
    who: Option<Res<TalkTo>>,
    game: Option<ResMut<Game>>,
    handle: Res<GameDataHandle>,
    data: Res<Assets<GameData>>,
    mut commands: Commands,
) {
    let (Ok(&Question(q)), Some(mut talk), Some(who), Some(mut game), Some(data)) = (
        questions.get(activated.0),
        talk,
        who,
        game,
        data.get(&handle.0),
    ) else {
        return;
    };
    let person = who.0;
    let word = |game: &Game, at: u16| game.0.word(at).unwrap_or(0);
    let hired = word(&game, HIRED + 2 * person);
    let level = word(&game, LEVEL + 2 * person);
    let byte = |at: u16| data.exe.ds_bytes(at, 1).map_or(0, |b| b[0]);
    // The developer's skill limits per course (DS:0x57ec: hero, candidate).
    let course_done = |game: &Game, skills: &[usize]| {
        let base = 0x57ec + 12 * word(game, 0x91ea) + 4 * word(game, 0x95bc);
        skills
            .iter()
            .all(|&s| u16::from(byte(base + s as u16)) <= word(game, 0x95ac + 2 * s as u16))
    };
    // FUN_1991_0000: which answer.
    let answer: u8 = match q {
        1 => 2 + random(2) as u8,
        2 => {
            if random(50) < 10 {
                4
            } else if opinion(&game, data, person, false).is_none() {
                6
            } else {
                5
            }
        }
        3 => 7,
        4 => 8,
        5 => {
            let mut a = if word(&game, AT_UNIVERSITY) > 0 {
                9
            } else if person == DEVELOPER {
                14
            } else {
                10
            };
            if person != DEVELOPER && u16::from(byte(0x57e0 + 3 * person + hired)) <= level {
                a = 15;
            }
            a
        }
        6 => {
            let cost = u32::from(word(&game, COST)) | u32::from(word(&game, COST + 2)) << 16;
            if game.0.money() < cost {
                12
            } else {
                let money = game.0.money() - cost;
                game.0.set_word(0x95be, money as u16);
                game.0.set_word(0x95c0, (money >> 16) as u16);
                game.0.set_word(AT_UNIVERSITY, person);
                game.0.set_word(UNIVERSITY_HOURS, 50 + random(20) as u16);
                13
            }
        }
        7 => 11,
        8..=11 => {
            let course = q - 7;
            game.0.set_word(COURSE, course.into());
            let skills: &[usize] = match course {
                1 => &[0, 1],
                2 => &[1, 2],
                3 => &[0, 2],
                _ => &[3],
            };
            if course_done(&game, skills) { 15 } else { 10 }
        }
        _ => return,
    };
    // FUN_1991_0580: "NN first line|second line".
    let full = line(&data.talk.answers, answer);
    let next: u8 = full.get(..2).and_then(|n| n.trim().parse().ok()).unwrap_or(0);
    let rest = full.get(3..).unwrap_or("");
    let (first, second) = rest.split_once('|').unwrap_or((rest, ""));
    let mut lines = [first.to_string(), second.to_string()];
    let invention_name = |i: u8| {
        data.exe
            .ds_string(0x5d77 + 0x35 * u16::from(i))
            .unwrap_or_default()
            .trim_end()
            .to_string()
    };
    match answer {
        7 | 8 => {
            let done = answer == 8;
            lines = match opinion(&game, data, person, done) {
                None => [
                    if done {
                        "I don't need anything right now.".into()
                    } else {
                        "I don't need anything right now".into()
                    },
                    String::new(),
                ],
                Some((index, invention)) => {
                    let name = invention_name(invention);
                    let first = match (done, random(50) < 25) {
                        (false, true) => format!("We could develop a {name}"),
                        (false, false) => format!("The most important thing would be a {name}"),
                        (true, true) => format!("We might need {name}."),
                        (true, false) => format!("We really need more {name}s"),
                    };
                    [first, line(&data.talk.reasons, index as u8)]
                }
            };
        }
        10 => {
            let cost = (random(10) + u32::from(level)) * 1000;
            game.0.set_word(COST, cost as u16);
            game.0.set_word(COST + 2, (cost >> 16) as u16);
            lines = [format!("It will be {cost} credits"), String::new()];
        }
        _ => {}
    }
    if next == 0 {
        commands.trigger(GoTo(GameScreen::MainScreen));
    }
    *talk = Talk {
        state: next.max(1),
        asked: Some(q),
        answer: lines,
    };
}
