//! The ground war: dividing your forces (screen 19), the battle (screen
//! 32) and, when it decides the game, its end (screens 35 and 36).
//! The battle itself is [`reunion_formats::ground`].
//!
//! Set-up (FUN_1430_033c / 0d3c): WAR/GRBEOSZT at row 49, twenty places
//! (four columns, 77 apart from x 10; five rows, 23 apart from y 56) with
//! the unit's picture, "Pcs" and "Str"; what's left below. Each place
//! removes its unit, or (its number) takes one piece more (the other button
//! one less). The icon bar adds units of the kinds left; OK ATTACK starts.
//!
//! Battle (FUN_1537_0029 / 0da8): WAR/GRWAR on the left (the selected unit,
//! Move and Attack), the battlefield from WAR/GRF<planet type> from x 64,
//! units from WAR/GRICON (turned to face their way), explosions and rockets
//! from WAR/GRICON2. Select a unit of yours, then Move (a cell) or Attack
//! (an enemy); the other button on a cell does the same directly.
//!
//! When it's over: WAR/GRVICT or GRLOST with the losses, END BATTLE.

use bevy::prelude::*;
use bevy::sprite::Anchor;
use reunion_formats::ground::{COLUMNS, GroundBattle, Outcome, ROWS};

use crate::focus::{Activated, AlternateUse, DefaultFocus, Hover, hotspot};
use crate::game::{Game, random};
use crate::game_data::{GameData, GameDataHandle};
use crate::hud::{ActionUsed, CONTENT_Y, ExtraActions, HoverLabel, IconSetOverride, YELLOW_TEXT};
use crate::pic::{MASKED, rgba_image};
use crate::screen::{GameScreen, place};
use crate::space_battle::BattleStart;
use crate::story::Tell;
use crate::text::{Label, label};
use crate::transition::GoTo;

pub struct GroundWarPlugin;

impl Plugin for GroundWarPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(OnEnter(GameScreen::GroundSetup), enter_setup)
            .add_systems(Update, show_setup.run_if(in_state(GameScreen::GroundSetup)))
            .add_systems(OnEnter(GameScreen::GroundWar), enter_battle)
            .add_systems(Update, (fight, draw, show_selected).chain().run_if(in_state(GameScreen::GroundWar)))
            .add_systems(OnEnter(GameScreen::GameEnd), enter_end)
            .add_observer(setup_action)
            .add_observer(setup_place)
            .add_observer(setup_less)
            .add_observer(battle_action)
            .add_observer(battle_click)
            .add_observer(battle_order)
            .add_observer(end_click);
    }
}

const OK_ATTACK: u8 = 11;
const CANCEL_ATTACK: u8 = 42;
const ADD_UNIT: [u8; 4] = [12, 19, 44, 36];
const RETREAT: u8 = 72;
const END_BATTLE: u8 = 65;
const BATTLE_OVER_SET: u8 = 30;
const KIND_NAMES: [&str; 4] = ["Trooper", "Battle tank", "Aircraft", "Rocket launcher"];
const FIELD: Vec2 = Vec2::new(64.0, CONTENT_Y + 3.0);
const VGA_REFRESH_HZ: f64 = 70.086;

/// The ground war being prepared or fought.
#[derive(Resource)]
struct War {
    battle: GroundBattle,
    frames: f64,
    retreated: bool,
    result: Option<[[i64; 4]; 2]>,
    selected: Option<(usize, usize)>,
    mode: Mode,
    changed: bool,
}

#[derive(Clone, Copy, PartialEq)]
enum Mode {
    Select,
    Move,
    Attack,
}

/// How the game ended (screen 35 or 36).
#[derive(Resource, Clone, Copy)]
pub struct GameEnd(pub Outcome);

#[derive(Component, Clone)]
struct SetupPart;

/// A place of the set-up: remove its unit, or change its size.
#[derive(Component, Clone, Copy)]
enum Place {
    Remove(usize),
    Resize(usize),
}

#[derive(Component)]
struct Cell(u8, u8);

#[derive(Component)]
enum Button {
    Move,
    Attack,
}

#[derive(Component, Clone)]
struct InfoPart;

