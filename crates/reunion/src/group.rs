//! GROUP (screen 22): what a unit carries, and loading it from stock.
//!
//! From REUNION.PRG FUN_2b8d_0002 (logic), FUN_2b8d_092b (drawing),
//! FUN_2b8d_11bf (hotspots and the stock source) and FUN_28cd_03fb (icon bar).
//! GRAFIKA/BEOSZTAS at row 49 shows one category of the unit's kinds
//! (warships, merchant ships, troops or satellites; the arrows on the right
//! page through the categories the unit's type carries): a row per kind with
//! its number and how many of each equipment its ships carry, and the stock
//! at the bottom. Stock is New Earth's (the inventions' stock) for units
//! there, otherwise the base's where the unit is.
//!
//! Only a unit docked at (or a base on) your own colony can be loaded:
//! confirming a number takes one more from stock, the other button (see
//! `Alternate`) puts one back, along with the equipment it can no longer
//! carry. The arrows at the top step through all units.

use bevy::prelude::*;
use bevy::sprite::Anchor;
use bevy::state::state::StateTransitionEvent;
use reunion_formats::exe::UnitCategory;
use reunion_formats::state::{UnitList, unit};

use crate::audio::Sfx;
use crate::focus::{Activated, AlternateUse, DefaultFocus, Focus, Hover, hotspot};
use crate::game::Game;
use crate::game_data::{GameData, GameDataHandle};
use crate::hud::{ActionUsed, CONTENT_Y, ExtraActions, HoverLabel, RED_TEXT, YELLOW_TEXT};
use crate::screen::{GameScreen, picture, place};
use crate::ship_info::{LIST, LIST_COUNT, SELECTED, unit_name};
use crate::text::{Label, TextEditing, label};
use crate::transition::GoTo;

pub struct GroupPlugin;

impl Plugin for GroupPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(OnEnter(GameScreen::Group), enter.after(crate::hud::spawn_hud))
            .add_systems(
                Update,
                (finish_typing, spawn_view, update_actions)
                    .chain()
                    .run_if(in_state(GameScreen::Group)),
            )
            .add_systems(OnExit(GameScreen::Group), |mut commands: Commands| {
                commands.remove_resource::<View>();
                commands.remove_resource::<TextEditing>();
            })
            .add_observer(activate)
            .add_observer(take_back)
            .add_observer(use_action);
    }
}

const CONTROL_PANEL: u8 = 16;
const TRANSFER: u8 = 3;
const GALACTIC_MAP: u8 = 46;
const PLANET_MAIN: u8 = 41;
const DISBAND: u8 = 37;
const SELECTED_PER_LIST: [u16; 2] = [0xa2ca, 0xa2cc];
/// A unit may hold up to 1000 of a kind.
const MAX_OF_A_KIND: u16 = 1000;
const NAME_COLUMNS: usize = 17;

/// The unit and category shown; `from_planet` when opened from PLANET MAIN
/// (the planet's base, no stepping to other units).
#[derive(Resource, Clone, Copy, PartialEq)]
struct View {
    list: UnitList,
    unit: usize,
    category: usize,
    from_planet: bool,
}

#[derive(Component, Clone)]
struct ViewPart;

#[derive(Component, Clone, Copy, PartialEq)]
enum Button {
    Name,
    Previous,
    Next,
    PageUp,
    PageDown,
    /// Number of kind i (0-based) of the category.
    Count(usize),
    /// Equipment j carried by kind i.
    Fitted(usize, usize),
}

fn list_of(game: &Game) -> UnitList {
    if game.0.word(LIST) == Some(1) {
        UnitList::Bases
    } else {
        UnitList::Groups
    }
}

