//! SETTINGS: not in the original. An icon on the main screen's bar (our
//! own art, `art://icons/SETTINGS.png`) opens a page of options: widescreen,
//! the upscaling filter, the music, speech, and the developer cheats found
//! in REUNION.PRG (see GAMEMECHANICS.md) that nothing in the shipped game
//! turns on.
//!
//! The options are kept in `SAVE/SETTINGS.TXT` next to the saved games.

use std::path::PathBuf;

use bevy::prelude::*;

use crate::audio::{GameMusic, Sfx, SoundSettings};
use crate::focus::{Activated, DefaultFocus, Hover, hotspot};
use crate::game::Game;
use crate::game_data::{GameData, GameDataHandle};
use crate::hud::{CONTENT_Y, HoverLabel, RED_TEXT, YELLOW_TEXT};
use crate::screen::{GameScreen, Widescreen};
use crate::text::{Label, label};
use crate::upscale::UpscaleMode;

pub struct SettingsPlugin;

impl Plugin for SettingsPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Cheats>()
            .add_systems(Startup, load)
            .add_systems(Update, (add_action, save, apply_cheats))
            .add_systems(OnEnter(GameScreen::Settings), enter)
            .add_systems(Update, update_rows.run_if(in_state(GameScreen::Settings)))
            .add_observer(change);
    }
}

/// The Settings action, after the original's 77.
pub const SETTINGS_ACTION: u8 = 78;
/// Its screen, after the original's 38.
pub const SETTINGS_SCREEN: u8 = 39;
const BACK_TO_MAIN: u8 = 24;
const MAIN_SCREEN: usize = 1;

/// The developer cheats (DS:0x91e7, 0x91e8).
#[derive(Resource, Default, Clone, Copy, PartialEq)]
pub struct Cheats {
    /// Money and research: turning it on finishes the research in progress
    /// and brings the money to 1,000,000; while on, +10,000 credits an hour.
    pub money: bool,
    /// Every battle is won: space battles' hits destroy, ground wars end
    /// at once.
    pub battles: bool,
}

/// Puts SETTINGS on the main screen's bar once the game data is in.
fn add_action(
    handle: Res<GameDataHandle>,
    mut data: ResMut<Assets<GameData>>,
    asset_server: Res<AssetServer>,
) {
    let action = usize::from(SETTINGS_ACTION);
    // Only once, and not before the data is in (get_mut would flag it changed).
    if data.get(&handle.0).is_none_or(|d| d.labels.len() > action) {
        return;
    }
    let Some(mut data) = data.get_mut(&handle.0) else { return };
    data.labels.resize(action + 1, String::new());
    data.labels[action] = "SETTINGS".into();
    data.action_icons.resize(action + 1, None);
    data.action_icons[action] = Some(asset_server.load("art://icons/SETTINGS.png"));
    data.action_voices.resize(action + 1, String::new());
    data.action_voices[action] = "x".into();
    let screen = usize::from(SETTINGS_SCREEN);
    data.icon_sets.resize(screen + 1, Vec::new());
    data.icon_sets[screen] = vec![BACK_TO_MAIN];
    data.icon_sets[MAIN_SCREEN].push(SETTINGS_ACTION);
    data.screen_triggers.resize(screen + 1, None);
    data.screen_triggers[screen] = Some(SETTINGS_ACTION);
}

/// One line of the page.
#[derive(Component, Clone, Copy, PartialEq, Debug)]
enum Row {
    Widescreen,
    Upscaling,
    Music,
    Speech,
    CheatMoney,
    CheatBattles,
}

const ROWS: [Row; 6] = [Row::Widescreen, Row::Upscaling, Row::Music, Row::Speech, Row::CheatMoney, Row::CheatBattles];
const ROW_TOP: f32 = 78.0;
const ROW_HEIGHT: f32 = 16.0;
const LEFT: f32 = 48.0;
const COLUMNS: usize = 36;

#[derive(Component)]
struct RowText(Row);

fn enter(mut commands: Commands) {
    let scoped = DespawnOnExit(GameScreen::Settings);
    commands.spawn((
        label(Label::new("SETTINGS", 8, RED_TEXT), Vec2::new(LEFT, CONTENT_Y + 10.0)),
        scoped.clone(),
    ));
    for (k, row) in ROWS.into_iter().enumerate() {
        let top = ROW_TOP + ROW_HEIGHT * k as f32;
        let mut e = commands.spawn((
            row,
            HoverLabel(row.name().into()),
            hotspot(Rect::new(LEFT - 4.0, top - 3.0, LEFT + 6.0 * COLUMNS as f32 + 4.0, top + 11.0), Hover::Outline),
            scoped.clone(),
        ));
        if k == 0 {
            e.insert(DefaultFocus);
        }
        commands.spawn((
            RowText(row),
            label(Label::new("", COLUMNS, YELLOW_TEXT), Vec2::new(LEFT, top)),
            scoped.clone(),
        ));
    }
    commands.spawn((
        label(
            Label::new("The cheats are the original developers'.", COLUMNS + 6, RED_TEXT),
            Vec2::new(LEFT, ROW_TOP + ROW_HEIGHT * ROWS.len() as f32 + 10.0),
        ),
        scoped,
    ));
}

impl Row {
    fn name(self) -> &'static str {
        match self {
            Row::Widescreen => "Widescreen (experimental)",
            Row::Upscaling => "Upscaling",
            Row::Music => "Music",
            Row::Speech => "Speech",
            Row::CheatMoney => "Cheat: money and research",
            Row::CheatBattles => "Cheat: win every battle",
        }
    }
}

