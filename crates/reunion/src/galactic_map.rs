//! GALACTIC MAP (screen 7): the selected star system as an orrery.
//!
//! From REUNION.PRG FUN_18e0_00e8 (screen logic), FUN_357b_23db (set-up),
//! FUN_357b_3010 (orbits), FUN_357b_29cd / 270b / 27e9 (drawing) and
//! FUN_357b_18cc (hotspots). Two views:
//!
//! - planets (selected planet 0): PLANETS/HATTER1 at row 49, the star (64x64
//!   from NAPR<system>) in the middle and the planets circling it, drawn 16 to
//!   32 pixels wide depending on how near they are;
//! - moons of the selected planet: HATTER2, the planet in the middle and its
//!   moons, 8 to 16 pixels, with a 7x7 status icon from HATTER4. The icon bar
//!   gains ZOOM OUT.
//!
//! Clicking a body opens PLANET INFO for it. Opened from the main screen the
//! map shows the planets; from PLANET INFO, PLANET MAIN or a unit's screens
//! it shows the moons of the planet being looked at.
//!
//! Moving a group (screen 18, FUN_18e0_07ec) uses the map to pick where to:
//! "SELECT DESTINATION" blinks, the icon bar has ABORT MOVE, known systems
//! get buttons on the right (other systems need a hyperdrive and a good
//! enough pilot), and picking a planet, a moon or an unexplored system's
//! star goes back to the cockpit, which sends the group off. The other
//! button (see `Alternate`) on a planet shows its moons.
//!
//! The moons view lists what's in orbit on the right (FUN_357b_18cc): your
//! groups and, when you can see them, the aliens' fleets, as HATTER4 icons.
//! Selecting one (FUN_18e0_00e8) offers MOVE UNITS for a group in orbit,
//! GROUND WAR for an army group at an alien planet you know well, and
//! ATTACK for an alien fleet when an army group is there; the other button
//! on a group opens its control panel.

use std::f64::consts::PI;

use bevy::prelude::*;
use bevy::sprite::Anchor;
use bevy::state::state::StateTransitionEvent;
use reunion_formats::state::Orbit;

use reunion_formats::aliens::{ALLIED, AT_WAR, MapEntry, MapObject};
use reunion_formats::state::{UnitList, unit};

use crate::control_panel::Destination;
use crate::space_battle::BattleStart;
use crate::focus::{Activated, AlternateUse, Hover, hotspot};
use crate::popup::ShowMessage;
use crate::game::Game;
use crate::game_data::{GameData, GameDataHandle};
use crate::hud::{ActionUsed, CONTENT_Y, ExtraActions, HoverLabel, IconSetOverride};
use crate::pic::MASKED;
use crate::planet_info::{napr_moon, napr_planet};
use crate::screen::{GameScreen, picture, place};
use crate::transition::GoTo;

pub struct GalacticMapPlugin;

impl Plugin for GalacticMapPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            OnEnter(GameScreen::GalacticMap),
            enter.after(crate::hud::spawn_hud),
        )
        .add_systems(
            Update,
            (spawn_view, spawn_objects, (turn, place_bodies).chain(), blink)
                .chain()
                .run_if(in_state(GameScreen::GalacticMap)),
        )
        .add_systems(OnExit(GameScreen::GalacticMap), |mut commands: Commands| {
            commands.remove_resource::<View>();
            commands.remove_resource::<Selected>();
        })
        .add_observer(open_body)
        .add_observer(show_moons)
        .add_observer(pick_system)
        .add_observer(zoom_out)
        .add_observer(select_object)
        .add_observer(open_group)
        .add_observer(object_action);
    }
}

