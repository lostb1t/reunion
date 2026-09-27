//! CONTROL PANEL (screen 17): the selected group's cockpit.
//!
//! From REUNION.PRG FUN_22ca_0105 (logic), FUN_22ca_08d4 / 09f9 (pictures)
//! and FUN_22ca_065e (hotspots): PLANETS/MUSZI at row 49 with the view
//! through its window (its yellow): the planet's surface (PLANETS/NAGY<type>
//! from row 43) while docked, space (NAGY0) otherwise.
//!
//! - the window: PLANET MAIN of the planet it's at (docked or in orbit);
//! - the lever: launch into orbit / dock on the planet, unless the planet is
//!   gaseous, a satellite carrier would dock away from New Earth, or the
//!   group has no ships;
//! - the middle: move (pick a destination on the galactic map), if a pilot
//!   is hired and there are ships;
//! - TRANSFER (trade and secret forces), SHIPS and GROUP.
//!
//! Not yet: the cockpit's animations and the story events of docking at
//! certain planets.


use bevy::prelude::*;
use bevy::sprite::Anchor;
use reunion_formats::state::{UnitList, unit};

use crate::focus::{Activated, Hover, hotspot};
use crate::game::{Game, random};
use crate::game_data::{GameData, GameDataHandle};
use crate::hud::{CONTENT_Y, HoverLabel};
use crate::popup::ShowMessage;
use crate::screen::{GameScreen, place};
use crate::ship_info::SELECTED;
use crate::transition::GoTo;

pub struct ControlPanelPlugin;

impl Plugin for ControlPanelPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(OnEnter(GameScreen::ControlPanel), enter.after(crate::hud::spawn_hud))
            .add_systems(
                Update,
                (spawn_view, key_window).run_if(in_state(GameScreen::ControlPanel)),
            )
            .add_observer(activate);
    }
}

/// Status of a group: docked, in orbit, travelling.
const DOCKED: u8 = 1;
const IN_ORBIT: u8 = 2;
/// Planet type of gas giants (record byte 0x15).
const GASEOUS: u8 = 2;

/// A destination picked on the galactic map for the group being moved
/// (the original's DS:0x787c / the map's return to screen 17).
#[derive(Resource, Clone, Copy)]
pub struct Destination {
    pub unit: usize,
    /// Set when a planet or moon was picked; None when the move was aborted.
    pub chosen: Option<(u8, u8, u8)>,
}

#[derive(Component, Clone)]
struct ViewPart;

#[derive(Component, Clone, Copy, PartialEq)]
enum Control {
    Window,
    Lever,
    Transfer,
    Move,
    Ships,
    Group,
}

/// MUSZI with its window color see-through, made once it's loaded.
#[derive(Component)]
struct Cockpit(Handle<Image>);

fn selected(game: &Game) -> usize {
    game.0.word(SELECTED).unwrap_or(0) as usize
}

fn enter(
    mut commands: Commands,
    destination: Option<Res<Destination>>,
    game: Option<ResMut<Game>>,
) {
    let Some(mut game) = game else { return };
    let n = selected(&game);
    // Back from the galactic map with a destination: off we go.
    if let Some(d) = destination.as_deref().copied() {
        commands.remove_resource::<Destination>();
        if let (Some(place), true) = (d.chosen, d.unit == n) {
            game.0.start_travel(n, place, random);
        }
    }
    // The planet screens look at where the group is.
    if let Some(group) = game.0.unit(UnitList::Groups, n)
        && matches!(group[unit::STATUS], DOCKED | IN_ORBIT)
    {
        let (s, p, m) = (group[unit::SYSTEM], group[unit::PLANET], group[unit::MOON]);
        game.select(s.into(), p.into(), m.into());
    }
}

/// The planet record where a group is (FUN_357b_3253).
fn planet_record(game: &Game, data: &GameData, group: &[u8]) -> Option<Vec<u8>> {
    let (s, p, m) = (group[unit::SYSTEM], group[unit::PLANET], group[unit::MOON]);
    let body = if m == 0 {
        p as usize
    } else {
        *data
            .star_systems
            .get((s as usize).checked_sub(1)?)?
            .moons
            .get((p as usize).checked_sub(1)?)?
            .get(m as usize - 1)? as usize
    };
    game.0.body(s as usize, body).map(<[u8]>::to_vec)
}

