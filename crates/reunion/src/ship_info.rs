//! SHIP INFO (screen 16): your units, army groups or planet bases.
//!
//! From REUNION.PRG FUN_26fe_001b (logic), FUN_26fe_02e6 / 0693 (pictures),
//! FUN_26fe_04d4 (slots), FUN_26fe_093b (the panel), FUN_26fe_1278 (hotspots)
//! and FUN_28cd_0542 (icon bar):
//!
//! - GRAFIKA/SHIP1 (groups) or SHIP2 (bases) at row 49, with 32 slots in four
//!   columns of eight from (0, 60), 48 by 16 apart: a select button (16x17)
//!   and the unit's icon (32x17, from SHIP3: row by unit type, column by
//!   status; bases use the column of their status in the first row);
//! - the selected unit's details on the right: name, where it is or is going,
//!   and what it carries;
//! - CHANGE at the bottom switches between groups and bases.
//!
//! The icon bar gains CONTROL PANEL, GROUP, NEW UNIT, TRANSFER and PLANET MAIN
//! depending on the selected unit. Confirming the selected unit's icon again
//! opens GROUP (the original opens it on a second kind of click).

use bevy::prelude::*;
use bevy::sprite::Anchor;
use reunion_formats::state::{UnitList, unit};

use crate::audio::Sfx;
use crate::focus::{Activated, Hover, hotspot};
use crate::game::Game;
use crate::game_data::{GameData, GameDataHandle};
use crate::hud::{ActionUsed, CONTENT_Y, ExtraActions, HoverLabel, RED_TEXT, YELLOW_TEXT};
use crate::screen::{GameScreen, picture, place};
use crate::text::{Label, label};
use crate::transition::GoTo;

pub struct ShipInfoPlugin;

impl Plugin for ShipInfoPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            OnEnter(GameScreen::ShipInfo),
            enter.after(crate::hud::spawn_hud),
        )
        .add_systems(
            Update,
            (spawn_view, (update_panel, update_actions))
                .chain()
                .run_if(in_state(GameScreen::ShipInfo)),
        )
        .add_systems(OnExit(GameScreen::ShipInfo), |mut commands: Commands| {
            commands.remove_resource::<View>();
        })
        .add_observer(activate)
        .add_observer(new_unit);
    }
}

/// DS addresses: the list shown (0 groups, 1 bases), the number of units in
/// it, the selected unit, and the selection remembered per list.
pub const LIST: u16 = 0xa2ce;
pub const LIST_COUNT: u16 = 0xa2c2;
pub const SELECTED: u16 = 0xa2c8;
const SELECTED_PER_LIST: [u16; 2] = [0xa2ca, 0xa2cc];
/// Set once NEW UNIT is researched.
const CAN_CREATE: u16 = 0xa2d4;

const NEW_UNIT: u8 = 15;
const CONTROL_PANEL: u8 = 16;
const GROUP: u8 = 49;
const TRANSFER: u8 = 3;
const PLANET_MAIN: u8 = 41;

const SLOTS: usize = 32;
/// SHIP3: the selected slot's light, 6x7.
const SELECTED_LIGHT: Rect = Rect {
    min: Vec2::new(213.0, 22.0),
    max: Vec2::new(219.0, 29.0),
};
const CHANGE: Rect = Rect {
    min: Vec2::new(0.0, 188.0),
    max: Vec2::new(200.0, 200.0),
};
/// Rows of carried units, from y 116, 9 apart (plus 3 between categories),
/// in the area FUN_26fe_093b clears (199, 115, 114x77).
const ROWS: usize = 8;
const ROWS_Y: f32 = 116.0;

#[derive(Resource, Clone, Copy, PartialEq)]
struct View(UnitList);

#[derive(Component, Clone)]
struct ViewPart;

#[derive(Component)]
struct SlotButton(usize);

#[derive(Component)]
struct UnitIcon(usize);

#[derive(Component)]
struct ChangeButton;

#[derive(Component)]
struct SelectedLight;