const ZOOM_OUT: u8 = 33;
const ABORT_MOVE: u8 = 57;
/// Icon bar set while picking a destination.
const DESTINATION_SET: u8 = 18;
/// "SELECT DESTINATION" in HATTER4, in two halves, blinking at (10, 188).
const SELECT_DESTINATION: [(Rect, Vec2); 2] = [
    (
        Rect {
            min: Vec2::new(263.0, 26.0),
            max: Vec2::new(320.0, 32.0),
        },
        Vec2::new(10.0, 188.0),
    ),
    (
        Rect {
            min: Vec2::new(262.0, 32.0),
            max: Vec2::new(319.0, 38.0),
        },
        Vec2::new(67.0, 188.0),
    ),
];
/// The orbits' center in the 320x151 picture area.
const CENTER: Vec2 = Vec2::new(160.0, 75.0);
/// The original steps the orbits once per frame of its 70 Hz VGA mode.
const STEPS_PER_SECOND: f32 = 70.086;
/// Tenths of a degree in a full orbit.
const FULL_TURN: u16 = 3600;
/// Status icons in HATTER4: 7x7, 8 apart, from (1, 34).
const STATUS_ICON: Vec2 = Vec2::new(1.0, 34.0);
/// "UNKNOWN MOONS" in HATTER4, drawn at (270, 5) of the picture area.
const UNKNOWN_MOONS: Rect = Rect {
    min: Vec2::new(194.0, 18.0),
    max: Vec2::new(227.0, 27.0),
};

/// Which view is showing: the planets, or one planet's moons.
#[derive(Resource, Clone, Copy, PartialEq, Debug)]
struct View {
    system: u16,
    planet: u16,
}

/// Everything drawn for the current view.
#[derive(Component, Clone)]
struct ViewPart;

/// A planet or moon on its orbit.
#[derive(Component)]
struct Body {
    orbit: Orbit,
    /// Tenths of a degree.
    angle: u16,
    moon: bool,
    hotspot: Entity,
}

/// The hotspot of the body with this planet or moon number.
#[derive(Component)]
struct BodyHotspot(u16);

/// The star, when picking a destination: exploring its system.
#[derive(Component)]
struct StarHotspot;

/// A system button (1-based system) when picking a destination.
#[derive(Component)]
struct SystemButton(u8);

#[derive(Component)]
struct Blinking;

#[derive(Resource, Default)]
struct Steps(f32);

/// What's selected in the list of what's in orbit.
#[derive(Resource, Clone, Copy, PartialEq)]
struct Selected(MapObject);

/// The list of what's in orbit, and its entries.
#[derive(Component, Clone)]
struct ObjectPart;

#[derive(Component)]
struct ObjectSlot(MapObject);

#[derive(Component)]
struct SelectionFrame;

const ATTACK: u8 = 55;
const MOVE_UNITS: u8 = 56;
const GROUND_WAR: u8 = 71;
/// The list's places: two columns of 32x16 from (257, 1) of the picture.
fn slot_corner(i: usize) -> Vec2 {
    Vec2::new(257.0 + 32.0 * (i % 2) as f32, 1.0 + 16.0 * (i / 2) as f32 + CONTENT_Y)
}
/// Icon `icon` in HATTER4: 31x15 cells, ten to a row.
fn object_icon(icon: u8) -> Rect {
    let i = f32::from(icon.max(1) - 1);
    let min = Vec2::new((i % 10.0).floor() * 32.0 + 1.0, (i / 10.0).floor() * 16.0 + 1.0);
    Rect::from_corners(min, min + Vec2::new(31.0, 15.0))
}
/// The selection frame in HATTER4.
const SELECTION_FRAME: Rect = Rect {
    min: Vec2::new(161.0, 17.0),
    max: Vec2::new(192.0, 32.0),
};

fn enter(
    mut commands: Commands,
    mut transitions: MessageReader<StateTransitionEvent<GameScreen>>,
    game: Option<ResMut<Game>>,
) {
    let Some(mut game) = game else { return };
    // FUN_18e0_00e8: the planet stays selected only when coming from a
    // planet screen.
    let from = transitions.read().last().and_then(|t| t.exited);
    let (system, planet, moon) = game.selection();
    let planet = if matches!(
        from,
        Some(
            GameScreen::PlanetInfo
                | GameScreen::PlanetMain
                | GameScreen::CreateUnit
                | GameScreen::Group
                | GameScreen::SpaceBattle
        )
    ) {
        planet
    } else {
        0
    };
    game.select(system, planet, moon);
    commands.insert_resource(View { system, planet });
    commands.init_resource::<Steps>();
}