fn enter(
    mut commands: Commands,
    mut transitions: MessageReader<StateTransitionEvent<GameScreen>>,
    game: Option<ResMut<Game>>,
    handle: Res<GameDataHandle>,
    data: Res<Assets<GameData>>,
) {
    let (Some(mut game), Some(data)) = (game, data.get(&handle.0)) else {
        return;
    };
    let from = transitions.read().last().and_then(|t| t.exited);
    let from_planet = from == Some(GameScreen::PlanetMain);
    let (list, n) = if from_planet {
        // FUN_2b8d_0002: the base where the planet screen is.
        let (s, p, m) = game.selection();
        let n = game.0.base_at(s as u8, p as u8, m as u8).unwrap_or(0);
        game.0.set_word(LIST, 1);
        let count = game.0.unit_count(UnitList::Bases) as u16;
        game.0.set_word(LIST_COUNT, count);
        (UnitList::Bases, n)
    } else {
        (list_of(&game), game.0.word(SELECTED).unwrap_or(0) as usize)
    };
    select(&mut game, list, n);
    let category = first_category(data, &game, list, n).unwrap_or(1);
    commands.insert_resource(View {
        list,
        unit: n,
        category,
        from_planet,
    });
}

fn categories<'a>(data: &'a GameData, record: &[u8]) -> &'a [UnitCategory] {
    data.unit_categories
        .get(record[unit::TYPE] as usize)
        .map_or(&[], Vec::as_slice)
}

/// FUN_2b8d_1a62: the first category the unit's type carries.
fn first_category(data: &GameData, game: &Game, list: UnitList, n: usize) -> Option<usize> {
    let record = game.0.unit(list, n)?;
    categories(data, record).first().map(|c| c.number)
}

/// Makes unit `n` the selected one; the planet screens then look at where it is.
fn select(game: &mut Game, list: UnitList, n: usize) {
    game.0.set_word(SELECTED, n as u16);
    game.0.set_word(SELECTED_PER_LIST[list as usize], n as u16);
    if let Some(record) = game.0.unit(list, n) {
        let (s, p, m) = (record[unit::SYSTEM], record[unit::PLANET], record[unit::MOON]);
        game.select(s.into(), p.into(), m.into());
    }
}

/// An invention's word field (1-based invention; records at DS:0x5d77 + 0x35 * n).
fn invention_word(game: &Game, invention: u8, field: u16) -> i16 {
    game.0
        .word(0x5d77 + 0x35 * u16::from(invention) + field)
        .unwrap_or(0) as i16
}

/// Status (0x11): anything above 0 makes the kind show up here.
fn known(game: &Game, invention: u8) -> bool {
    invention_word(game, invention, 0x11) > 0
}

/// Where the unit's stock comes from (FUN_2b8d_11bf).
#[derive(Clone, Copy)]
enum Stock {
    /// New Earth: the inventions' stock (word 0x1b).
    NewEarth,
    /// The base at the unit's planet: words at base + 0x83 + 2 * index.
    Base(usize),
    None,
}

impl Stock {
    fn of(game: &Game, record: &[u8]) -> Self {
        let (s, p, m) = (record[unit::SYSTEM], record[unit::PLANET], record[unit::MOON]);
        if (s, p, m) == (1, 5, 0) {
            Stock::NewEarth
        } else {
            game.0.base_at(s, p, m).map_or(Stock::None, Stock::Base)
        }
    }

    /// Stock of something made by `invention`, kept at a base under `index`.
    fn get(self, game: &Game, invention: u8, index: u8) -> Option<i16> {
        match self {
            Stock::NewEarth => Some(invention_word(game, invention, 0x1b)),
            Stock::Base(b) if index > 0 => game
                .0
                .unit(UnitList::Bases, b)
                .map(|r| word(r, 0x83 + 2 * index as usize) as i16),
            _ => None,
        }
    }

    fn add(self, game: &mut Game, invention: u8, index: u8, delta: i16) {
        match self {
            Stock::NewEarth => {
                let at = 0x5d77 + 0x35 * u16::from(invention) + 0x1b;
                let v = game.0.word(at).unwrap_or(0) as i16;
                game.0.set_word(at, v.wrapping_add(delta) as u16);
            }
            Stock::Base(b) if index > 0 => {
                if let Some(r) = game.0.unit_mut(UnitList::Bases, b) {
                    let at = 0x83 + 2 * index as usize;
                    let v = word(r, at) as i16;
                    r[at..at + 2].copy_from_slice(&v.wrapping_add(delta).to_le_bytes());
                }
            }
            _ => {}
        }
    }
}