/// Panel text: fixed lines, and rows of (name, colon, number).
#[derive(Component, Clone, Copy, PartialEq)]
enum Panel {
    Name,
    Status,
    /// Star names at the three x positions the three statuses use.
    StarLong,
    StarDestination,
    StarCurrent,
    Planet,
    Moon,
    Remaining,
    RowName(usize),
    RowColon(usize),
    RowNumber(usize),
}

fn slot_origin(k: usize) -> Vec2 {
    Vec2::new(
        ((k - 1) / 8) as f32 * 48.0,
        60.0 + ((k - 1) % 8) as f32 * 16.0,
    )
}

fn list_of(game: &Game) -> UnitList {
    if game.0.word(LIST) == Some(1) {
        UnitList::Bases
    } else {
        UnitList::Groups
    }
}

fn selected(game: &Game) -> usize {
    game.0.word(SELECTED).unwrap_or(0) as usize
}

fn enter(mut commands: Commands, game: Option<Res<Game>>) {
    let Some(game) = game else { return };
    commands.insert_resource(View(list_of(&game)));
}

fn spawn_view(
    mut commands: Commands,
    view: Option<Res<View>>,
    game: Option<Res<Game>>,
    asset_server: Res<AssetServer>,
    parts: Query<Entity, With<ViewPart>>,
) {
    let (Some(view), Some(game)) = (view, game) else {
        return;
    };
    if !view.is_changed() {
        return;
    }
    for part in &parts {
        commands.entity(part).despawn();
    }
    let scoped = (ViewPart, DespawnOnExit(GameScreen::ShipInfo));
    let list = view.0;
    let state = &game.0;
    let picture_name = if list == UnitList::Groups { "SHIP1" } else { "SHIP2" };
    commands.spawn((
        picture(
            asset_server.load(format!("GRAFIKA/{picture_name}.PIC")),
            Vec2::new(0.0, CONTENT_Y),
        ),
        scoped.clone(),
    ));
    let icons = asset_server.load::<Image>("GRAFIKA/SHIP3.PIC");
    for k in 1..=SLOTS {
        let origin = slot_origin(k);
        let record = state.unit(list, k);
        let name = record.map(unit_name).unwrap_or_default();
        commands.spawn((
            SlotButton(k),
            HoverLabel(name.clone()),
            hotspot(
                Rect::from_corners(origin, origin + Vec2::new(16.0, 17.0)),
                Hover::Outline,
            ),
            scoped.clone(),
        ));
        let Some(record) = record else { continue };
        let min = origin + Vec2::new(17.0, 0.0);
        commands.spawn((
            UnitIcon(k),
            HoverLabel(name),
            hotspot(Rect::from_corners(min, min + Vec2::new(32.0, 17.0)), Hover::Outline),
            scoped.clone(),
        ));
        commands.spawn((
            Sprite {
                image: icons.clone(),
                rect: Some(unit_icon(record)),
                ..default()
            },
            Anchor::TOP_LEFT,
            place(origin + Vec2::new(17.0, 1.0), 0.5),
            scoped.clone(),
        ));
    }
    commands.spawn((
        SelectedLight,
        Sprite {
            image: icons,
            rect: Some(SELECTED_LIGHT),
            ..default()
        },
        Anchor::TOP_LEFT,
        place(Vec2::ZERO, 0.6),
        Visibility::Hidden,
        scoped.clone(),
    ));
    commands.spawn((
        ChangeButton,
        HoverLabel("Change mode".into()),
        hotspot(CHANGE, Hover::Outline),
        scoped.clone(),
    ));

    let lines = [
        (Panel::Name, 202.0, 60.0, 18, RED_TEXT),
        (Panel::Status, 202.0, 70.0, 18, YELLOW_TEXT),
        (Panel::StarLong, 258.0, 70.0, 8, YELLOW_TEXT),
        (Panel::StarDestination, 276.0, 70.0, 5, YELLOW_TEXT),
        (Panel::StarCurrent, 282.0, 70.0, 5, YELLOW_TEXT),
        (Panel::Planet, 211.0, 79.0, 15, YELLOW_TEXT),
        (Panel::Moon, 220.0, 89.0, 14, YELLOW_TEXT),
        (Panel::Remaining, 202.0, 100.0, 18, YELLOW_TEXT),
    ];
    for (panel, x, y, columns, colors) in lines {
        let mut text = commands.spawn((
            panel,
            label(Label::new("", columns, colors), Vec2::new(x, y)),
            scoped.clone(),
        ));
        // Star names are drawn after (over) the heading.
        if matches!(panel, Panel::StarLong | Panel::StarDestination | Panel::StarCurrent) {
            text.insert(place(Vec2::new(x, y), 1.1));
        }
    }
    for row in 0..ROWS {
        for (panel, x, columns, colors) in [
            (Panel::RowName(row), 204.0, 11, YELLOW_TEXT),
            (Panel::RowColon(row), 274.0, 1, YELLOW_TEXT),
            (Panel::RowNumber(row), 280.0, 4, RED_TEXT),
        ] {
            commands.spawn((
                panel,
                label(
                    Label::new("", columns, colors),
                    Vec2::new(x, ROWS_Y + 9.0 * row as f32),
                ),
                scoped.clone(),
            ));
        }
    }
}