/// Rebuilds the picture whenever the view changes (and on entering).
fn spawn_view(
    mut commands: Commands,
    view: Option<Res<View>>,
    game: Option<Res<Game>>,
    handle: Res<GameDataHandle>,
    data: Res<Assets<GameData>>,
    asset_server: Res<AssetServer>,
    parts: Query<Entity, With<ViewPart>>,
    mut extra: ResMut<ExtraActions>,
    mut set: ResMut<IconSetOverride>,
    destination: Option<Res<Destination>>,
) {
    let (Some(view), Some(game), Some(data)) = (view, game, data.get(&handle.0)) else {
        return;
    };
    if !view.is_changed() {
        return;
    }
    let picking = destination.is_some();
    set.0 = picking.then_some(DESTINATION_SET);
    for part in &parts {
        commands.entity(part).despawn();
    }
    let scoped = (ViewPart, DespawnOnExit(GameScreen::GalacticMap));
    let state = &game.0;
    let system = view.system as usize;
    let Some(layout) = data.star_systems.get(system.wrapping_sub(1)) else {
        return;
    };
    let napr = asset_server.load::<Image>(format!("PLANETS/NAPR{system}.PIC#{MASKED}"));
    let hatter4 = asset_server.load::<Image>(format!("PLANETS/HATTER4.PIC#{MASKED}"));
    let sprite = |image: &Handle<Image>, rect: Rect| Sprite {
        image: image.clone(),
        rect: Some(rect),
        ..default()
    };
    let on_screen = |pos: Vec2| pos + Vec2::new(0.0, CONTENT_Y);
    let planets = layout.moons.len();

    // (orbit, sheet cell, index, moon) of every body to draw.
    let mut bodies = Vec::new();
    if view.planet == 0 {
        extra.0.retain(|&a| a != ZOOM_OUT);
        commands.spawn((
            picture(asset_server.load("PLANETS/HATTER1.PIC"), on_screen(Vec2::ZERO)),
            scoped.clone(),
        ));
        commands.spawn((
            sprite(&napr, Rect::new(0.0, 1.0, 64.0, 65.0)),
            Anchor::TOP_LEFT,
            place(on_screen(CENTER - 32.0), 0.5),
            scoped.clone(),
        ));
        if picking {
            let name = layout.name.trim_end().to_string();
            commands.spawn((
                StarHotspot,
                HoverLabel(name),
                hotspot(Rect::from_center_size(on_screen(CENTER), Vec2::splat(40.0)), Hover::Outline),
                scoped.clone(),
            ));
        }
        // FUN_357b_270b: nothing of an unexplored system, and no planets
        // the player doesn't know of.
        if state.system_known(system) {
            for planet in 1..=planets {
                if state.planet_visibility(system, planet) >= 0
                    && let Some(orbit) = state.orbit(system, planet)
                {
                    bodies.push((orbit, napr_planet(planet), planet, false));
                }
            }
        }
    } else {
        if !extra.0.contains(&ZOOM_OUT) {
            extra.0.push(ZOOM_OUT);
        }
        let planet = view.planet as usize;
        commands.spawn((
            picture(asset_server.load("PLANETS/HATTER2.PIC"), on_screen(Vec2::ZERO)),
            scoped.clone(),
        ));
        commands.spawn((
            sprite(&napr, napr_planet(planet)),
            Anchor::TOP_LEFT,
            place(on_screen(CENTER - 16.0), 0.5),
            scoped.clone(),
        ));
        if let Some(icon) = state.body(system, planet).and_then(status_icon) {
            commands.spawn((
                sprite(&hatter4, icon),
                Anchor::TOP_LEFT,
                place(on_screen(CENTER - 16.0), 0.6),
                scoped.clone(),
            ));
        }
        let visibility = state.planet_visibility(system, planet);
        if visibility == 0 {
            commands.spawn((
                sprite(&hatter4, UNKNOWN_MOONS),
                Anchor::TOP_LEFT,
                place(on_screen(Vec2::new(270.0, 5.0)), 0.5),
                scoped.clone(),
            ));
        }
        if visibility >= 1
            && let Some(moons) = layout.moons.get(planet - 1)
        {
            for (i, &body) in moons.iter().enumerate() {
                if let Some(orbit) = state.orbit(system, body as usize) {
                    bodies.push((orbit, napr_moon(body as usize - planets), i + 1, true));
                }
            }
        }
    }

    if picking {
        // FUN_357b_29cd: buttons of the known systems, from HATTER3.
        let hatter3 = asset_server.load::<Image>("PLANETS/HATTER3.PIC");
        for (i, s) in data.star_systems.iter().enumerate() {
            let known = state.byte(0x4815 + i as u16 + 1).is_some_and(|k| (k as i8) >= 0);
            if !known {
                continue;
            }
            let min = Vec2::new(((i % 4) * 64) as f32, ((i / 4) * 19) as f32);
            let at = on_screen(Vec2::new(256.0, 19.0 * i as f32));
            commands.spawn((
                sprite(&hatter3, Rect::from_corners(min, min + Vec2::new(64.0, 19.0))),
                Anchor::TOP_LEFT,
                place(at, 0.7),
                scoped.clone(),
            ));
            commands.spawn((
                SystemButton(i as u8 + 1),
                HoverLabel(s.name.trim_end().to_string()),
                hotspot(Rect::from_corners(at, at + Vec2::new(64.0, 19.0)), Hover::Outline),
                scoped.clone(),
            ));
        }
        for (rect, at) in SELECT_DESTINATION {
            commands.spawn((
                Blinking,
                sprite(&hatter4, rect),
                Anchor::TOP_LEFT,
                place(at, 0.7),
                scoped.clone(),
            ));
        }
    }

    let names = state
        .star_systems()
        .into_iter()
        .nth(system - 1)
        .map(|s| s.bodies.into_iter().map(|b| b.name).collect::<Vec<_>>())
        .unwrap_or_default();
    for (orbit, cell, index, moon) in bodies {
        let body_number = if moon {
            layout.moons[view.planet as usize - 1][index - 1] as usize
        } else {
            index
        };
        let name = names.get(body_number - 1).cloned().unwrap_or_default();
        let hotspot = commands
            .spawn((
                BodyHotspot(index as u16),
                HoverLabel(name.trim_end().to_string()),
                hotspot(Rect::from_center_size(CENTER, Vec2::splat(16.0)), Hover::Outline),
                scoped.clone(),
            ))
            .id();
        let mut body = commands.spawn((
            Body {
                orbit,
                angle: random_angle(),
                moon,
                hotspot,
            },
            sprite(&napr, cell),
            Anchor::TOP_LEFT,
            place(on_screen(CENTER), 0.2),
            scoped.clone(),
        ));
        if moon
            && let Some(icon) = state.body(system, body_number).and_then(status_icon)
        {
            // 3 pixels up and left of the moon, over it.
            body.with_child((
                sprite(&hatter4, icon),
                Anchor::TOP_LEFT,
                Transform::from_xyz(-3.0, 3.0, 0.05),
            ));
        }
    }
}