fn enter_setup(
    mut commands: Commands,
    start: Option<Res<BattleStart>>,
    game: Option<Res<Game>>,
    handle: Res<GameDataHandle>,
    data: Res<Assets<GameData>>,
    asset_server: Res<AssetServer>,
) {
    let (Some(start), Some(game), Some(data)) = (start, game, data.get(&handle.0)) else {
        commands.trigger(GoTo(GameScreen::GalacticMap));
        return;
    };
    let battle = GroundBattle::new(&game.0, &data.exe, start.place, start.you_attack);
    commands.insert_resource(War {
        battle,
        frames: 0.0,
        retreated: false,
        result: None,
        selected: None,
        mode: Mode::Select,
        changed: true,
    });
    commands.spawn((
        Sprite::from_image(asset_server.load("WAR/GRBEOSZT.PIC")),
        Anchor::TOP_LEFT,
        place(Vec2::new(0.0, CONTENT_Y), 0.0),
        DespawnOnExit(GameScreen::GroundSetup),
    ));
}

fn slot_corner(i: usize) -> Vec2 {
    Vec2::new(10.0 + 77.0 * (i % 4) as f32, 56.0 + 23.0 * (i / 4) as f32)
}

/// FUN_1430_0d3c: the places and what's left; the icon bar's ADD ... UNIT.
fn show_setup(
    mut commands: Commands,
    war: Option<ResMut<War>>,
    start: Option<Res<BattleStart>>,
    asset_server: Res<AssetServer>,
    parts: Query<Entity, With<SetupPart>>,
    mut extra: ResMut<ExtraActions>,
) {
    let Some(mut war) = war else { return };
    if !war.changed {
        return;
    }
    war.changed = false;
    for part in &parts {
        commands.entity(part).despawn();
    }
    let scoped = (SetupPart, DespawnOnExit(GameScreen::GroundSetup));
    let icons = asset_server.load::<Image>(format!("WAR/GRICON.PIC#{MASKED}"));
    let text = |commands: &mut Commands, s: String, columns: usize, at: Vec2| {
        commands.spawn((label(Label::new(s, columns, YELLOW_TEXT), at), scoped.clone()));
    };
    let battle = &war.battle;
    for (i, u) in battle.units[0].iter().enumerate() {
        let corner = slot_corner(i);
        let min = Vec2::new(80.0 * f32::from(u.kind - 1), 0.0);
        commands.spawn((
            Sprite { image: icons.clone(), rect: Some(Rect::from_corners(min, min + 16.0)), ..default() },
            Anchor::TOP_LEFT,
            place(corner, 0.5),
            scoped.clone(),
        ));
        text(&mut commands, format!("{:>3}", u.pieces), 3, corner + Vec2::new(51.0, 0.0));
        let strength = i64::from(u.pieces) * battle.forces[0].attack[usize::from(u.kind) - 1]
            / battle.forces[0].pieces[usize::from(u.kind) - 1].max(1);
        text(&mut commands, format!("{strength:>4}"), 4, corner + Vec2::new(45.0, 8.0));
        let mut remove = commands.spawn((
            Place::Remove(i),
            HoverLabel("Remove unit".into()),
            hotspot(Rect::from_corners(corner, corner + Vec2::new(18.0, 16.0)), Hover::Outline),
            scoped.clone(),
        ));
        if i == 0 {
            remove.insert(DefaultFocus);
        }
        let at = corner + Vec2::new(41.0, 0.0);
        commands.spawn((
            Place::Resize(i),
            HoverLabel("Add/sub".into()),
            hotspot(Rect::from_corners(at, at + Vec2::new(30.0, 16.0)), Hover::Outline),
            scoped.clone(),
        ));
    }
    for (k, name) in KIND_NAMES.iter().enumerate() {
        let left = battle.reserve[0][k];
        if left <= 0 {
            continue;
        }
        let (x, y) = ([60.0, 60.0, 160.0, 160.0][k], [176.0, 186.0, 176.0, 186.0][k]);
        text(&mut commands, (*name).to_string(), 16, Vec2::new(x, y));
        text(&mut commands, format!("{left:>5}"), 5, Vec2::new(x + 70.0 + if k >= 2 { 30.0 } else { 0.0 }, y));
    }
    // FUN_1430_033c: CANCEL ATTACK when you attack, a button per kind left.
    let mut actions = Vec::new();
    if start.is_some_and(|s| s.you_attack) || battle.you_attack {
        actions.push(CANCEL_ATTACK);
    }
    for (k, &action) in ADD_UNIT.iter().enumerate() {
        if battle.reserve[0][k] > 0 {
            actions.push(action);
        }
    }
    if extra.0 != actions {
        extra.0 = actions;
    }
}