/// FUN_26fe_04d4: SHIP3 row (type - 1) * 16, column status - 1, for statuses
/// up to 6; higher statuses (bases) use their column of the first row.
fn unit_icon(record: &[u8]) -> Rect {
    let status = record[unit::STATUS].max(1) as f32;
    let row = if record[unit::STATUS] <= 6 {
        (record[unit::TYPE].max(1) - 1) as f32
    } else {
        0.0
    };
    let min = Vec2::new((status - 1.0) * 32.0 + 1.0, row * 16.0 + 1.0);
    Rect::from_corners(min, min + Vec2::new(31.0, 14.0))
}

pub fn unit_name(record: &[u8]) -> String {
    let len = (record[unit::NAME] as usize).min(unit::SYSTEM - unit::NAME - 1);
    record[unit::NAME + 1..unit::NAME + 1 + len]
        .iter()
        .map(|&b| b as char)
        .collect()
}

fn word(record: &[u8], offset: usize) -> u16 {
    u16::from_le_bytes([record[offset], record[offset + 1]])
}

/// What the panel shows for a unit (FUN_26fe_093b).
fn panel_text(game: &Game, data: &GameData, record: &[u8]) -> Vec<(Panel, String)> {
    let (system, planet, moon) = (
        record[unit::SYSTEM] as u16,
        record[unit::PLANET] as u16,
        record[unit::MOON] as u16,
    );
    let long_star = data
        .star_systems
        .get((system as usize).wrapping_sub(1))
        .map(|s| s.name.clone())
        .unwrap_or_default();
    let short_star = data
        .short_star_names
        .get((system as usize).wrapping_sub(1))
        .cloned()
        .unwrap_or_default();
    let planet_name = game.body_name(&data.star_systems, system, planet, 0);
    let moon_name = if moon == 0 {
        String::new()
    } else {
        game.body_name(&data.star_systems, system, planet, moon)
    };
    let mut lines = vec![(Panel::Name, unit_name(record))];
    let status = record[unit::STATUS];
    let (heading, star) = match status {
        7 => ("Base on: ", (Panel::StarLong, long_star)),
        4..=6 => ("Destination:", (Panel::StarDestination, short_star)),
        1 | 2 => ("Currently on:", (Panel::StarCurrent, short_star)),
        _ => ("", (Panel::StarCurrent, String::new())),
    };
    lines.push((Panel::Status, heading.into()));
    lines.push(star);
    lines.push((Panel::Planet, planet_name));
    lines.push((Panel::Moon, moon_name));
    let remaining = if (4..=6).contains(&status) {
        // Days: (pilots' level + the two distance parts) / (level + 1),
        // plus the days part.
        let level = u32::from(game.0.word(0x95a4).unwrap_or(0));
        let distance = u32::from(word(record, unit::TRAVEL_A)) + u32::from(word(record, unit::TRAVEL_B));
        let days = (level + distance) / (level + 1) + u32::from(word(record, unit::TRAVEL_DAYS));
        format!("Time remaining:{days}")
    } else {
        String::new()
    };
    lines.push((Panel::Remaining, remaining));
    lines
}