/// FUN_357b_18cc: which status icon a body gets: the player's (2) on your
/// own; a surveyed alien planet shows its owner (owner + 1) or, less
/// surveyed, a question mark (1).
fn status_icon(record: &[u8]) -> Option<Rect> {
    let owner = record[0];
    let survey = record[0x0c] as i8;
    let icon = match owner {
        1 => 2,
        o if o > 1 && survey > 39 => o + 1,
        o if o > 1 && survey > 29 => 1,
        _ => return None,
    };
    let min = STATUS_ICON + Vec2::new(8.0 * (icon - 1) as f32, 0.0);
    Some(Rect::from_corners(min, min + 7.0))
}

/// The original starts every body at Random(3600).
fn random_angle() -> u16 {
    use std::hash::{BuildHasher, RandomState};
    (RandomState::new().hash_one(0u8) % u64::from(FULL_TURN)) as u16
}

/// FUN_357b_3010: planets turn speed / 2, moons speed / 4 tenths of a degree
/// per step.
fn turn(time: Res<Time>, mut steps: ResMut<Steps>, mut bodies: Query<&mut Body>) {
    steps.0 += time.delta_secs() * STEPS_PER_SECOND;
    let n = steps.0.floor();
    steps.0 -= n;
    for _ in 0..n as u32 {
        for mut body in &mut bodies {
            let step = u16::from(body.orbit.speed) >> if body.moon { 2 } else { 1 };
            body.angle = (body.angle + step) % FULL_TURN;
        }
    }
}

/// The trig table at DS:0x7fd4 (FUN_3e17_18db): cos in whole degrees, x 500.
fn cos500(degrees: u16) -> f64 {
    ((f64::from(degrees % 360) * PI / 180.0).cos() * 500.0).round()
}