fn word(record: &[u8], at: usize) -> u16 {
    u16::from_le_bytes([record[at], record[at + 1]])
}

/// Only a unit docked at (or a base on) your own colony can be loaded:
/// the planet's owner is you and it has a colony (FUN_357b_3253), and the
/// unit's status is 1 or 7.
fn editable(game: &Game, data: &GameData, record: &[u8]) -> bool {
    let (s, p, m) = (record[unit::SYSTEM], record[unit::PLANET], record[unit::MOON]);
    let body = if m == 0 {
        Some(p as usize)
    } else {
        data.star_systems
            .get((s as usize).wrapping_sub(1))
            .and_then(|l| l.moons.get((p as usize).wrapping_sub(1)))
            .and_then(|moons| moons.get(m as usize - 1))
            .map(|&b| b as usize)
    };
    let colony = body
        .and_then(|b| game.0.body(s as usize, b))
        .is_some_and(|b| b[0] == 1 && b[6] != 0);
    colony && matches!(record[unit::STATUS], 1 | 7)
}

/// Offset of kind `i` (0-based) of a category in a unit record.
fn kind_at(category: &UnitCategory, i: usize) -> usize {
    unit::SLOTS + (category.slot - 1) * unit::SLOT_LEN + i * unit::KIND_LEN
}

fn row_y(i: usize) -> f32 {
    109.0 + 10.0 * i as f32
}

fn column_x(j: usize) -> f32 {
    146.0 + 38.0 * j as f32
}