fn setup_place(activated: On<Activated>, places: Query<&Place>, war: Option<ResMut<War>>) {
    let (Ok(&place), Some(mut war)) = (places.get(activated.0), war) else { return };
    match place {
        Place::Remove(i) => war.battle.remove_unit(0, i),
        Place::Resize(i) => war.battle.resize_unit(0, i, 1),
    }
    war.changed = true;
}

fn setup_less(used: On<AlternateUse>, places: Query<&Place>, war: Option<ResMut<War>>) {
    let (Ok(&Place::Resize(i)), Some(mut war)) = (places.get(used.0), war) else { return };
    war.battle.resize_unit(0, i, -1);
    war.changed = true;
}

fn setup_action(
    action: On<ActionUsed>,
    screen: Res<State<GameScreen>>,
    war: Option<ResMut<War>>,
    mut commands: Commands,
) {
    if *screen.get() != GameScreen::GroundSetup {
        return;
    }
    let Some(mut war) = war else { return };
    match action.0 {
        OK_ATTACK => {
            war.battle.deploy();
            commands.trigger(GoTo(GameScreen::GroundWar));
        }
        CANCEL_ATTACK => commands.trigger(GoTo(GameScreen::GalacticMap)),
        a => {
            if let Some(k) = ADD_UNIT.iter().position(|&x| x == a) {
                war.battle.add_unit(0, k as u8 + 1);
                war.changed = true;
            }
        }
    }
}

fn enter_battle(
    mut commands: Commands,
    war: Option<ResMut<War>>,
    game: Option<Res<Game>>,
    handle: Res<GameDataHandle>,
    data: Res<Assets<GameData>>,
    asset_server: Res<AssetServer>,
    mut images: ResMut<Assets<Image>>,
) {
    let (Some(mut war), Some(game), Some(data)) = (war, game, data.get(&handle.0)) else {
        commands.trigger(GoTo(GameScreen::GalacticMap));
        return;
    };
    war.changed = true;
    let scoped = DespawnOnExit(GameScreen::GroundWar);
    let place_ = war.battle.place;
    let planet_type = game
        .body_number(&data.star_systems, place_)
        .and_then(|b| game.0.body(place_.0.into(), b))
        .map_or(1, |r| r[0x15]);
    let terrain = if matches!(planet_type, 1 | 3..=8) { planet_type } else { 1 };
    // The panel on the left: WAR/GRWAR's rows from 49.
    commands.spawn((
        Sprite {
            image: asset_server.load("WAR/GRWAR.PIC"),
            rect: Some(Rect::new(0.0, CONTENT_Y, 64.0, 200.0)),
            ..default()
        },
        Anchor::TOP_LEFT,
        place(Vec2::new(0.0, CONTENT_Y), 0.0),
        scoped.clone(),
    ));
    let image = images.add(rgba_image(256, 151, vec![0; 256 * 151 * 4]));
    commands.spawn((
        Sprite::from_image(image.clone()),
        Anchor::TOP_LEFT,
        place(Vec2::new(64.0, CONTENT_Y), 0.2),
        scoped.clone(),
    ));
    commands.insert_resource(FieldImage {
        image,
        field: asset_server.load(format!("WAR/GRF{terrain}.PIC")),
        icons: asset_server.load("WAR/GRICON.PIC"),
        effects: asset_server.load("WAR/GRICON2.PIC"),
    });
    for c in 0..COLUMNS as u8 {
        for r in 0..ROWS as u8 {
            let min = FIELD + Vec2::new(16.0 * f32::from(c), 16.0 * f32::from(r));
            let mut cell = commands.spawn((
                Cell(c, r),
                HoverLabel(String::new()),
                hotspot(Rect::from_corners(min, min + 16.0), Hover::Outline),
                scoped.clone(),
            ));
            if (c, r) == (2, 4) {
                cell.insert(DefaultFocus);
            }
        }
    }
    for (button, y, name) in [(Button::Move, 168.0, "Move"), (Button::Attack, 184.0, "Attack")] {
        commands.spawn((
            button,
            HoverLabel(name.into()),
            hotspot(Rect::new(0.0, y, 64.0, y + 16.0), Hover::Outline),
            scoped.clone(),
        ));
    }
}