/// Where a body is drawn: its top-left corner in the picture area, its size,
/// and whether it's behind the star or planet in the middle.
fn position(orbit: Orbit, angle: u16, moon: bool) -> (Vec2, f32, bool) {
    let deg = angle / 10;
    let x = CENTER.x + (f64::from(orbit.radius_x) * cos500(deg) / 500.0).round() as f32;
    let depth = cos500(deg + 270);
    let y = CENTER.y + (f64::from(orbit.radius_y) * depth / 500.0).round() as f32;
    let nearness = ((depth * 67.0 / 500.0).round() + 67.0) as u16;
    let size = if moon {
        nearness / 16 + 8
    } else {
        nearness / 8 + 16
    };
    let half = (size / 2) as f32;
    let behind = if moon { y + 8.0 < CENTER.y } else { y < CENTER.y };
    (Vec2::new(x - half, y - half), f32::from(size), behind)
}

fn place_bodies(
    mut bodies: Query<(&Body, &mut Sprite, &mut Transform)>,
    mut hotspots: Query<&mut crate::focus::Hotspot>,
) {
    for (body, mut sprite, mut transform) in &mut bodies {
        let (corner, size, behind) = position(body.orbit, body.angle, body.moon);
        let screen = corner + Vec2::new(0.0, CONTENT_Y);
        // Nearer bodies over farther ones; behind the middle one at z 0.5.
        let z = if behind { 0.1 } else { 0.6 } + corner.y / 1000.0;
        *transform = place(screen, z);
        sprite.custom_size = Some(Vec2::splat(size));
        // Hotspots: the body itself for planets, 16x16 for moons.
        let area = if body.moon { 16.0 } else { size };
        let rect = Rect::from_center_size(screen + size / 2.0, Vec2::splat(area));
        if let Ok(mut hotspot) = hotspots.get_mut(body.hotspot)
            && hotspot.rect != rect
        {
            hotspot.rect = rect;
        }
    }
}

/// FUN_18e0_00e8: a planet or moon hotspot opens PLANET INFO for it.
fn open_body(
    activated: On<Activated>,
    hotspots: Query<&BodyHotspot>,
    destination: Option<ResMut<Destination>>,
    view: Option<Res<View>>,
    game: Option<ResMut<Game>>,
    mut commands: Commands,
) {
    let (Ok(BodyHotspot(index)), Some(view), Some(mut game)) =
        (hotspots.get(activated.0), view, game)
    else {
        return;
    };
    if let Some(mut destination) = destination {
        let (s, p) = (view.system as u8, view.planet as u8);
        destination.chosen = Some(if p == 0 { (s, *index as u8, 0) } else { (s, p, *index as u8) });
        commands.trigger(GoTo(GameScreen::ControlPanel));
        return;
    }
    if view.planet == 0 {
        game.select(view.system, *index, 0);
    } else {
        game.select(view.system, view.planet, *index);
    }
    commands.trigger(GoTo(GameScreen::PlanetInfo));
}

/// Picking a destination: the other button on a planet shows its moons;
/// the star is the system itself.
fn show_moons(
    used: On<AlternateUse>,
    hotspots: Query<&BodyHotspot>,
    destination: Option<Res<Destination>>,
    view: Option<ResMut<View>>,
    game: Option<ResMut<Game>>,
) {
    let (Ok(BodyHotspot(index)), Some(_), Some(mut view), Some(mut game)) =
        (hotspots.get(used.0), destination, view, game)
    else {
        return;
    };
    if view.planet == 0 {
        game.select(view.system, *index, 0);
        view.planet = *index;
    }
}