/// FUN_2b8d_1abb: true when the group has no ships (DS:0xbe5 / 0xbe9: the
/// record slots to look in, 0xbed: how many kinds).
pub fn no_ships(data: &GameData, group: &[u8]) -> bool {
    let t = u16::from(group[unit::TYPE]);
    let byte = |at: u16| data.exe.ds_bytes(at, 1).map_or(0, |b| b[0]) as usize;
    let kinds = byte(0xbed + t);
    [byte(0xbe5 + t), byte(0xbe9 + t)].into_iter().all(|slot| {
        (0..kinds).all(|i| {
            let at = unit::SLOTS + (slot.max(1) - 1) * unit::SLOT_LEN + i * unit::KIND_LEN;
            group.get(at..at + 2).is_none_or(|w| w == [0, 0])
        })
    })
}

fn spawn_view(
    mut commands: Commands,
    game: Option<Res<Game>>,
    handle: Res<GameDataHandle>,
    data: Res<Assets<GameData>>,
    asset_server: Res<AssetServer>,
    parts: Query<Entity, With<ViewPart>>,
    mut shown: Local<Option<(usize, Vec<u8>)>>,
) {
    let (Some(game), Some(data)) = (game, data.get(&handle.0)) else {
        return;
    };
    let n = selected(&game);
    let Some(group) = game.0.unit(UnitList::Groups, n).map(<[u8]>::to_vec) else {
        return;
    };
    // Redraw when the group changes (docking, launching, leaving).
    let key = (n, group[unit::STATUS..=unit::STATUS].to_vec());
    if shown.as_ref() == Some(&key) && !parts.is_empty() {
        return;
    }
    *shown = Some(key);
    for part in &parts {
        commands.entity(part).despawn();
    }
    let scoped = (ViewPart, DespawnOnExit(GameScreen::ControlPanel));
    let status = group[unit::STATUS];
    // The view: the planet's surface while docked, space otherwise.
    let (view, rect) = if status == DOCKED {
        let kind = planet_record(&game, data, &group).map_or(0, |r| r[0x15]);
        (format!("PLANETS/NAGY{kind}.PIC"), Rect::new(0.0, 43.0, 320.0, 151.0))
    } else {
        ("PLANETS/NAGY0.PIC".to_string(), Rect::new(0.0, 0.0, 320.0, 151.0))
    };
    commands.spawn((
        Sprite {
            image: asset_server.load(view),
            rect: Some(rect),
            ..default()
        },
        Anchor::TOP_LEFT,
        place(Vec2::new(0.0, CONTENT_Y), 0.0),
        scoped.clone(),
    ));
    commands.spawn((
        Cockpit(asset_server.load("PLANETS/MUSZI.PIC")),
        Sprite::default(),
        Anchor::TOP_LEFT,
        place(Vec2::new(0.0, CONTENT_Y), 0.1),
        // Until the window is cut out.
        Visibility::Hidden,
        scoped.clone(),
    ));

    let lever = match status {
        DOCKED => "Launch",
        IN_ORBIT => "Docking",
        _ => "No effect",
    };
    let middle = match status {
        4..=6 => "Change destination",
        IN_ORBIT => "Move",
        _ => "No effect",
    };
    let controls = [
        (Control::Window, Rect::new(0.0, 49.0, 320.0, 136.0), "Planet main", status <= IN_ORBIT),
        (Control::Lever, Rect::new(0.0, 149.0, 71.0, 200.0), lever, true),
        (
            Control::Transfer,
            Rect::new(77.0, 141.0, 119.0, 181.0),
            "Transfer",
            matches!(group[unit::TYPE], 2 | 3),
        ),
        (Control::Move, Rect::new(131.0, 137.0, 190.0, 200.0), middle, true),
        (Control::Ships, Rect::new(202.0, 145.0, 243.0, 184.0), "Ships", true),
        (Control::Group, Rect::new(255.0, 153.0, 320.0, 200.0), "Group", true),
    ];
    for (control, rect, name, enabled) in controls {
        if enabled {
            commands.spawn((
                control,
                HoverLabel(name.into()),
                hotspot(rect, Hover::Outline),
                scoped.clone(),
            ));
        }
    }
}