#[derive(Resource)]
struct FieldImage {
    image: Handle<Image>,
    field: Handle<Image>,
    icons: Handle<Image>,
    effects: Handle<Image>,
}

fn fight(
    time: Res<Time>,
    war: Option<ResMut<War>>,
    game: Option<ResMut<Game>>,
    handle: Res<GameDataHandle>,
    data: Res<Assets<GameData>>,
    asset_server: Res<AssetServer>,
    mut set: ResMut<IconSetOverride>,
    cells: Query<Entity, Or<(With<Cell>, With<Button>)>>,
    mut commands: Commands,
) {
    let (Some(mut war), Some(mut game), Some(data)) = (war, game, data.get(&handle.0)) else { return };
    if war.result.is_some() {
        return;
    }
    war.frames += time.delta_secs_f64() * VGA_REFRESH_HZ;
    let cheat = game.0.byte(0x91e8).unwrap_or(0) != 0;
    while war.frames >= 1.0 && !war.battle.over {
        war.frames -= 1.0;
        war.battle.frame(cheat, &mut random);
        war.changed = true;
    }
    if !war.battle.over && !war.retreated {
        return;
    }
    // FUN_1537_312c: the result.
    let losses = war.battle.finish(&mut game.0, &data.exe);
    war.result = Some(losses);
    war.selected = None;
    for cell in &cells {
        commands.entity(cell).despawn();
    }
    commands.spawn((
        HoverLabel(String::new()),
        hotspot(Rect::new(0.0, CONTENT_Y, 320.0, 200.0), Hover::Outline),
        DefaultFocus,
        DespawnOnExit(GameScreen::GroundWar),
    ));
    set.0 = Some(BATTLE_OVER_SET);
    let won = war.battle.won() && !war.retreated;
    let scoped = DespawnOnExit(GameScreen::GroundWar);
    let name = if won { "GRVICT" } else { "GRLOST" };
    commands.spawn((
        Sprite::from_image(asset_server.load(format!("WAR/{name}.PIC"))),
        Anchor::TOP_LEFT,
        place(Vec2::new(0.0, CONTENT_Y), 1.0),
        scoped.clone(),
    ));
    for (side, top) in [(0, 0x4f), (1, 0x86)] {
        for k in 1..=4 {
            let at = Vec2::new(87.0, (top + 9 * k) as f32);
            let text = format!("{:>4}", losses[side][k - 1].max(0));
            commands.spawn((label(Label::new(text, 4, YELLOW_TEXT), at), scoped.clone())).insert(place(at, 1.1));
        }
    }
}