fn on_off(on: bool) -> &'static str {
    if on { "ON" } else { "OFF" }
}

fn update_rows(
    widescreen: Res<Widescreen>,
    upscale: Res<UpscaleMode>,
    music: Res<GameMusic>,
    sound: Res<SoundSettings>,
    cheats: Res<Cheats>,
    mut texts: Query<(&RowText, &mut Label)>,
) {
    for (RowText(row), label) in &mut texts {
        let value = match row {
            Row::Widescreen => on_off(widescreen.0).to_string(),
            Row::Upscaling => upscale.name().to_uppercase(),
            Row::Music => ["OFF", "1", "2"][usize::from(music.background.min(2))].to_string(),
            Row::Speech => on_off(sound.speech).to_string(),
            Row::CheatMoney => on_off(cheats.money).to_string(),
            Row::CheatBattles => on_off(cheats.battles).to_string(),
        };
        let name = row.name();
        let dots = COLUMNS.saturating_sub(name.len() + value.len() + 2);
        Label::set(label, format!("{name} {} {value}", ".".repeat(dots)));
    }
}

fn change(
    activated: On<Activated>,
    rows: Query<&Row>,
    mut widescreen: ResMut<Widescreen>,
    mut upscale: ResMut<UpscaleMode>,
    mut music: ResMut<GameMusic>,
    mut sound: ResMut<SoundSettings>,
    mut cheats: ResMut<Cheats>,
    game: Option<ResMut<Game>>,
    mut commands: Commands,
) {
    let Ok(&row) = rows.get(activated.0) else { return };
    commands.trigger(Sfx::named("x"));
    match row {
        Row::Widescreen => widescreen.0 = !widescreen.0,
        Row::Upscaling => {
            *upscale = if *upscale == UpscaleMode::Off { UpscaleMode::Crt } else { UpscaleMode::Off };
        }
        Row::Music => music.background = (music.background + 2) % 3,
        Row::Speech => sound.speech = !sound.speech,
        Row::CheatBattles => cheats.battles = !cheats.battles,
        Row::CheatMoney => {
            cheats.money = !cheats.money;
            if cheats.money
                && let Some(mut game) = game
            {
                boost(&mut game);
            }
        }
    }
}

/// FUN_26eb_0000's first round: the money up to 1,000,000 and the research
/// in progress finished.
fn boost(game: &mut Game) {
    let money = game.0.money().max(1_000_000);
    game.0.set_word(0x95be, money as u16);
    game.0.set_word(0x95c0, (money >> 16) as u16);
    for n in 1..=35u16 {
        let at = 0x5d77 + 0x35 * n + 0x11;
        if matches!(game.0.word(at), Some(1..=4)) {
            game.0.set_word(at, 5);
        }
    }
}

/// The cheats in the game: the battle flag the battles read, and +10,000
/// credits an hour for the money cheat.
fn apply_cheats(cheats: Res<Cheats>, game: Option<ResMut<Game>>, mut last: Local<Option<[u16; 4]>>) {
    let Some(mut game) = game else {
        *last = None;
        return;
    };
    let battles = u8::from(cheats.battles);
    if game.0.byte(0x91e8) != Some(battles) {
        game.0.set_byte(0x91e8, battles);
    }
    let now = game.0.date();
    if cheats.money && last.is_some_and(|l| l != now) {
        let money = game.0.money().saturating_add(10_000);
        game.0.set_word(0x95be, money as u16);
        game.0.set_word(0x95c0, (money >> 16) as u16);
    }
    *last = Some(now);
}

fn settings_path() -> PathBuf {
    crate::disk::game_folder().join("SAVE").join("SETTINGS.TXT")
}

/// Reads `SAVE/SETTINGS.TXT` (`key=value` lines), if there is one.
fn load(
    mut widescreen: ResMut<Widescreen>,
    mut upscale: ResMut<UpscaleMode>,
    mut music: ResMut<GameMusic>,
    mut sound: ResMut<SoundSettings>,
    mut cheats: ResMut<Cheats>,
) {
    let Ok(text) = std::fs::read_to_string(settings_path()) else { return };
    for line in text.lines() {
        let Some((key, value)) = line.split_once('=') else { continue };
        let on = value.trim() == "on";
        match key.trim() {
            "widescreen" => widescreen.0 = on,
            "upscaling" => *upscale = if value.trim() == "crt" { UpscaleMode::Crt } else { UpscaleMode::Off },
            "music" => music.background = value.trim().parse().unwrap_or(1).min(2),
            "speech" => sound.speech = on,
            "cheat_money" => cheats.money = on,
            "cheat_battles" => cheats.battles = on,
            _ => {}
        }
    }
}

/// Writes the settings whenever one changes.
fn save(
    widescreen: Res<Widescreen>,
    upscale: Res<UpscaleMode>,
    music: Res<GameMusic>,
    sound: Res<SoundSettings>,
    cheats: Res<Cheats>,
    mut saved: Local<Option<String>>,
) {
    let on = |b: bool| if b { "on" } else { "off" };
    let text = format!(
        "widescreen={}\nupscaling={}\nmusic={}\nspeech={}\ncheat_money={}\ncheat_battles={}\n",
        on(widescreen.0),
        if *upscale == UpscaleMode::Crt { "crt" } else { "off" },
        music.background,
        on(sound.speech),
        on(cheats.money),
        on(cheats.battles),
    );
    // The first look is what was loaded: nothing to write yet.
    if saved.as_ref() == Some(&text) {
        return;
    }
    if saved.is_some()
        && let Err(error) = std::fs::write(settings_path(), &text)
    {
        warn!("can't save the settings: {error}");
    }
    *saved = Some(text);
}