/// Rows of (name, number) for what a unit carries, and the gap before each
/// category's rows.
fn carried(data: &GameData, record: &[u8]) -> Vec<(String, u16, bool)> {
    let unit_type = record[unit::TYPE] as usize;
    let mut rows = Vec::new();
    for category in data.unit_categories.get(unit_type).into_iter().flatten() {
        let slot = unit::SLOTS + (category.slot - 1) * unit::SLOT_LEN;
        let mut first = true;
        for (i, name) in category.kinds.iter().enumerate() {
            let n = word(record, slot + i * unit::KIND_LEN);
            if n as i16 > 0 {
                rows.push((name.name.clone(), n, first));
                first = false;
            }
        }
    }
    if unit_type == 4 {
        // Satellite carriers count each satellite kind in the first kind's values.
        for (i, name) in data.satellites.iter().enumerate() {
            let n = word(record, unit::SLOTS + 2 * (i + 1));
            if n as i16 > 0 {
                rows.push((name.clone(), n, false));
            }
        }
    }
    if unit_type == 2 {
        let total: u16 = (1..=4)
            .map(|i| word(record, unit::SLOTS + i * unit::KIND_LEN - 2))
            .sum();
        if total as i16 > 0 {
            rows.push(("Miner stat".into(), total, true));
        }
    }
    rows
}

fn update_panel(
    view: Option<Res<View>>,
    game: Option<Res<Game>>,
    handle: Res<GameDataHandle>,
    data: Res<Assets<GameData>>,
    mut labels: Query<(&Panel, &mut Label, &mut Transform)>,
    mut light: Query<(&mut Transform, &mut Visibility), (With<SelectedLight>, Without<Panel>)>,
) {
    let (Some(view), Some(game), Some(data)) = (view, game, data.get(&handle.0)) else {
        return;
    };
    if !game.is_changed() && !view.is_changed() {
        return;
    }
    let selected = selected(&game);
    let record = game.0.unit(view.0, selected);
    if let Ok((mut transform, mut visibility)) = light.single_mut() {
        if record.is_some() {
            *transform = place(slot_origin(selected) + 5.0, 0.6);
            *visibility = Visibility::Inherited;
        } else {
            *visibility = Visibility::Hidden;
        }
    }
    let lines = record.map(|r| panel_text(&game, data, r)).unwrap_or_default();
    let rows = record.map(|r| carried(data, r)).unwrap_or_default();
    // Row y positions: 9 apart, 3 more before each category after the first.
    let mut y = ROWS_Y;
    let row_y: Vec<f32> = rows
        .iter()
        .enumerate()
        .map(|(i, (_, _, first))| {
            if *first && i > 0 {
                y += 3.0;
            }
            let this = y;
            y += 9.0;
            this
        })
        .collect();
    for (panel, label, mut transform) in &mut labels {
        let text = match panel {
            Panel::RowName(r) | Panel::RowColon(r) | Panel::RowNumber(r) => {
                let Some((name, n, _)) = rows.get(*r) else {
                    Label::set(label, "");
                    continue;
                };
                let x = match panel {
                    Panel::RowName(_) => 204.0,
                    Panel::RowColon(_) => 274.0,
                    _ => 280.0,
                };
                *transform = place(Vec2::new(x, row_y[*r]), 1.0);
                match panel {
                    Panel::RowName(_) => name.clone(),
                    Panel::RowColon(_) => ":".into(),
                    _ => format!("{n:>3}"),
                }
            }
            fixed => lines
                .iter()
                .find(|(p, _)| p == fixed)
                .map(|(_, t)| t.clone())
                .unwrap_or_default(),
        };
        Label::set(label, text);
    }
}