/// Draws the whole screen for the current view (on entering and on changes).
fn spawn_view(
    mut commands: Commands,
    view: Option<Res<View>>,
    game: Option<Res<Game>>,
    handle: Res<GameDataHandle>,
    data: Res<Assets<GameData>>,
    asset_server: Res<AssetServer>,
    parts: Query<Entity, With<ViewPart>>,
    editing: Option<Res<TextEditing>>,
    focus: Res<Focus>,
    buttons: Query<&Button>,
    mut shown: Local<Vec<u8>>,
) {
    let (Some(view), Some(game), Some(data)) = (view, game, data.get(&handle.0)) else {
        return;
    };
    // Redraw only when something shown changes (not every game hour), and
    // keep the focus on the same button.
    let key = shown_state(&game, &view, editing.as_deref());
    if !view.is_changed() && *shown == key {
        return;
    }
    *shown = key;
    let focused = focus.0.and_then(|e| buttons.get(e).ok()).copied();
    for part in &parts {
        commands.entity(part).despawn();
    }
    let scoped = (ViewPart, DespawnOnExit(GameScreen::Group));
    commands.spawn((
        picture(asset_server.load("GRAFIKA/BEOSZTAS.PIC"), Vec2::new(0.0, CONTENT_Y)),
        scoped.clone(),
    ));
    let Some(record) = game.0.unit(view.list, view.unit) else {
        return;
    };
    let text = |commands: &mut Commands, s: String, columns: usize, colors, pos: Vec2| {
        commands.spawn((label(Label::new(s, columns, colors), pos), scoped.clone()));
    };
    let black = |commands: &mut Commands, rect: Rect| {
        commands.spawn((
            Sprite::from_color(Color::BLACK, rect.size()),
            Anchor::TOP_LEFT,
            place(rect.min, 0.5),
            scoped.clone(),
        ));
    };
    let button = |commands: &mut Commands, b: Button, rect: Rect, name: String| {
        let mut entity =
            commands.spawn((b, HoverLabel(name), hotspot(rect, Hover::Outline), scoped.clone()));
        if focused == Some(b) {
            entity.insert(DefaultFocus);
        }
    };

    // Name, type and where the unit is.
    let name = match &editing {
        Some(e) if !e.done => format!("{}_", e.text),
        _ => unit_name(record),
    };
    text(&mut commands, name, NAME_COLUMNS, RED_TEXT, Vec2::new(24.0, 56.0));
    let type_name = data
        .exe
        .ds_string(0x591a + 14 * u16::from(record[unit::TYPE]))
        .unwrap_or_default();
    text(&mut commands, type_name, 13, YELLOW_TEXT, Vec2::new(223.0, 56.0));
    text(&mut commands, location(&game, data, record), 49, YELLOW_TEXT, Vec2::new(12.0, 67.0));
    button(
        &mut commands,
        Button::Name,
        Rect::new(21.0, 53.0, 129.0, 65.0),
        "Change name".into(),
    );

    // Stepping through units (not from the planet screen).
    let groups = game.0.unit_count(UnitList::Groups);
    let bases = game.0.unit_count(UnitList::Bases);
    let first = view.from_planet
        || ((view.list == UnitList::Groups || groups == 0) && view.unit == 1);
    let last = view.from_planet || (view.list == UnitList::Bases && view.unit == bases);
    if first {
        black(&mut commands, Rect::new(9.0, 53.0, 21.0, 63.0));
    } else {
        button(
            &mut commands,
            Button::Previous,
            Rect::new(7.0, 53.0, 21.0, 65.0),
            "Previous group".into(),
        );
    }
    if last {
        black(&mut commands, Rect::new(131.0, 53.0, 143.0, 63.0));
    } else {
        button(
            &mut commands,
            Button::Next,
            Rect::new(129.0, 53.0, 142.0, 65.0),
            "Next group".into(),
        );
    }

    let cats = categories(data, record);
    let Some(category) = cats.iter().find(|c| c.number == view.category) else {
        return;
    };
    // Paging through the type's categories.
    if cats.first().map(|c| c.number) == Some(view.category) {
        black(&mut commands, Rect::new(294.0, 87.0, 322.0, 104.0));
    } else {
        button(&mut commands, Button::PageUp, Rect::new(291.0, 84.0, 319.0, 119.0), "Page up".into());
    }
    if cats.last().map(|c| c.number) == Some(view.category) {
        black(&mut commands, Rect::new(294.0, 124.0, 322.0, 141.0));
    } else {
        button(
            &mut commands,
            Button::PageDown,
            Rect::new(291.0, 121.0, 319.0, 156.0),
            "Page down".into(),
        );
    }

    let stock = Stock::of(&game, record);
    let can_edit = editable(&game, data, record);
    let invention_name = |invention: u8| {
        data.exe
            .ds_string(0x5d77 + 0x35 * u16::from(invention))
            .unwrap_or_default()
    };
    // Equipment names over the columns.
    for (j, e) in category.equipment.iter().take(category.columns).enumerate() {
        text(&mut commands, e.name.clone(), 5, YELLOW_TEXT, Vec2::new(144.0 + 38.0 * j as f32, 99.0));
    }
    for (i, kind) in category.kinds.iter().enumerate() {
        if !known(&game, kind.invention) {
            continue;
        }
        let at = kind_at(category, i);
        let y = row_y(i);
        let count = word(record, at);
        let kind_editable = can_edit && (matches!(stock, Stock::NewEarth) || kind.stock_index > 0);
        let colors = if kind_editable { RED_TEXT } else { YELLOW_TEXT };
        text(&mut commands, kind.name.clone(), 11, YELLOW_TEXT, Vec2::new(21.0, y));
        text(&mut commands, format!("{count:>3}"), 3, colors, Vec2::new(109.0, y));
        if kind_editable {
            button(
                &mut commands,
                Button::Count(i),
                Rect::new(106.0, y - 1.0, 132.0, y + 8.0),
                invention_name(kind.invention),
            );
        }
        for j in 0..category.columns {
            let x = column_x(j);
            let capacity = kind.capacity.get(j).copied().unwrap_or(0);
            let shown = if capacity == 0 {
                "   -".to_string()
            } else {
                format!("{:>4}", word(record, at + 2 + 2 * j))
            };
            let Some(equipment) = category.equipment.get(j) else {
                continue;
            };
            let fit_editable = can_edit
                && (matches!(stock, Stock::NewEarth) || equipment.stock_index > 0)
                && known(&game, equipment.invention);
            let colors = if fit_editable && capacity > 0 { RED_TEXT } else { YELLOW_TEXT };
            text(&mut commands, shown, 4, colors, Vec2::new(x, y));
            if fit_editable {
                button(
                    &mut commands,
                    Button::Fitted(i, j),
                    Rect::new(x - 3.0, y - 1.0, x + 30.0, y + 8.0),
                    invention_name(equipment.invention),
                );
            }
        }
    }
    // The stock line: names, and numbers where they can be taken from.
    for kind in &category.kinds {
        if !known(&game, kind.invention) {
            continue;
        }
        let at = |(x, y): (u16, u8)| Vec2::new(f32::from(x), f32::from(y));
        text(
            &mut commands,
            kind.name.clone(),
            kind.name_columns as usize,
            YELLOW_TEXT,
            at(kind.name_at),
        );
        if can_edit && let Some(n) = stock.get(&game, kind.invention, kind.stock_index) {
            text(&mut commands, format!("{n:>4}"), 4, YELLOW_TEXT, at(kind.stock_at));
        }
    }
    for e in &category.equipment {
        if !known(&game, e.invention) {
            continue;
        }
        let at = |(x, y): (u16, u8)| Vec2::new(f32::from(x), f32::from(y));
        text(&mut commands, e.name.clone(), 5, YELLOW_TEXT, at(e.name_at));
        if can_edit && let Some(n) = stock.get(&game, e.invention, e.stock_index) {
            text(&mut commands, format!("{n:>4}"), 4, YELLOW_TEXT, at(e.stock_at));
        }
    }
}

