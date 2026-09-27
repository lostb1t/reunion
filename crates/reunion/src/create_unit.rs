//! NEW UNIT's form (screen 21): the new group's name and type.
//!
//! From REUNION.PRG FUN_2b8d_0773, FUN_2b8d_15ec and FUN_2b8d_1659: the
//! screen table's background (GRAFIKA/BEOSZT) with the name in red at
//! (135, 94) and the type at (135, 119). "Change name" (132, 91, 93x13) types
//! a new name (FUN_398f_0059, up to 14 characters); "Change type" (132, 117,
//! 93x11) steps through the types researched so far (DS:0xa2d5 + type).
//! OK,CREATE IT goes on to GROUP; ABORT drops the group again.

use bevy::prelude::*;
use reunion_formats::state::{UnitList, unit};

use crate::focus::{Activated, DefaultFocus, Hover, hotspot};
use crate::game::Game;
use crate::game_data::{GameData, GameDataHandle};
use crate::hud::{ActionUsed, CONTENT_Y, HoverLabel, RED_TEXT};
use crate::screen::{GameScreen, picture};
use crate::ship_info::{LIST_COUNT, SELECTED, unit_name};
use crate::text::{Label, TextEditing, label};
use crate::transition::GoTo;

pub struct CreateUnitPlugin;

impl Plugin for CreateUnitPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(OnEnter(GameScreen::CreateUnit), enter)
            .add_systems(
                Update,
                (finish_typing, update_texts)
                    .chain()
                    .run_if(in_state(GameScreen::CreateUnit)),
            )
            .add_systems(OnExit(GameScreen::CreateUnit), |mut commands: Commands| {
                commands.remove_resource::<TextEditing>();
            })
            .add_observer(activate)
            .add_observer(decide);
    }
}

const CREATE: u8 = 60;
const ABORT: u8 = 58;
/// Unit types researched so far: DS:0xa2d5 + type.
const TYPE_AVAILABLE: u16 = 0xa2d5;
const NAME_COLUMNS: usize = 14;

#[derive(Component)]
struct NameField;

#[derive(Component)]
struct TypeField;

#[derive(Component)]
struct NameText;

#[derive(Component)]
struct TypeText;

fn enter(mut commands: Commands, asset_server: Res<AssetServer>, game: Option<ResMut<Game>>) {
    let scoped = DespawnOnExit(GameScreen::CreateUnit);
    commands.spawn((
        picture(asset_server.load("GRAFIKA/BEOSZT.PIC"), Vec2::new(0.0, CONTENT_Y)),
        scoped.clone(),
    ));
    commands.spawn((
        NameField,
        DefaultFocus,
        HoverLabel("Change name".into()),
        hotspot(Rect::new(132.0, 91.0, 225.0, 104.0), Hover::Outline),
        scoped.clone(),
    ));
    commands.spawn((
        TypeField,
        HoverLabel("Change type".into()),
        hotspot(Rect::new(132.0, 117.0, 225.0, 128.0), Hover::Outline),
        scoped.clone(),
    ));
    commands.spawn((
        NameText,
        label(Label::new("", NAME_COLUMNS, RED_TEXT), Vec2::new(135.0, 94.0)),
        scoped.clone(),
    ));
    commands.spawn((
        TypeText,
        label(Label::new("", 13, RED_TEXT), Vec2::new(135.0, 119.0)),
        scoped,
    ));
    // The type starts at the first one researched.
    if let Some(mut game) = game {
        next_type(&mut game, false);
    }
}

fn selected(game: &Game) -> usize {
    game.0.word(SELECTED).unwrap_or(0) as usize
}

/// Steps the new group's type to the next researched one (or keeps it, if
/// it is researched and `step` is false).
fn next_type(game: &mut Game, step: bool) {
    let k = selected(game);
    let available = |game: &Game, t: u8| game.0.byte(TYPE_AVAILABLE + u16::from(t)).unwrap_or(0) != 0;
    let Some(current) = game.0.unit(UnitList::Groups, k).map(|r| r[unit::TYPE]) else {
        return;
    };
    let mut t = if step { current % 4 + 1 } else { current };
    for _ in 0..4 {
        if available(game, t) {
            break;
        }
        t = t % 4 + 1;
    }
    if let Some(record) = game.0.unit_mut(UnitList::Groups, k) {
        record[unit::TYPE] = t;
    }
}

fn activate(
    activated: On<Activated>,
    name: Query<(), With<NameField>>,
    kind: Query<(), With<TypeField>>,
    game: Option<ResMut<Game>>,
    mut commands: Commands,
) {
    let Some(mut game) = game else { return };
    if name.contains(activated.0) {
        let current = game
            .0
            .unit(UnitList::Groups, selected(&game))
            .map(unit_name)
            .unwrap_or_default();
        commands.insert_resource(TextEditing::new(current, NAME_COLUMNS));
    } else if kind.contains(activated.0) {
        next_type(&mut game, true);
    }
}

fn finish_typing(
    editing: Option<Res<TextEditing>>,
    game: Option<ResMut<Game>>,
    mut commands: Commands,
) {
    let (Some(editing), Some(mut game)) = (editing, game) else {
        return;
    };
    if editing.done {
        let k = selected(&game);
        game.0.rename_unit(UnitList::Groups, k, editing.text.trim_end());
        commands.remove_resource::<TextEditing>();
    }
}

fn update_texts(
    game: Option<Res<Game>>,
    editing: Option<Res<TextEditing>>,
    handle: Res<GameDataHandle>,
    data: Res<Assets<GameData>>,
    mut name: Query<&mut Label, (With<NameText>, Without<TypeText>)>,
    mut kind: Query<&mut Label, (With<TypeText>, Without<NameText>)>,
) {
    let (Some(game), Some(data)) = (game, data.get(&handle.0)) else {
        return;
    };
    let Some(record) = game.0.unit(UnitList::Groups, selected(&game)) else {
        return;
    };
    let shown = match &editing {
        // A cursor while typing.
        Some(e) if !e.done => format!("{}_", e.text),
        _ => unit_name(record),
    };
    if let Ok(label) = name.single_mut() {
        Label::set(label, shown);
    }
    // Type names at DS:0x591a + 14 * type.
    let type_name = data
        .exe
        .ds_string(0x591a + 14 * u16::from(record[unit::TYPE]))
        .unwrap_or_default();
    if let Ok(label) = kind.single_mut() {
        Label::set(label, type_name);
    }
}

/// OK,CREATE IT and ABORT.
fn decide(
    action: On<ActionUsed>,
    screen: Res<State<GameScreen>>,
    game: Option<ResMut<Game>>,
    mut commands: Commands,
) {
    if *screen.get() != GameScreen::CreateUnit {
        return;
    }
    let Some(mut game) = game else { return };
    match action.0 {
        CREATE => commands.trigger(GoTo(GameScreen::Group)),
        ABORT => {
            game.0.remove_last_group();
            let count = game.0.unit_count(UnitList::Groups) as u16;
            game.0.set_word(LIST_COUNT, count);
            game.0.set_word(SELECTED, 0);
            game.0.set_word(0xa2ca, 0);
            commands.trigger(GoTo(GameScreen::ShipInfo));
        }
        _ => {}
    }
}