/// FUN_28cd_0542: the icon bar follows the selection.
fn update_actions(
    view: Option<Res<View>>,
    game: Option<Res<Game>>,
    mut extra: ResMut<ExtraActions>,
) {
    let (Some(view), Some(game)) = (view, game) else {
        return;
    };
    let can_create = game.0.byte(CAN_CREATE).unwrap_or(0) != 0 && view.0 == UnitList::Groups;
    let mut actions = Vec::new();
    match game.0.unit(view.0, selected(&game)) {
        None => {
            if can_create {
                actions.push(NEW_UNIT);
            }
        }
        Some(record) => {
            if view.0 == UnitList::Groups {
                actions.push(CONTROL_PANEL);
            }
            actions.push(GROUP);
            if can_create {
                actions.push(NEW_UNIT);
            }
            if matches!(record[unit::TYPE], 2 | 3) {
                actions.push(TRANSFER);
            }
            if matches!(record[unit::STATUS], 1 | 2 | 7) {
                actions.push(PLANET_MAIN);
            }
        }
    }
    if extra.0 != actions {
        extra.0 = actions;
    }
}

/// Selects unit `k` of the current list; PLANET MAIN and the galactic map
/// then look at where it is.
fn select(game: &mut Game, list: UnitList, k: usize) {
    game.0.set_word(SELECTED, k as u16);
    game.0.set_word(SELECTED_PER_LIST[list as usize], k as u16);
    if let Some(record) = game.0.unit(list, k) {
        let (s, p, m) = (record[unit::SYSTEM], record[unit::PLANET], record[unit::MOON]);
        game.select(s.into(), p.into(), m.into());
    }
}

fn activate(
    activated: On<Activated>,
    buttons: Query<&SlotButton>,
    icons: Query<&UnitIcon>,
    change: Query<(), With<ChangeButton>>,
    view: Option<ResMut<View>>,
    game: Option<ResMut<Game>>,
    mut commands: Commands,
) {
    let (Some(mut view), Some(mut game)) = (view, game) else {
        return;
    };
    let list = view.0;
    let count = game.0.unit_count(list);
    if let Ok(SlotButton(k)) = buttons.get(activated.0) {
        if *k <= count {
            commands.trigger(Sfx::named("x"));
            select(&mut game, list, *k);
        }
    } else if let Ok(UnitIcon(k)) = icons.get(activated.0) {
        if selected(&game) == *k {
            commands.trigger(Sfx::named("group"));
            commands.trigger(GoTo(GameScreen::Group));
        } else {
            select(&mut game, list, *k);
        }
    } else if change.contains(activated.0) {
        let other = match list {
            UnitList::Groups => UnitList::Bases,
            UnitList::Bases => UnitList::Groups,
        };
        commands.trigger(Sfx::named("change"));
        let remembered = game.0.word(SELECTED_PER_LIST[other as usize]).unwrap_or(0);
        game.0.set_word(LIST, other as u16);
        let count = game.0.unit_count(other) as u16;
        game.0.set_word(LIST_COUNT, count);
        game.0.set_word(SELECTED, remembered);
        view.0 = other;
    }
}

/// NEW UNIT: a new army group, then its name and type (screen 21).
fn new_unit(
    action: On<ActionUsed>,
    screen: Res<State<GameScreen>>,
    game: Option<ResMut<Game>>,
    mut commands: Commands,
) {
    if action.0 != NEW_UNIT || *screen.get() != GameScreen::ShipInfo {
        return;
    }
    let Some(mut game) = game else { return };
    game.0.set_word(LIST, UnitList::Groups as u16);
    let Some(k) = game.0.add_group() else { return };
    game.0.set_word(LIST_COUNT, k as u16);
    select(&mut game, UnitList::Groups, k);
    commands.trigger(GoTo(GameScreen::CreateUnit));
}