/// Picking a destination: the star explores its system; system buttons
/// switch systems if the group can get there (FUN_18e0_07ec).
fn pick_system(
    activated: On<Activated>,
    stars: Query<(), With<StarHotspot>>,
    buttons: Query<&SystemButton>,
    destination: Option<ResMut<Destination>>,
    view: Option<ResMut<View>>,
    game: Option<ResMut<Game>>,
    mut commands: Commands,
) {
    let (Some(mut destination), Some(mut view), Some(mut game)) = (destination, view, game) else {
        return;
    };
    if stars.contains(activated.0) {
        destination.chosen = Some((view.system as u8, 0, 0));
        commands.trigger(GoTo(GameScreen::ControlPanel));
        return;
    }
    let Ok(&SystemButton(system)) = buttons.get(activated.0) else {
        return;
    };
    if u16::from(system) == view.system {
        return;
    }
    let Some(group) = game.0.unit(UnitList::Groups, destination.unit).map(<[u8]>::to_vec) else {
        return;
    };
    let word = |at: usize| i16::from_le_bytes([group[at], group[at + 1]]);
    // Hyperdrives: destroyers or cruisers (warships), galleons (merchant
    // ships), or the hyperspace drive for satellite carriers.
    let hyperspace = game.0.word(0x606e) == Some(5) || game.0.word(0x60a3) == Some(5);
    let has_hyperdrive = match group[unit::TYPE] {
        1 | 3 => word(0x31) > 0 || word(0x3b) > 0,
        2 => word(0x3b) > 0,
        4 => hyperspace,
        _ => false,
    };
    if !has_hyperdrive {
        commands.trigger(ShowMessage::new(" No hyperdrive in this unit! "));
        return;
    }
    // The pilot you hired decides how far groups can go.
    let pilot = game.0.word(0x95b6).unwrap_or(0);
    let near = view.system < 4 && system < 4;
    if !(pilot == 3 || (pilot == 2 && near) || group[unit::TYPE] == 4) {
        commands.trigger(ShowMessage::new(
            " Your pilot is unable to undertake |        such a long voyage!",
        ));
        return;
    }
    game.select(system.into(), 0, 0);
    *view = View {
        system: system.into(),
        planet: 0,
    };
}

/// "SELECT DESTINATION" blinks (FUN_357b_29cd: on 13 of every 21 frames).
fn blink(time: Res<Time>, mut parts: Query<&mut Visibility, With<Blinking>>) {
    let frame = (time.elapsed_secs() * 70.086) as u32 % 21;
    for mut visibility in &mut parts {
        visibility.set_if_neq(if frame > 7 {
            Visibility::Inherited
        } else {
            Visibility::Hidden
        });
    }
}

/// ZOOM OUT: from the moons back to the planets.
fn zoom_out(
    action: On<ActionUsed>,
    screen: Res<State<GameScreen>>,
    view: Option<ResMut<View>>,
    game: Option<ResMut<Game>>,
    mut commands: Commands,
) {
    if *screen.get() != GameScreen::GalacticMap {
        return;
    }
    if action.0 == ABORT_MOVE {
        commands.trigger(GoTo(GameScreen::ControlPanel));
        return;
    }
    if action.0 != ZOOM_OUT {
        return;
    }
    let (Some(mut view), Some(mut game)) = (view, game) else {
        return;
    };
    game.select(view.system, 0, 0);
    view.planet = 0;
}