/// The battlefield: ground, headquarters, units, rockets, markers.
fn draw(war: Option<ResMut<War>>, pictures: Option<Res<FieldImage>>, mut images: ResMut<Assets<Image>>) {
    let (Some(mut war), Some(pictures)) = (war, pictures) else { return };
    if !war.changed {
        return;
    }
    let (Some(field), Some(icons), Some(effects)) = (
        images.get(&pictures.field).and_then(|i| i.data.clone()),
        images.get(&pictures.icons).and_then(|i| i.data.clone()),
        images.get(&pictures.effects).and_then(|i| i.data.clone()),
    ) else {
        return;
    };
    war.changed = false;
    const W: i32 = 256;
    const H: i32 = 151;
    let mut pixels = vec![0u8; (W * H * 4) as usize];
    // The ground: GRF's rows 49-199, columns 64-319.
    for y in 0..H {
        for x in 0..W {
            let s = (((y + 49) * 320 + x + 64) * 4) as usize;
            let t = ((y * W + x) * 4) as usize;
            if let Some(px) = field.get(s..s + 4) {
                pixels[t..t + 4].copy_from_slice(px);
            }
        }
    }
    // Copies a picture part, black left out, turned: 1 as it is (facing
    // left), 2 up, 3 right, 4 down.
    let mut blit = |source: &[u8], width: i32, from: (i32, i32), size: (i32, i32), at: (i32, i32), turn: u8| {
        for dy in 0..size.1 {
            for dx in 0..size.0 {
                let (sx, sy) = match turn {
                    2 => (dy, size.0 - 1 - dx),
                    3 => (size.0 - 1 - dx, dy),
                    4 => (size.1 - 1 - dy, dx),
                    _ => (dx, dy),
                };
                let s = (((from.1 + sy) * width + from.0 + sx) * 4) as usize;
                let Some(px) = source.get(s..s + 4) else { continue };
                if px[..3] == [0, 0, 0] {
                    continue;
                }
                let (x, y) = (at.0 + dx, at.1 + dy);
                if x < 0 || y < 0 || x >= W || y >= H {
                    continue;
                }
                let t = ((y * W + x) * 4) as usize;
                pixels[t..t + 4].copy_from_slice(px);
            }
        }
    };
    let battle = &war.battle;
    // FUN_1537_0ca0: the headquarters from GRF's top rows.
    for (side, ((c, r), size)) in battle.headquarters.into_iter().enumerate() {
        let from = match (side, size) {
            (0, 2) => (64, 0),
            (0, _) => (64, 33),
            (_, 2) => (192, 0),
            _ => (132, 33),
        };
        let s = i32::from(size) * 16;
        blit(&field, 320, from, (s, s), (i32::from(c) * 16, i32::from(r) * 16 + 3), 1);
    }
    for side in 0..2 {
        for u in &battle.units[side] {
            if u.pieces <= 0 && u.animation == 0 {
                continue;
            }
            let (x, y) = u.position();
            let (cell, row, on_effects) = battle.picture(side, u);
            let from = (16 * (i32::from(cell) - 1), 16 * (i32::from(row) - 1));
            if u.pieces > 0 {
                let base = ((i32::from(u.kind) - 1) * 80, 16 * side as i32);
                let own = if on_effects { base } else { from };
                blit(&icons, 320, own, (16, 16), (x, y + 3), u.facing);
            }
            if on_effects {
                blit(&effects, 320, from, (16, 16), (x, y + 3), 1);
            }
        }
        for r in &battle.rockets[side] {
            let (x, y) = r.position();
            blit(&effects, 320, (1 + 11 * i32::from(r.heading), 16), (10, 11), (x + 3, y + 5), 1);
        }
    }
    // FUN_1537_0a0c: the selected unit, and its target.
    if let Some((side, i)) = war.selected
        && let Some(u) = battle.units[side].get(i)
    {
        let (x, y) = u.position();
        blit(&effects, 320, (224, 0), (16, 16), (x, y + 3), 1);
        if side == 0 {
            match u.order {
                reunion_formats::ground::MOVE => {
                    blit(&effects, 320, (240, 16), (16, 16), (i32::from(u.target) * 16, i32::from(u.target_row) * 16 + 3), 1);
                }
                reunion_formats::ground::ATTACK => {
                    if let Some(t) = battle.units[1].get(usize::from(u.target).wrapping_sub(1)) {
                        let (tx, ty) = t.position();
                        blit(&effects, 320, (240, 0), (16, 16), (tx, ty + 3), 1);
                    }
                }
                _ => {}
            }
        }
    }
    if let Some(mut image) = images.get_mut(&pictures.image) {
        image.data = Some(pixels);
    }
}

/// FUN_1537_0642: the selected unit on the left.
fn show_selected(
    mut commands: Commands,
    war: Option<Res<War>>,
    parts: Query<Entity, With<InfoPart>>,
    mut shown: Local<Option<(Option<(usize, usize)>, i32, u8)>>,
) {
    let Some(war) = war else { return };
    let unit = war.selected.and_then(|(s, i)| war.battle.units[s].get(i));
    let key = (war.selected, unit.map_or(0, |u| u.pieces), war.mode as u8);
    if shown.as_ref() == Some(&key) && !parts.is_empty() {
        return;
    }
    *shown = Some(key);
    for part in &parts {
        commands.entity(part).despawn();
    }
    let Some(((side, _), u)) = war.selected.zip(unit) else { return };
    let scoped = (InfoPart, DespawnOnExit(GameScreen::GroundWar));
    let k = usize::from(u.kind) - 1;
    let strength = i64::from(u.pieces) * war.battle.forces[side].attack[k] / war.battle.forces[side].pieces[k].max(1);
    for (i, text) in [KIND_NAMES[k].to_string(), format!("Pcs:{}", u.pieces), format!("Str:{strength}")].into_iter().enumerate() {
        let at = Vec2::new(5.0, 136.0 + 10.0 * i as f32);
        let colors = if i == 0 { crate::hud::RED_TEXT } else { YELLOW_TEXT };
        commands.spawn((label(Label::new(text, 9, colors), at), scoped.clone()));
    }
}