/// Makes MUSZI's window (yellow) see-through once the picture is loaded.
fn key_window(
    mut cockpits: Query<(&Cockpit, &mut Sprite, &mut Visibility)>,
    mut images: ResMut<Assets<Image>>,
) {
    for (cockpit, mut sprite, mut visibility) in &mut cockpits {
        if *visibility != Visibility::Hidden {
            continue;
        }
        let Some(source) = images.get(&cockpit.0) else { continue };
        let Some(data) = source.data.clone() else { continue };
        let mut keyed = data;
        for px in keyed.chunks_exact_mut(4) {
            if px[..3] == [255, 255, 0] {
                px[3] = 0;
            }
        }
        let size = source.texture_descriptor.size;
        sprite.image = images.add(crate::pic::rgba_image(size.width, size.height, keyed));
        *visibility = Visibility::Inherited;
    }
}

fn activate(
    activated: On<Activated>,
    controls: Query<&Control>,
    game: Option<ResMut<Game>>,
    handle: Res<GameDataHandle>,
    data: Res<Assets<GameData>>,
    mut commands: Commands,
) {
    let (Ok(&control), Some(mut game), Some(data)) =
        (controls.get(activated.0), game, data.get(&handle.0))
    else {
        return;
    };
    let n = selected(&game);
    let Some(group) = game.0.unit(UnitList::Groups, n).map(<[u8]>::to_vec) else {
        return;
    };
    let status = group[unit::STATUS];
    let say = |commands: &mut Commands, text: &str| commands.trigger(ShowMessage::new(text));
    match control {
        Control::Window => commands.trigger(GoTo(GameScreen::PlanetMain)),
        Control::Lever if matches!(status, DOCKED | IN_ORBIT) => {
            let planet = planet_record(&game, data, &group);
            let at_new_earth = (group[unit::SYSTEM], group[unit::PLANET], group[unit::MOON]) == (1, 5, 0);
            if planet.as_ref().is_some_and(|p| p[0x15] == GASEOUS) {
                say(&mut commands, " You cannot land on this type of planet, |          since it is gaseous!");
            } else if group[unit::TYPE] == 4 && !at_new_earth {
                say(&mut commands, " The satellite carriers can only land |          on your main planet");
            } else if no_ships(data, &group) {
                say(&mut commands, " No ships in this unit ! ");
            } else {
                // Landing surveys the planet a little.
                let (s, p, m) = (group[unit::SYSTEM], group[unit::PLANET], group[unit::MOON]);
                game.select(s.into(), p.into(), m.into());
                if let Some(body) = game.selected_body(&data.star_systems)
                    && let Some(record) = game.0.planet_mut(s as usize, body)
                    && (record[0x0c] as i8) < 6
                {
                    record[0x0c] = 6;
                }
                if let Some(g) = game.0.unit_mut(UnitList::Groups, n) {
                    g[unit::STATUS] = 3 - status;
                }
            }
        }
        Control::Transfer => commands.trigger(GoTo(GameScreen::Transfer)),
        Control::Move if status > DOCKED => {
            let pilots = game.0.word(0x95a4).unwrap_or(0);
            if pilots == 0 && group[unit::TYPE] != 4 {
                say(&mut commands, " You haven't got a pilot! ");
            } else if no_ships(data, &group) {
                say(&mut commands, " No ships in this unit! ");
            } else {
                // The galactic map of the group's system, to pick where to go.
                let s = group[unit::SYSTEM];
                game.select(s.into(), 0, 0);
                commands.insert_resource(Destination { unit: n, chosen: None });
                commands.trigger(GoTo(GameScreen::GalacticMap));
            }
        }
        Control::Ships => commands.trigger(GoTo(GameScreen::ShipInfo)),
        Control::Group => commands.trigger(GoTo(GameScreen::Group)),
        _ => {}
    }
}