/// Keeps the list of what's in orbit up to date in the moons view.
#[allow(clippy::too_many_arguments)]
fn spawn_objects(
    mut commands: Commands,
    view: Option<Res<View>>,
    game: Option<Res<Game>>,
    handle: Res<GameDataHandle>,
    data: Res<Assets<GameData>>,
    asset_server: Res<AssetServer>,
    parts: Query<Entity, With<ObjectPart>>,
    destination: Option<Res<Destination>>,
    selected: Option<Res<Selected>>,
    mut frames: Query<(&mut Transform, &mut Visibility), With<SelectionFrame>>,
    mut extra: ResMut<ExtraActions>,
    mut shown: Local<Option<(u16, u16, Vec<Option<MapEntry>>)>>,
) {
    let (Some(view), Some(game), Some(data)) = (view, game, data.get(&handle.0)) else {
        return;
    };
    let list = if view.planet == 0 || destination.is_some() {
        Vec::new()
    } else {
        game.0.map_objects(&data.exe, view.system as u8, view.planet as u8)
    };
    let key = (view.system, view.planet, list.clone());
    if shown.as_ref() != Some(&key) || parts.is_empty() {
        *shown = Some(key);
        for part in &parts {
            commands.entity(part).despawn();
        }
        let scoped = (ObjectPart, DespawnOnExit(GameScreen::GalacticMap));
        let hatter4 = asset_server.load::<Image>(format!("PLANETS/HATTER4.PIC#{MASKED}"));
        commands.spawn((
            SelectionFrame,
            Sprite { image: hatter4.clone(), rect: Some(SELECTION_FRAME), ..default() },
            Anchor::TOP_LEFT,
            place(Vec2::ZERO, 0.8),
            Visibility::Hidden,
            scoped.clone(),
        ));
        for (i, entry) in list.iter().enumerate() {
            let Some(entry) = entry else { continue };
            let name = match entry.object {
                MapObject::Group(n) => game.0.unit(UnitList::Groups, n).map(crate::ship_info::unit_name).unwrap_or_default(),
                MapObject::Fleet(id) => game.0.fleet_name(&data.exe, id),
            };
            commands.spawn((
                Sprite { image: hatter4.clone(), rect: Some(object_icon(entry.icon)), ..default() },
                Anchor::TOP_LEFT,
                place(slot_corner(i), 0.7),
                scoped.clone(),
            ));
            commands.spawn((
                ObjectSlot(entry.object),
                HoverLabel(name),
                hotspot(Rect::from_corners(slot_corner(i), slot_corner(i) + Vec2::new(32.0, 16.0)), Hover::Outline),
                scoped.clone(),
            ));
        }
        // A selection that's gone from the list is dropped.
        if let Some(selected) = &selected
            && !list.iter().flatten().any(|e| e.object == selected.0)
        {
            commands.remove_resource::<Selected>();
        }
    }
    // The frame and the icon bar follow the selection.
    let at = selected.as_ref().and_then(|s| list.iter().position(|e| e.map(|e| e.object) == Some(s.0)));
    for (mut transform, mut visibility) in &mut frames {
        match at {
            Some(i) => {
                *transform = place(slot_corner(i), 0.8);
                visibility.set_if_neq(Visibility::Inherited);
            }
            None => {
                visibility.set_if_neq(Visibility::Hidden);
            }
        }
    }
    let mut actions: Vec<u8> = extra.0.iter().copied().filter(|a| ![ATTACK, MOVE_UNITS, GROUND_WAR].contains(a)).collect();
    if let Some(selected) = &selected
        && at.is_some()
    {
        let armies = list.iter().flatten().any(|e| match e.object {
            MapObject::Group(n) => game.0.unit(UnitList::Groups, n).is_some_and(|g| g[unit::TYPE] == 1),
            MapObject::Fleet(_) => false,
        });
        match selected.0 {
            MapObject::Fleet(id) => {
                if armies && game.0.standing(id.race) < ALLIED {
                    actions.push(ATTACK);
                }
            }
            MapObject::Group(n) => {
                actions.push(MOVE_UNITS);
                if let Some(g) = game.0.unit(UnitList::Groups, n) {
                    let place = (g[unit::SYSTEM], g[unit::PLANET], g[unit::MOON]);
                    let body = game.body_number(&data.star_systems, place);
                    let record = body.and_then(|b| game.0.body(place.0.into(), b));
                    if g[unit::TYPE] == 1
                        && record.is_some_and(|r| r[0] > 1 && (r[0x0c] as i8) > 39 && game.0.standing(r[0]) < ALLIED)
                    {
                        actions.push(GROUND_WAR);
                    }
                }
            }
        }
    }
    if extra.0 != actions {
        extra.0 = actions;
    }
}

/// Selecting something in the list.
fn select_object(activated: On<Activated>, slots: Query<&ObjectSlot>, mut commands: Commands) {
    if let Ok(slot) = slots.get(activated.0) {
        commands.insert_resource(Selected(slot.0));
    }
}

/// The other button on a group: its control panel.
fn open_group(used: On<AlternateUse>, slots: Query<&ObjectSlot>, game: Option<ResMut<Game>>, mut commands: Commands) {
    let (Ok(ObjectSlot(MapObject::Group(n))), Some(mut game)) = (slots.get(used.0), game) else {
        return;
    };
    for (at, value) in [(0xa2ce, 0), (0xa2c8, *n as u16), (0xa2ca, *n as u16)] {
        game.0.set_word(at, value);
    }
    let count = game.0.word(0xa2c4).unwrap_or(0);
    game.0.set_word(0xa2c2, count);
    commands.trigger(GoTo(GameScreen::ControlPanel));
}