/// Everything the screen shows: the view, the unit, all bases (their stock),
/// the inventions (New Earth's stock) and the name being typed.
fn shown_state(game: &Game, view: &View, editing: Option<&TextEditing>) -> Vec<u8> {
    let mut key = vec![view.list as u8, view.unit as u8, view.category as u8];
    key.extend(game.0.unit(view.list, view.unit).unwrap_or_default());
    for b in 1..=game.0.unit_count(UnitList::Bases) {
        key.extend(game.0.unit(UnitList::Bases, b).unwrap_or_default());
    }
    key.extend(game.0.block(0x5dac).unwrap_or_default());
    if let Some(e) = editing {
        key.extend(e.text.bytes());
        key.push(e.done as u8);
    }
    key
}

/// FUN_357b_355e with the status prefix: "In the dock of Amnes system New
/// Earth", "Fly to ...".
fn location(game: &Game, data: &GameData, record: &[u8]) -> String {
    let (s, p, m) = (record[unit::SYSTEM], record[unit::PLANET], record[unit::MOON]);
    let star = data
        .star_systems
        .get((s as usize).wrapping_sub(1))
        .map(|l| l.name.trim_end().to_string())
        .unwrap_or_default();
    let mut place = format!("{star} system {}", game.body_name(&data.star_systems, s.into(), p.into(), 0));
    if m > 0 {
        place.push(' ');
        place.push_str(&game.body_name(&data.star_systems, s.into(), p.into(), m.into()));
    }
    let prefix = match record[unit::STATUS] {
        4..=6 => "Fly to ",
        2 => "In orbit ",
        1 => "In the dock of ",
        _ => "",
    };
    format!("{prefix}{place}")
}

/// FUN_28cd_03fb: SHIP INFO and BACK, plus what fits the unit.
fn update_actions(
    view: Option<Res<View>>,
    game: Option<Res<Game>>,
    handle: Res<GameDataHandle>,
    data: Res<Assets<GameData>>,
    mut extra: ResMut<ExtraActions>,
) {
    let (Some(view), Some(game), Some(data)) = (view, game, data.get(&handle.0)) else {
        return;
    };
    let Some(record) = game.0.unit(view.list, view.unit) else {
        return;
    };
    let mut actions = Vec::new();
    if view.list == UnitList::Groups {
        actions.push(CONTROL_PANEL);
    }
    if matches!(record[unit::TYPE], 2 | 3) {
        actions.push(TRANSFER);
    }
    if matches!(record[unit::STATUS], 1 | 2 | 7) {
        actions.extend([GALACTIC_MAP, PLANET_MAIN]);
    }
    // FUN_2b8d_1bad: an empty group docked at your colony can be disbanded.
    let empty = categories(data, record).iter().all(|c| {
        (0..c.kinds.len()).all(|i| word(record, kind_at(c, i)) == 0)
    });
    if view.list == UnitList::Groups
        && record[unit::STATUS] == 1
        && empty
        && editable(&game, data, record)
    {
        actions.push(DISBAND);
    }
    if extra.0 != actions {
        extra.0 = actions;
    }
}