fn battle_click(
    activated: On<Activated>,
    cells: Query<&Cell>,
    buttons: Query<&Button>,
    war: Option<ResMut<War>>,
) {
    let Some(mut war) = war else { return };
    if war.result.is_some() {
        return;
    }
    if let Ok(button) = buttons.get(activated.0) {
        if war.selected.is_some_and(|(s, _)| s == 0) {
            war.mode = match button {
                Button::Move => Mode::Move,
                Button::Attack => Mode::Attack,
            };
        }
        return;
    }
    let Ok(&Cell(c, r)) = cells.get(activated.0) else { return };
    let at = (i32::from(c) * 16 + 8, i32::from(r) * 16 + 8);
    match (war.mode, war.selected) {
        (Mode::Move, Some((0, i))) => {
            war.battle.order_move(i, (c, r));
            war.mode = Mode::Select;
        }
        (Mode::Attack, Some((0, i))) => {
            if let Some((1, j)) = war.battle.unit_at(at) {
                war.battle.order_attack(i, j);
            }
            war.mode = Mode::Select;
        }
        _ => {
            war.selected = war.battle.unit_at(at);
            war.mode = Mode::Select;
        }
    }
    war.changed = true;
}

/// The other button on a cell: the selected unit moves there, or attacks
/// what's there.
fn battle_order(used: On<AlternateUse>, cells: Query<&Cell>, war: Option<ResMut<War>>) {
    let (Ok(&Cell(c, r)), Some(mut war)) = (cells.get(used.0), war) else { return };
    let Some((0, i)) = war.selected else { return };
    let at = (i32::from(c) * 16 + 8, i32::from(r) * 16 + 8);
    match war.battle.unit_at(at) {
        Some((1, j)) => war.battle.order_attack(i, j),
        _ => war.battle.order_move(i, (c, r)),
    }
    war.changed = true;
}

fn battle_action(
    action: On<ActionUsed>,
    screen: Res<State<GameScreen>>,
    war: Option<ResMut<War>>,
    game: Option<ResMut<Game>>,
    handle: Res<GameDataHandle>,
    data: Res<Assets<GameData>>,
    mut commands: Commands,
) {
    if *screen.get() != GameScreen::GroundWar {
        return;
    }
    let (Some(mut war), Some(mut game), Some(data)) = (war, game, data.get(&handle.0)) else { return };
    match action.0 {
        RETREAT if war.result.is_none() => war.retreated = true,
        END_BATTLE if war.result.is_some() => {
            let won = war.battle.won() && !war.retreated;
            let place = war.battle.place;
            let (events, outcome) =
                game.0.ground_aftermath(&data.exe, &data.sim_texts, place, won, war.battle.you_attack, &mut random);
            game.select(place.0.into(), place.1.into(), place.2.into());
            for event in events {
                commands.trigger(Tell(event));
            }
            match outcome {
                Outcome::Continue => commands.trigger(GoTo(GameScreen::GalacticMap)),
                end => {
                    commands.insert_resource(GameEnd(end));
                    commands.trigger(GoTo(GameScreen::GameEnd));
                }
            }
        }
        _ => {}
    }
}

/// Screens 35 and 36 (FUN_3abd_17dd / 1a47): GRAFIKA/DEATHSZ1 and your
/// hero's death, or the victory.
fn enter_end(mut commands: Commands, end: Option<Res<GameEnd>>, asset_server: Res<AssetServer>) {
    let path = match end.map(|e| e.0) {
        Some(Outcome::Won) => "VICTORY/END.PIC",
        _ => "GRAFIKA/DEATHSZ1.PIC",
    };
    commands.spawn((
        Sprite::from_image(asset_server.load(path)),
        Anchor::TOP_LEFT,
        place(Vec2::ZERO, 20.0),
        DespawnOnExit(GameScreen::GameEnd),
    ));
    commands.spawn((
        HoverLabel(String::new()),
        hotspot(Rect::new(0.0, 0.0, 320.0, 200.0), Hover::Outline),
        DefaultFocus,
        DespawnOnExit(GameScreen::GameEnd),
    ));
}

fn end_click(activated: On<Activated>, screen: Res<State<GameScreen>>, mut commands: Commands) {
    let _ = activated;
    if *screen.get() == GameScreen::GameEnd {
        commands.remove_resource::<Game>();
        commands.trigger(GoTo(GameScreen::MainMenu));
    }
}