/// ATTACK, GROUND WAR and MOVE UNITS (FUN_18e0_00e8).
fn object_action(
    action: On<ActionUsed>,
    screen: Res<State<GameScreen>>,
    selected: Option<Res<Selected>>,
    view: Option<Res<View>>,
    game: Option<ResMut<Game>>,
    handle: Res<GameDataHandle>,
    data: Res<Assets<GameData>>,
    mut commands: Commands,
) {
    if *screen.get() != GameScreen::GalacticMap || ![ATTACK, MOVE_UNITS, GROUND_WAR].contains(&action.0) {
        return;
    }
    let (Some(selected), Some(view), Some(mut game), Some(data)) = (selected, view, game, data.get(&handle.0)) else {
        return;
    };
    let say = |commands: &mut Commands, text: &str| commands.trigger(ShowMessage::new(text));
    let fighter = game.0.word(0x95a8).unwrap_or(0);
    match (action.0, selected.0) {
        (ATTACK, MapObject::Fleet(id)) => {
            if fighter == 0 {
                say(&mut commands, " You haven't got a fighter! ");
                return;
            }
            game.0.set_standing(id.race, AT_WAR);
            commands.insert_resource(BattleStart {
                place: (view.system as u8, view.planet as u8, 0),
                you_attack: true,
                ground: false,
            });
            commands.trigger(GoTo(GameScreen::SpaceBattle));
        }
        (GROUND_WAR, MapObject::Group(n)) => {
            if fighter == 0 {
                say(&mut commands, " You haven't got a battle advisor! ");
                return;
            }
            if game.0.word(0x95ba).unwrap_or(0) < 2 {
                say(&mut commands, " Your fighter not skilled to lead a ground battle ");
                return;
            }
            let Some(g) = game.0.unit(UnitList::Groups, n).map(<[u8]>::to_vec) else { return };
            let place = (g[unit::SYSTEM], g[unit::PLANET], g[unit::MOON]);
            let owner = game
                .body_number(&data.star_systems, place)
                .and_then(|b| game.0.body(place.0.into(), b))
                .map_or(0, |r| r[0]);
            if owner > 1 {
                game.0.set_standing(owner, AT_WAR);
            }
            commands.insert_resource(BattleStart { place, you_attack: true, ground: true });
            commands.trigger(GoTo(GameScreen::SpaceBattle));
        }
        (MOVE_UNITS, MapObject::Group(n)) => {
            let Some(g) = game.0.unit(UnitList::Groups, n).map(<[u8]>::to_vec) else { return };
            if game.0.word(0x95a4).unwrap_or(0) == 0 && g[unit::TYPE] != 4 {
                say(&mut commands, " You haven't hired a pilot! ");
            } else if crate::control_panel::no_ships(data, &g) {
                say(&mut commands, " No ships in this unit! ");
            } else if g[unit::STATUS] != 2 {
                say(&mut commands, " The ships are on ground! ");
            } else {
                commands.insert_resource(Destination { unit: n, chosen: None });
                game.select(view.system, 0, 0);
                commands.insert_resource(View { system: view.system, planet: 0 });
            }
        }
        _ => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const ORBIT: Orbit = Orbit {
        speed: 10,
        radius_x: 100,
        radius_y: 40,
    };

    #[test]
    fn nearest_point_is_below_the_star_at_full_size() {
        // 90 degrees: x at the center, y at the bottom of the ellipse.
        let (corner, size, behind) = position(ORBIT, 900, false);
        assert_eq!(size, 32.0);
        assert_eq!(corner, Vec2::new(160.0 - 16.0, 75.0 + 40.0 - 16.0));
        assert!(!behind);
    }

    #[test]
    fn farthest_point_is_above_and_behind_at_half_size() {
        let (corner, size, behind) = position(ORBIT, 2700, false);
        assert_eq!(size, 16.0);
        assert_eq!(corner, Vec2::new(160.0 - 8.0, 75.0 - 40.0 - 8.0));
        assert!(behind);
    }

    #[test]
    fn moons_are_half_as_big() {
        assert_eq!(position(ORBIT, 900, true).1, 16.0);
        assert_eq!(position(ORBIT, 2700, true).1, 8.0);
    }
}