fn activate(
    activated: On<Activated>,
    buttons: Query<&Button>,
    view: Option<ResMut<View>>,
    game: Option<ResMut<Game>>,
    handle: Res<GameDataHandle>,
    data: Res<Assets<GameData>>,
    mut commands: Commands,
) {
    let (Ok(&button), Some(mut view), Some(mut game), Some(data)) =
        (buttons.get(activated.0), view, game, data.get(&handle.0))
    else {
        return;
    };
    commands.trigger(Sfx::named("x"));
    match button {
        Button::Name => {
            let name = game.0.unit(view.list, view.unit).map(unit_name).unwrap_or_default();
            commands.insert_resource(TextEditing::new(name, NAME_COLUMNS));
        }
        Button::Previous | Button::Next => {
            // FUN_2b8d_0002: groups then bases, as one list.
            let groups = game.0.unit_count(UnitList::Groups);
            let bases = game.0.unit_count(UnitList::Bases);
            let (list, n) = match (button, view.list) {
                (Button::Previous, UnitList::Bases) if view.unit == 1 && groups > 0 => {
                    (UnitList::Groups, groups)
                }
                (Button::Previous, list) => (list, view.unit.saturating_sub(1).max(1)),
                (_, UnitList::Groups) if view.unit == groups && bases > 0 => (UnitList::Bases, 1),
                (_, list) => (list, view.unit + 1),
            };
            if game.0.unit(list, n).is_none() {
                return;
            }
            game.0.set_word(LIST, list as u16);
            let count = game.0.unit_count(list) as u16;
            game.0.set_word(LIST_COUNT, count);
            select(&mut game, list, n);
            view.list = list;
            view.unit = n;
            view.category = first_category(data, &game, list, n).unwrap_or(1);
        }
        Button::PageUp | Button::PageDown => {
            let Some(record) = game.0.unit(view.list, view.unit) else {
                return;
            };
            let numbers: Vec<usize> = categories(data, record).iter().map(|c| c.number).collect();
            let Some(pos) = numbers.iter().position(|&c| c == view.category) else {
                return;
            };
            let next = if button == Button::PageUp {
                pos.checked_sub(1)
            } else {
                Some(pos + 1)
            };
            if let Some(&c) = next.and_then(|p| numbers.get(p)) {
                view.category = c;
            }
        }
        Button::Count(i) => add_kind(&mut game, data, &view, i),
        Button::Fitted(i, j) => fit(&mut game, data, &view, i, j),
    }
}

fn take_back(
    used: On<AlternateUse>,
    buttons: Query<&Button>,
    view: Option<Res<View>>,
    game: Option<ResMut<Game>>,
    handle: Res<GameDataHandle>,
    data: Res<Assets<GameData>>,
) {
    let (Ok(&button), Some(view), Some(mut game), Some(data)) =
        (buttons.get(used.0), view, game, data.get(&handle.0))
    else {
        return;
    };
    match button {
        Button::Count(i) => remove_kind(&mut game, data, &view, i),
        Button::Fitted(i, j) => unfit(&mut game, data, &view, i, j),
        _ => {}
    }
}

/// The category shown and the unit's record, copied.
fn current(game: &Game, data: &GameData, view: &View) -> Option<(UnitCategory, Vec<u8>)> {
    let record = game.0.unit(view.list, view.unit)?.to_vec();
    let category = categories(data, &record)
        .iter()
        .find(|c| c.number == view.category)?
        .clone();
    Some((category, record))
}

fn set_word(game: &mut Game, view: &View, at: usize, value: u16) {
    if let Some(r) = game.0.unit_mut(view.list, view.unit) {
        r[at..at + 2].copy_from_slice(&value.to_le_bytes());
    }
}

