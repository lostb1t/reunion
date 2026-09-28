//! SPACE LOCAL, the space station's pub (screen 23), and talking to the
//! people there (screen 25). From REUNION.PRG segment 0x1fd2 and the main
//! loop.
//!
//! GRAFIKA/KOCSMA at row 49. Ten people (27-byte records from DS:0x11f):
//! a name, their picture number, whether they're here (0: part of KOCSMA,
//! 2: drawn from GRAFIKA/PIRATES, others away) and when they're back, and
//! their place. Clicking one talks to them.
//!
//! Talking: GRAFIKA/BESZED (BESZED2 at the bar), the person
//! (ALIEN/KOCSMB<n>, 76x79 at (77, 49)) and their face (ALIEN/ALIEN<pic>,
//! at (224, 50)); what you may say and their replies from
//! TEXT/KERDES<n>.LOC and VALASZ<n>.LOC (the same "NN text" lines as the
//! other conversations, choices from DS:0x3d6). Where the talk starts, and
//! what some lines do, depends on the story so far.

use bevy::prelude::*;
use bevy::sprite::Anchor;

use crate::focus::{Activated, DefaultFocus, Hover, hotspot};
use crate::game::{Game, random};
use crate::game_data::{GameData, GameDataHandle};
use crate::hud::{ActionUsed, CONTENT_Y, HoverLabel, RED_TEXT, YELLOW_TEXT};
use crate::pic::MASKED;
use crate::screen::{GameScreen, picture, place};
use crate::text::{Label, label};
use crate::transition::GoTo;

pub struct PubPlugin;

impl Plugin for PubPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(OnEnter(GameScreen::SpaceLocal), enter_pub)
            .add_systems(OnEnter(GameScreen::PubTalk), enter_talk)
            .add_systems(Update, spawn_text.run_if(in_state(GameScreen::PubTalk)))
            .add_observer(pick_person)
            .add_observer(choose)
            .add_observer(end_talk);
    }
}

const PEOPLE: u16 = 0x11f;
const PERSON_LEN: u16 = 27;
const END_TALK: u8 = 54;
const TEXT_TOP: f32 = 134.0;
const COLUMNS: usize = 36;

/// A person in the pub (1-10).
#[derive(Component, Clone, Copy)]
struct Person(u8);

#[derive(Resource, Clone, Copy)]
struct TalkingTo(u8);

#[derive(Resource, Clone, PartialEq)]
struct Talk {
    person: u8,
    state: u8,
    reply: Option<Vec<String>>,
    /// The Stranger's deal: the race to spy on, and the price.
    target: u8,
    price: u32,
}

#[derive(Component, Clone)]
struct TextPart;

#[derive(Component, Clone, Copy)]
struct Choice(u8);

fn person_byte(game: &Game, n: u8, field: u16) -> u8 {
    game.0.byte(PEOPLE + PERSON_LEN * u16::from(n) + field).unwrap_or(0)
}

fn person_word(game: &Game, n: u8, field: u16) -> u16 {
    game.0.word(PEOPLE + PERSON_LEN * u16::from(n) + field).unwrap_or(0)
}

fn person_name(game: &Game, n: u8) -> String {
    let len = person_byte(game, n, 0).min(13);
    (1..=u16::from(len)).map(|i| char::from(person_byte(game, n, i))).collect::<String>().trim_end().to_string()
}

/// FUN_1fd2_00d9 / 01ef: the pub and who's in it.
fn enter_pub(mut commands: Commands, asset_server: Res<AssetServer>, game: Option<Res<Game>>) {
    let scoped = DespawnOnExit(GameScreen::SpaceLocal);
    commands.spawn((picture(asset_server.load("GRAFIKA/KOCSMA.PIC"), Vec2::new(0.0, CONTENT_Y)), scoped.clone()));
    let Some(game) = game else { return };
    let pirates = asset_server.load::<Image>(format!("GRAFIKA/PIRATES.PIC#{MASKED}"));
    for n in 1..=10u8 {
        let status = person_byte(&game, n, 0x0f);
        let away = person_word(&game, n, 0x10);
        let x = f32::from(person_word(&game, n, 0x16));
        let y = f32::from(person_byte(&game, n, 0x18));
        let (w, h) = (f32::from(person_byte(&game, n, 0x19)), f32::from(person_byte(&game, n, 0x1a)));
        if matches!(status, 2 | 3) && away == 0 {
            let sx = f32::from(person_word(&game, n, 0x13));
            let sy = f32::from(person_byte(&game, n, 0x15));
            commands.spawn((
                Sprite {
                    image: pirates.clone(),
                    rect: Some(Rect::new(sx + 1.0, sy + 1.0, sx + w - 1.0, sy + h - 1.0)),
                    ..default()
                },
                Anchor::TOP_LEFT,
                place(Vec2::new(x + 1.0, y + CONTENT_Y + 1.0), 0.5),
                scoped.clone(),
            ));
        }
        if matches!(status, 0 | 2) && away == 0 {
            let min = Vec2::new(x + 1.0, y + 50.0);
            commands.spawn((
                Person(n),
                HoverLabel(person_name(&game, n)),
                hotspot(Rect::from_corners(min, min + Vec2::new(w - 2.0, h - 2.0)), Hover::Outline),
                scoped.clone(),
            ));
        }
    }
}

fn pick_person(activated: On<Activated>, people: Query<&Person>, mut commands: Commands) {
    if let Ok(&Person(n)) = people.get(activated.0) {
        commands.insert_resource(TalkingTo(n));
        commands.trigger(GoTo(GameScreen::PubTalk));
    }
}

/// Where a talk starts (the main loop, screen 25).
fn first_state(game: &Game, n: u8) -> u8 {
    let b = |at: u16| game.0.byte(at).unwrap_or(0);
    let mut state = 1;
    if n == 1 && b(0x773e) != 0 {
        state = if b(0x164) == 2 && (game.0.word(0x165).unwrap_or(0) as i16) < 1 { 3 } else { 2 };
    }
    if n == 2 && b(0x7740) != 0 && b(0x773f) == 0 {
        state = 2;
    }
    if n == 2 && b(0x773f) != 0 {
        state = if stranger_targets(game, false).is_empty() { 10 } else { 3 };
    }
    if n == 3 && b(0x7749) != 0 {
        state = 2;
    }
    if n == 4 && b(0x7741) == 0 && b(0x221) == 2 {
        state = 3;
    }
    if n == 5 && b(0x7742) != 0 && b(0x7743) == 0 && b(0x705d) == 2 && b(0x7141) == 2 {
        state = 2;
    }
    if n == 6 && b(0x164) == 2 && b(0x7740) == 0 {
        state = 2;
    }
    if n == 6 && b(0x7742) == 0 && (b(0x481a) == 1 || b(0x481b) == 1) && b(0x5d98) == 0 {
        state = 4;
    }
    if n == 6 && b(0x221) == 2 && b(0x7741) == 0 && b(0x7747) == 0 {
        state = 3;
    }
    if n == 7 && b(0x7748) != 0 {
        state = if b(0x7744) == 0 { 6 } else { 2 };
        if b(0x7745) != 0 {
            state = 3;
        }
    }
    if n == 9 && b(0x7741) != 0 && b(0x7746) == 0 {
        state = 2;
    }
    state
}

fn enter_talk(
    mut commands: Commands,
    to: Option<Res<TalkingTo>>,
    game: Option<Res<Game>>,
    asset_server: Res<AssetServer>,
) {
    let (Some(to), Some(game)) = (to, game) else { return };
    let n = to.0;
    commands.insert_resource(Talk { person: n, state: first_state(&game, n), reply: None, target: 0, price: 0 });
    let scoped = DespawnOnExit(GameScreen::PubTalk);
    let background = if n == 6 { "GRAFIKA/BESZED2.PIC" } else { "GRAFIKA/BESZED.PIC" };
    commands.spawn((picture(asset_server.load(background), Vec2::new(0.0, CONTENT_Y)), scoped.clone()));
    if n != 6 {
        commands.spawn((
            Sprite {
                image: asset_server.load(format!("ALIEN/KOCSMB{n}.PIC#{MASKED}")),
                rect: Some(Rect::new(1.0, 1.0, 77.0, 80.0)),
                ..default()
            },
            Anchor::TOP_LEFT,
            place(Vec2::new(77.0, CONTENT_Y), 0.5),
            scoped.clone(),
        ));
    }
    let face = person_byte(&game, n, 0x0e);
    commands.spawn((
        Sprite {
            image: asset_server.load(format!("ALIEN/ALIEN{face}.PIC")),
            rect: Some(Rect::new(1.0, 1.0, 96.0, 144.0)),
            ..default()
        },
        Anchor::TOP_LEFT,
        place(Vec2::new(224.0, 50.0), 0.5),
        scoped,
    ));
}