/// One more of kind `i` from stock.
fn add_kind(game: &mut Game, data: &GameData, view: &View, i: usize) {
    let Some((category, record)) = current(game, data, view) else {
        return;
    };
    let Some(kind) = category.kinds.get(i) else { return };
    let stock = Stock::of(game, &record);
    let at = kind_at(&category, i);
    let count = word(&record, at);
    if stock.get(game, kind.invention, kind.stock_index).unwrap_or(0) > 0 && count < MAX_OF_A_KIND {
        set_word(game, view, at, count + 1);
        stock.add(game, kind.invention, kind.stock_index, -1);
    }
}

/// One of kind `i` back to stock, with the equipment it no longer has room for.
fn remove_kind(game: &mut Game, data: &GameData, view: &View, i: usize) {
    let Some((category, record)) = current(game, data, view) else {
        return;
    };
    let Some(kind) = category.kinds.get(i) else { return };
    let stock = Stock::of(game, &record);
    let at = kind_at(&category, i);
    let count = word(&record, at);
    if count == 0 {
        return;
    }
    let count = count - 1;
    set_word(game, view, at, count);
    stock.add(game, kind.invention, kind.stock_index, 1);
    for (j, equipment) in category.equipment.iter().enumerate() {
        let fitted_at = at + 2 + 2 * j;
        let fitted = word(&record, fitted_at);
        let room = u16::from(kind.capacity.get(j).copied().unwrap_or(0)) * count;
        let excess = fitted.saturating_sub(room);
        if excess > 0 {
            set_word(game, view, fitted_at, fitted - excess);
            stock.add(game, equipment.invention, equipment.stock_index, excess as i16);
        }
    }
}

/// One more equipment `j` on kind `i`, while there's room: its capacity per
/// ship (satellite carriers: one satellite per carrier in all).
fn fit(game: &mut Game, data: &GameData, view: &View, i: usize, j: usize) {
    let Some((category, record)) = current(game, data, view) else {
        return;
    };
    let (Some(kind), Some(equipment)) = (category.kinds.get(i), category.equipment.get(j)) else {
        return;
    };
    let stock = Stock::of(game, &record);
    let at = kind_at(&category, i);
    let count = word(&record, at);
    let fitted = word(&record, at + 2 + 2 * j);
    let room = u16::from(kind.capacity.get(j).copied().unwrap_or(0)) * count;
    let all_fitted: u16 = (0..4).map(|k| word(&record, at + 2 + 2 * k)).sum();
    let carrier_full = record[unit::TYPE] == 4 && all_fitted >= count;
    if stock.get(game, equipment.invention, equipment.stock_index).unwrap_or(0) > 0
        && fitted < room
        && !carrier_full
    {
        set_word(game, view, at + 2 + 2 * j, fitted + 1);
        stock.add(game, equipment.invention, equipment.stock_index, -1);
    }
}

fn unfit(game: &mut Game, data: &GameData, view: &View, i: usize, j: usize) {
    let Some((category, record)) = current(game, data, view) else {
        return;
    };
    let Some(equipment) = category.equipment.get(j) else { return };
    let stock = Stock::of(game, &record);
    let at = kind_at(&category, i) + 2 + 2 * j;
    let fitted = word(&record, at);
    if fitted > 0 {
        set_word(game, view, at, fitted - 1);
        stock.add(game, equipment.invention, equipment.stock_index, 1);
    }
}

fn finish_typing(
    editing: Option<Res<TextEditing>>,
    view: Option<Res<View>>,
    game: Option<ResMut<Game>>,
    mut commands: Commands,
) {
    let (Some(editing), Some(view), Some(mut game)) = (editing, view, game) else {
        return;
    };
    if editing.done {
        game.0.rename_unit(view.list, view.unit, editing.text.trim_end());
        commands.remove_resource::<TextEditing>();
    }
}

/// DISBAND UNIT.
fn use_action(
    action: On<ActionUsed>,
    screen: Res<State<GameScreen>>,
    view: Option<Res<View>>,
    game: Option<ResMut<Game>>,
    mut commands: Commands,
) {
    if action.0 != DISBAND || *screen.get() != GameScreen::Group {
        return;
    }
    let (Some(view), Some(mut game)) = (view, game) else {
        return;
    };
    game.0.disband_unit(view.list, view.unit);
    commands.trigger(GoTo(GameScreen::ShipInfo));
}