/// FUN_1fd2_0b30: the races the Stranger can spy on (races you've met):
/// the lines of KERDES2.LOC naming them.
fn stranger_targets(game: &Game, league: bool) -> Vec<u8> {
    let met = |r: u8| matches!(game.0.standing(r), 2 | 4);
    let b = |at: u16| game.0.byte(at).unwrap_or(0) as i8;
    if league {
        return (7..=10u8).filter(|&r| met(r)).map(|r| r + 8).collect();
    }
    let mut lines = Vec::new();
    if met(3) && b(0x4818) >= 1 {
        lines.push(10);
    }
    if met(4) && b(0x6ccd) >= 0 {
        lines.push(11);
    }
    if met(5) && b(0x6ccd) >= 0 {
        lines.push(12);
    }
    if met(6) && b(0x5d90) == 0 {
        lines.push(13);
    }
    if (7..=10).any(met) {
        lines.push(14);
    }
    if met(12) {
        lines.push(26);
    }
    lines
}

/// The lines you may say (DS:0x3d6 + 4 * person: 6-byte entries, a count
/// and up to five line numbers).
fn choices(data: &GameData, n: u8, state: u8) -> Vec<u8> {
    let Some(pointer) = data.exe.ds_bytes(0x3d6 + 4 * u16::from(n), 2) else { return Vec::new() };
    let at = u16::from_le_bytes([pointer[0], pointer[1]]);
    let Some(entry) = data.exe.ds_bytes(at + 6 * u16::from(state), 6) else { return Vec::new() };
    entry[1..=usize::from(entry[0]).min(5)].to_vec()
}

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
    game: Option<Res<Game>>,
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
    let scoped = (TextPart, DespawnOnExit(GameScreen::PubTalk));
    let questions = data.pub_talks.get(usize::from(talk.person)).map(|t| t.0.clone()).unwrap_or_default();
    let lines = match (talk.person, talk.state, game.as_ref()) {
        (2, 6, Some(game)) => stranger_targets(game, false),
        (2, 7, Some(game)) => stranger_targets(game, true),
        _ => choices(data, talk.person, talk.state),
    };
    let text = |commands: &mut Commands, s: &str, colors, row: usize| {
        let at = Vec2::new(6.0, TEXT_TOP + 9.0 * row as f32);
        commands.spawn((label(Label::new(s, COLUMNS, colors), at), scoped.clone()));
    };
    let mut row = 0;
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
            for (k, q) in lines.into_iter().enumerate() {
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

fn add_money(game: &mut Game, change: i64) {
    let money = (i64::from(game.0.money()) + change).clamp(0, i64::from(u32::MAX)) as u32;
    game.0.set_word(0x95be, money as u16);
    game.0.set_word(0x95c0, (money >> 16) as u16);
}

/// What saying line `q` to person `n` does; the reply line to show
/// instead, if any (the main loop, screen 25).
fn consequences(game: &mut Game, n: u8, q: u8) -> Option<u8> {
    let set = |game: &mut Game, at: u16, v: u8| game.0.set_byte(at, v);
    let b = |game: &Game, at: u16| game.0.byte(at).unwrap_or(0);
    let mut reply = None;
    // FUN_1fd2_0b30 keeps this up to date: can the Stranger spy for you?
    let any = !stranger_targets(game, false).is_empty();
    set(game, 0x774a, u8::from(any));
    match (n, q) {
        (1, 1) => set(game, 0x773e, 1),
        (1, 2) => reply = Some(if b(game, 0x773f) == 0 { 4 } else { 6 }),
        (1, 4) => {
            set(game, 0x164, 1);
            game.0.set_word(0x5d8a, 200);
        }
        (2, 8) => {
            set(game, 0x773f, 1);
            set(game, 0x4818, 0);
        }
        (2, 5) => add_money(game, -30_000),
        (2, 6) => add_money(game, -40_000),
        (2, 7) => add_money(game, -50_000),
        (3, _) => set(game, 0x7749, 1),
        (6, 8) | (6, 10) => set(game, 0x7740, 1),
        (6, 11) => set(game, 0x7747, 1),
        (6, 13) => {
            set(game, 0x7742, 1);
            add_money(game, -5000);
        }
        (6, 6 | 15) => {
            reply = Some(3);
            if b(game, 0x6db1) == 0 || b(game, 0x6e95) == 0 {
                reply = Some(8);
            }
            if b(game, 0x6be9) == 0 {
                reply = Some(7);
            }
        }
        (5, 5 | 6) => {
            set(game, 0x7743, 1);
            game.0.send_person(5, random(100) + 100, q - 4);
        }
        (7, 4 | 6) => {
            add_money(game, -100_000);
            game.0.send_person(7, random(100) + 100, 2);
        }
        (7, 5) => set(game, 0x7745, 1),
        (10, 3) => add_money(game, -100_000),
        (10, 4) => add_money(game, -150_000),
        (10, 5) => add_money(game, -200_000),
        (10, 8) => set(game, 0x23c, 1),
        (10, 9 | 10) => {
            set(game, 0x7744, 1);
            set(game, 0x5d99, 1);
            set(game, 0x23c, 1);
        }
        _ => {}
    }
    // The informer's news: what's going on (0x75ee), for a price.
    if n == 4 {
        let mut news = 0;
        if (b(game, 0x6ccd) as i8) > 0 {
            news = 1;
        }
        if b(game, 0x5d5e) != 0 && b(game, 0x5d6c) == 0 {
            news = 2;
        }
        if b(game, 0x4817) == 1 && b(game, 0x6e95) == 0 {
            news = 3;
        }
        if b(game, 0x4819) == 1 && b(game, 0x5d91) == 0 {
            news = 4;
        }
        match q {
            1 => reply = Some(if news != 0 { 3 } else { 2 }),
            3 => {
                add_money(game, -500);
                reply = Some(news + 4);
                if news == 3 {
                    set(game, 0x47e2, 0);
                }
            }
            5 => {
                set(game, 0x7741, 1);
                add_money(game, -500);
            }
            _ => {}
        }
    }
    if n == 7 {
        set(game, 0x7748, 1);
    }
    reply
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
    let (questions, answers) = data.pub_talks.get(usize::from(talk.person)).cloned().unwrap_or_default();
    if q == 0 {
        if talk.state == 0 {
            commands.trigger(GoTo(GameScreen::SpaceLocal));
        } else {
            talk.reply = None;
        }
        return;
    }
    let (mut reply_line, _) = split(&line(&questions, q));
    if let Some(r) = consequences(&mut game, talk.person, q) {
        reply_line = r;
    }
    // The Stranger: which race, what it costs, and the deal.
    if talk.person == 2 {
        let target = match q {
            10..=13 => q - 7,
            15..=18 => q - 8,
            26 => 12,
            _ => 0,
        };
        if target != 0 {
            talk.target = target;
        }
        if q == 23 && talk.price > 0 {
            add_money(&mut game, -i64::from(talk.price));
            game.0.send_person(2, random(100) + 100, 0);
        }
    }
    let (next, mut reply) = split(&line(&answers, reply_line));
    if talk.person == 2 && (10..=13).contains(&reply_line) {
        let price = data.exe.stranger_price(talk.target, reply_line);
        talk.price = price;
        game.0.set_byte(reunion_formats::pub_people::STRANGER_MISSION, (reply_line - 9) * 20 + talk.target);
        reply = vec![if random(2) == 0 {
            format!("It will be {price} credits")
        } else {
            format!("My work will cost {price} credits")
        }];
        talk.state = 9;
        talk.reply = Some(reply);
        return;
    }
    talk.state = next;
    talk.reply = Some(reply);
}

/// END TALK: back to the pub.
fn end_talk(action: On<ActionUsed>, screen: Res<State<GameScreen>>, mut commands: Commands) {
    if action.0 == END_TALK && *screen.get() == GameScreen::PubTalk {
        commands.trigger(GoTo(GameScreen::SpaceLocal));
    }
}
