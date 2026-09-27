//! PLANET INFO (screen 8): what is known about the selected planet.
//!
//! From REUNION.PRG FUN_1413_000a, FUN_357b_10d9 and FUN_357b_0359 (the text
//! logic was read from the machine code; the decompiler drops its calls):
//! BOLYGO.PIC (the screen table's background for screen 8) at row 49, the owner's race picture (FAJ<n>) at (2, 51), the
//! planet picture (PLANET<type>) at (2, 147), the planet's icon from its
//! system's NAPR<n> at (13, 105), and yellow text lines. How much is shown
//! depends on the planet's survey level (record byte 0x0c).
//!
//! On your own populated planet the icon bar gains INCREASE / DECREASE TAX,
//! which change the tax level (byte 0x12, 0-7).

use bevy::prelude::*;
use bevy::sprite::Anchor;

use reunion_formats::deploy;

use crate::game::{Game, random};
use crate::popup::ShowMessage;
use crate::transition::GoTo;
use crate::game_data::{GameData, GameDataHandle};
use crate::hud::{ActionUsed, CONTENT_Y, ExtraActions, YELLOW_TEXT};
use crate::pic::MASKED;
use crate::screen::{GameScreen, picture, place};

pub struct PlanetInfoPlugin;

impl Plugin for PlanetInfoPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            OnEnter(GameScreen::PlanetInfo),
            enter.after(crate::hud::spawn_hud),
        )
        .add_systems(
            Update,
            (update_texts, deploy_icons).run_if(in_state(GameScreen::PlanetInfo)),
        )
        .add_observer(change_tax)
        .add_observer(deploy);
    }
}

const INCREASE_TAX: u8 = 38;
const DECREASE_TAX: u8 = 39;

const MINERALS: usize = 6;
/// Record bytes: owner, mineral flag, population flag, colonists, satellites,
/// spy ship, miner droids, solar sats, survey level, population (u32),
/// tech level, tax, attitude, type, diameter (u16), temperature (i16), minerals.
mod field {
    pub const OWNER: usize = 0x00;
    pub const HAS_LIFE: usize = 0x02;
    pub const MINERALS_KNOWN: usize = 0x03;
    pub const COLONY: usize = 0x06;
    pub const COLONISTS: usize = 0x07;
    pub const SATELLITE: usize = 0x08;
    pub const SPY_SHIP: usize = 0x09;
    pub const MINERS: usize = 0x0a;
    pub const SOLAR_SATS: usize = 0x0b;
    pub const SURVEY: usize = 0x0c;
    pub const POPULATION: usize = 0x0d;
    pub const TECH: usize = 0x11;
    pub const TAX: usize = 0x12;
    pub const ATTITUDE: usize = 0x13;
    pub const TYPE: usize = 0x15;
    pub const DIAMETER: usize = 0x17;
    pub const TEMPERATURE: usize = 0x19;
    pub const MINERAL_AMOUNTS: usize = 0x3b;
}

const ENERGY_SHIELD_BUILDING: u8 = 0x14;

/// One line of text and its width in characters.
#[derive(Component)]
struct InfoText {
    columns: usize,
    line: Line,
}

#[derive(Clone, Copy, PartialEq)]
enum Line {
    Name,
    Population,
    Tax,
    Tech,
    Type,
    Mineral(usize),
    Diameter,
    Temperature,
    Object(usize),
}

fn text_lines() -> Vec<(Vec2, usize, Line)> {
    let mut lines = vec![
        (Vec2::new(94.0, 53.0), 35, Line::Name),
        (Vec2::new(97.0, 69.0), 36, Line::Population),
        (Vec2::new(97.0, 85.0), 35, Line::Tax),
        (Vec2::new(97.0, 101.0), 35, Line::Tech),
        (Vec2::new(97.0, 117.0), 36, Line::Type),
        (Vec2::new(124.0, 133.0), 6, Line::Diameter),
        (Vec2::new(272.0, 133.0), 4, Line::Temperature),
    ];
    for i in 0..MINERALS {
        lines.push((
            Vec2::new(160.0, 149.0 + 8.0 * i as f32),
            3,
            Line::Mineral(i),
        ));
    }
    for i in 0..6 {
        lines.push((
            Vec2::new(194.0, 149.0 + 8.0 * i as f32),
            20,
            Line::Object(i),
        ));
    }
    lines
}

fn enter(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    game: Option<Res<Game>>,
    handle: Res<GameDataHandle>,
    data: Res<Assets<GameData>>,
    mut extra: ResMut<ExtraActions>,
) {
    let scoped = DespawnOnExit(GameScreen::PlanetInfo);
    commands.spawn((
        picture(
            asset_server.load("GRAFIKA/BOLYGO.PIC"),
            Vec2::new(0.0, CONTENT_Y),
        ),
        scoped.clone(),
    ));
    for (pos, columns, line) in text_lines() {
        commands.spawn((
            InfoText { columns, line },
            Sprite::default(),
            Anchor::TOP_LEFT,
            place(pos, 1.0),
            scoped.clone(),
        ));
    }
    let (Some(game), Some(data)) = (game, data.get(&handle.0)) else {
        return;
    };
    let (system, planet, moon) = game.selection();
    let Some(body) = game.selected_body(&data.star_systems) else {
        return;
    };
    let Some(record) = record(&game, system, body) else {
        return;
    };
    let survey = record[field::SURVEY] as i8;
    let owner = record[field::OWNER];
    let race_picture = if owner < 2 {
        owner
    } else if survey < 30 {
        0
    } else if survey < 40 {
        13
    } else {
        owner
    };
    let planet_picture = if survey < 6 { 0 } else { record[field::TYPE] };
    let pictures = [
        (
            format!("PLANETS/FAJ{race_picture}.PIC"),
            Rect::new(2.0, 2.0, 55.0, 45.0),
            Vec2::new(2.0, 51.0),
        ),
        (
            format!("PLANETS/PLANET{planet_picture}.PIC"),
            Rect::new(2.0, 2.0, 94.0, 53.0),
            Vec2::new(2.0, 147.0),
        ),
    ];
    let planets = data.star_systems[system as usize - 1].moons.len();
    let (icon, icon_pos) = if moon == 0 {
        (napr_planet(planet as usize), Vec2::new(13.0, 105.0))
    } else {
        (napr_moon(body - planets), Vec2::new(21.0, 113.0))
    };
    let pictures = pictures.into_iter().chain([(
        format!("PLANETS/NAPR{system}.PIC#{MASKED}"),
        icon,
        icon_pos,
    )]);
    for (path, rect, pos) in pictures {
        commands.spawn((
            Sprite {
                image: asset_server.load(path),
                rect: Some(rect),
                ..default()
            },
            Anchor::TOP_LEFT,
            place(pos, 0.5),
            scoped.clone(),
        ));
    }
    extra.0 = icons(&game, data, (system as u8, planet as u8, moon as u8), body);
}

/// The icon bar's extras: FUN_1413_000a's INCREASE / DECREASE TAX on your
/// own populated planet, and what ships in orbit can put there
/// (FUN_28cd_062b).
fn icons(game: &Game, data: &GameData, place: (u8, u8, u8), body: usize) -> Vec<u8> {
    let Some(record) = game.0.body(place.0.into(), body) else {
        return Vec::new();
    };
    let mut icons = Vec::new();
    if record[field::OWNER] == 1 && record[field::COLONY] != 0 {
        icons.extend([INCREASE_TAX, DECREASE_TAX]);
    }
    icons.extend(game.0.deploy_actions(&data.exe, place, body));
    icons
}

/// Keeps the extras up to date as things are put on the planet.
fn deploy_icons(
    game: Option<Res<Game>>,
    handle: Res<GameDataHandle>,
    data: Res<Assets<GameData>>,
    mut extra: ResMut<ExtraActions>,
) {
    let (Some(game), Some(data)) = (game, data.get(&handle.0)) else {
        return;
    };
    if !game.is_changed() {
        return;
    }
    let (system, planet, moon) = game.selection();
    let Some(body) = game.selected_body(&data.star_systems) else { return };
    let icons = icons(&game, data, (system as u8, planet as u8, moon as u8), body);
    if extra.0 != icons {
        extra.0 = icons;
    }
}

/// FUN_28cd_0948: an extra icon was used.
fn deploy(
    action: On<ActionUsed>,
    screen: Res<State<GameScreen>>,
    game: Option<ResMut<Game>>,
    handle: Res<GameDataHandle>,
    data: Res<Assets<GameData>>,
    mut commands: Commands,
) {
    if *screen.get() != GameScreen::PlanetInfo {
        return;
    }
    let (Some(mut game), Some(data)) = (game, data.get(&handle.0)) else {
        return;
    };
    let (system, planet, moon) = game.selection();
    let Some(body) = game.selected_body(&data.star_systems) else { return };
    let place = (system as u8, planet as u8, moon as u8);
    if action.0 == deploy::COLONIZATION {
        if !game.0.deploy_actions(&data.exe, place, body).contains(&deploy::COLONIZATION) {
            return;
        }
        match game.0.colony_builder_problem(place.0) {
            Some(problem) => commands.trigger(ShowMessage::new(problem)),
            None => commands.trigger(GoTo(GameScreen::Colonize)),
        }
        return;
    }
    for event in game.0.deploy(&data.exe, &data.sim_texts, action.0, place, body, &mut random) {
        commands.trigger(crate::story::Tell(event));
    }
}

fn record(game: &Game, system: u16, body: usize) -> Option<Vec<u8>> {
    game.0.body(system as usize, body).map(<[u8]>::to_vec)
}

/// A planet's 32x32 picture in its system's NAPR sheet (1-based planet).
pub fn napr_planet(planet: usize) -> Rect {
    let x = 64.0 + (planet as f32 - 1.0) * 32.0;
    Rect::new(x, 1.0, x + 32.0, 33.0)
}

/// A moon's picture in the NAPR sheet: 17-pixel cells, 14 per row, from
/// (66, 34); `moon` counts the system's moons from 1.
pub fn napr_moon(moon: usize) -> Rect {
    let i = moon.saturating_sub(1);
    let x = 66.0 + 17.0 * (i % 14) as f32;
    let y = 34.0 + 17.0 * (i / 14) as f32;
    Rect::new(x, y, x + 16.0, y + 16.0)
}

fn update_texts(
    game: Option<Res<Game>>,
    handle: Res<GameDataHandle>,
    data: Res<Assets<GameData>>,
    mut texts: Query<(&InfoText, &mut Sprite)>,
    added: Query<(), Added<InfoText>>,
    mut images: ResMut<Assets<Image>>,
) {
    let (Some(game), Some(data)) = (game, data.get(&handle.0)) else {
        return;
    };
    if !game.is_changed() && added.is_empty() {
        return;
    }
    let (system, planet, moon) = game.selection();
    let Some(body) = game.selected_body(&data.star_systems) else {
        return;
    };
    let Some(record) = record(&game, system, body) else {
        return;
    };
    let name = game
        .0
        .star_systems()
        .get(system as usize - 1)
        .and_then(|s| s.bodies.get(body - 1))
        .map(|b| b.name.trim_end().to_string())
        .unwrap_or_default();
    let info = PlanetInfo {
        record: &record,
        name,
        is_moon: moon != 0,
        races: game.0.races().into_iter().map(|r| r.name).collect(),
        habitable: data
            .terrain_for_type
            .get(record[field::TYPE] as usize)
            .and_then(|t| data.terrains.get(*t as usize))
            .is_some_and(|t| t.habitable),
        energy_shields: game
            .0
            .buildings()
            .iter()
            .filter(|b| {
                b[0] == ENERGY_SHIELD_BUILDING
                    && b[1] as u16 == system
                    && b[2] as u16 == planet
                    && b[3] as u16 == moon
            })
            .count(),
        // FUN_357b_0359 only counts space stations around New Earth.
        space_stations: if system == 1 && planet == 5 && moon == 0 && game.0.word(0x6280) == Some(5)
        {
            game.0.word(0x628a).unwrap_or(0)
        } else {
            0
        },
    };
    let objects = info.objects();
    for (text, mut sprite) in &mut texts {
        let content = match text.line {
            Line::Object(i) => objects.get(i).cloned().unwrap_or_default(),
            line => info.line(line),
        };
        sprite.image = images.add(data.font.render(&content, text.columns, YELLOW_TEXT));
    }
}

/// Everything FUN_357b_0359 looks at, and the text it makes of it.
struct PlanetInfo<'a> {
    record: &'a [u8],
    name: String,
    is_moon: bool,
    races: Vec<String>,
    habitable: bool,
    energy_shields: usize,
    space_stations: u16,
}

impl PlanetInfo<'_> {
    fn byte(&self, field: usize) -> i8 {
        self.record[field] as i8
    }

    fn owner(&self) -> u8 {
        self.record[field::OWNER]
    }

    fn survey(&self) -> i8 {
        self.byte(field::SURVEY)
    }

    fn population(&self) -> u32 {
        let p = &self.record[field::POPULATION..field::POPULATION + 4];
        u32::from_le_bytes([p[0], p[1], p[2], p[3]])
    }

    fn is_alien(&self) -> bool {
        (2..=20).contains(&self.owner())
    }

    fn line(&self, line: Line) -> String {
        match line {
            Line::Name => self.name_line(),
            Line::Population => self.population_line(),
            Line::Tax => self.tax_line(),
            Line::Tech => self.tech_line(),
            Line::Type => self.type_line(),
            Line::Mineral(i) => self.mineral(i).to_string(),
            Line::Diameter => {
                if self.survey() < 3 {
                    "??????".into()
                } else {
                    let d = u16::from_le_bytes([
                        self.record[field::DIAMETER],
                        self.record[field::DIAMETER + 1],
                    ]);
                    format!("{d:>6}")
                }
            }
            Line::Temperature => {
                if self.survey() < 5 {
                    "????".into()
                } else {
                    let t = i16::from_le_bytes([
                        self.record[field::TEMPERATURE],
                        self.record[field::TEMPERATURE + 1],
                    ]);
                    format!("{t:>4}")
                }
            }
            Line::Object(_) => String::new(),
        }
    }

    fn name_line(&self) -> String {
        let owner = match self.owner() {
            0 => "  ".to_string(),
            1 => "YOUR ".to_string(),
            o if self.is_alien() => match self.survey() {
                s if s < 30 => String::new(),
                s if s < 40 => "ALIEN ".to_string(),
                // Owner o is race record o - 2 (DS:0x6a06 + 0xe4 * o).
                _ => format!(
                    "{} ",
                    self.races.get(o as usize - 2).map_or("", String::as_str)
                ),
            },
            _ => String::new(),
        };
        let kind = if self.is_moon {
            "MOON'S NAME: "
        } else {
            "PLANET'S NAME:  "
        };
        format!("{owner}{kind}{}", self.name)
    }

    fn population_line(&self) -> String {
        let mut text = if self.population() == 0 {
            "No".to_string()
        } else {
            self.population().to_string()
        };
        let empty = self.record[field::HAS_LIFE] != 0
            && self.habitable
            && self.survey() >= 30
            && (self.owner() == 0
                || (self.owner() == 1
                    && self.record[field::COLONY] == 0
                    && self.record[field::COLONISTS] == 0));
        if empty {
            text = "Empty planet, ready for colonysation".into();
        }
        if self.owner() >= 2 {
            text = match self.survey() {
                s if s < 30 => "No".into(),
                s if s < 40 => "Unknown".into(),
                _ => format!("{} alien", self.population()),
            };
        }
        if self.owner() == 1 && self.population() > 0 {
            let attitude = match self.byte(field::ATTITUDE) {
                0..=9 => " revolting men",
                10..=19 => " people hate you",
                20..=29 => " people don't like you",
                30..=39 => " neutral men",
                40..=49 => " people find you sympathetic",
                50..=59 => " people like you",
                60..=69 => " loyal people",
                _ => "",
            };
            text = format!("{}{attitude}", self.population());
        }
        text
    }

    fn tax_line(&self) -> String {
        if self.owner() != 1 {
            return "No".into();
        }
        [
            "None",
            "Very low",
            "Low",
            "Normal",
            "High",
            "Difficult",
            "Very difficult",
            "Oppressing",
        ]
        .get(self.record[field::TAX] as usize)
        .copied()
        .unwrap_or("")
        .into()
    }

    fn tech_line(&self) -> String {
        let alien = self.owner() >= 2;
        if alien && self.survey() < 30 {
            return "No".into();
        }
        if alien && self.survey() < 40 {
            return "Unknown".into();
        }
        if self.owner() != 1 && self.survey() < 40 {
            return String::new();
        }
        match self.byte(field::TECH) {
            0..=9 => "No",
            10..=19 => "Under developed",
            20..=29 => "Poor",
            30..=39 => "Developing",
            40..=49 => "Enhanced",
            50..=59 => "High-tech",
            60..=69 => "Super-tech",
            _ => "",
        }
        .into()
    }

    fn type_line(&self) -> String {
        let mut text = if self.survey() > 5 {
            [
                "No data",
                "Earth-like",
                "Gaseous",
                "Icy",
                "Watery",
                "Tropical",
                "Desert",
                "Swampy",
                "Rocky",
                "Too hot",
                "No atmosphere",
            ]
            .get(self.record[field::TYPE] as usize)
            .copied()
            .unwrap_or("")
            .to_string()
        } else {
            "Unknown".to_string()
        };
        let mut status = String::new();
        if self.survey() >= 20 {
            status = if self.record[field::HAS_LIFE] != 0 && self.habitable {
                "life supporting".into()
            } else {
                "not accomodable".into()
            };
        }
        if (30..40).contains(&self.survey()) && self.owner() >= 2 {
            status = "enemy movement".into();
        }
        if self.record[field::HAS_LIFE] != 0 && self.habitable && self.survey() >= 40 {
            if self.owner() == 1 {
                if self.record[field::COLONY] != 0 {
                    status = "colony".into();
                }
                if self.record[field::COLONY] == 0 && self.byte(field::COLONISTS) > 0 {
                    status = "under colonization".into();
                }
            } else if self.is_alien() && self.record[field::COLONY] != 0 {
                status = "Alien colony".into();
            }
        }
        if !status.is_empty() {
            text = format!("{text}, {status}");
        }
        text
    }

    fn mineral(&self, i: usize) -> &'static str {
        if self.survey() < 10 {
            "???"
        } else if self.record[field::MINERAL_AMOUNTS + i] >= 10
            && self.record[field::MINERALS_KNOWN] != 0
        {
            "YES"
        } else {
            "NO "
        }
    }

    fn objects(&self) -> Vec<String> {
        let satellite = |spy_name: &str| match self.byte(field::SATELLITE) {
            1 => Some("Satellite".to_string()),
            s if s > 1 => Some(spy_name.to_string()),
            _ => None,
        };
        let mut list = Vec::new();
        match self.owner() {
            0 => list.extend(satellite("Spy satellite")),
            1 => {
                if self.record[field::COLONY] != 0 {
                    list.push("Colony".into());
                }
                if self.energy_shields > 0 {
                    list.push("Energy Shield".into());
                }
                if self.space_stations > 0 {
                    list.push(format!("{} Space Station", self.space_stations));
                }
                let miners = self.record[field::MINERS];
                if miners > 0 {
                    list.push(if self.record[field::COLONY] == 0 {
                        "Miner Station".into()
                    } else {
                        format!("{miners} Miner Droid")
                    });
                }
                let solar = self.record[field::SOLAR_SATS];
                if solar > 0 && self.record[field::COLONY] != 0 {
                    list.push(format!("{solar} Solar Sat"));
                }
                list.extend(satellite("Spy Satellite"));
            }
            _ => {
                if self.survey() > 30 && self.record[field::COLONY] != 0 {
                    list.push("Colony".into());
                }
                if self.survey() > 40
                    && self.record[field::MINERS] > 0
                    && self.record[field::COLONY] == 0
                {
                    list.push("Miner Station".into());
                }
                if self.survey() > 40 && self.energy_shields > 0 {
                    list.push("Energy Shield".into());
                }
                list.extend(satellite("Spy Satellite"));
                if self.record[field::SPY_SHIP] != 0 {
                    list.push("Spy Ship".into());
                }
            }
        }
        list
    }
}

fn change_tax(
    action: On<ActionUsed>,
    screen: Res<State<GameScreen>>,
    game: Option<ResMut<Game>>,
    handle: Res<GameDataHandle>,
    data: Res<Assets<GameData>>,
) {
    if *screen.get() != GameScreen::PlanetInfo {
        return;
    }
    let (Some(mut game), Some(data)) = (game, data.get(&handle.0)) else {
        return;
    };
    let (system, _, _) = game.selection();
    let Some(body) = game.selected_body(&data.star_systems) else {
        return;
    };
    let Some(record) = game.0.planet_mut(system as usize, body) else {
        return;
    };
    let tax = &mut record[field::TAX];
    match action.0 {
        INCREASE_TAX => *tax = (*tax + 1).min(7),
        DECREASE_TAX => *tax = tax.saturating_sub(1),
        _ => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// New Earth's record at the start of a game.
    const NEW_EARTH: [u8; 65] = {
        let mut r = [0u8; 65];
        let head = [
            0x01, 0x01, 0x01, 0x01, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x02, 0x00, 0x64, 0x30,
            0x75, 0x00, 0x00, 0x28, 0x03, 0x32, 0xff, 0x01, 0x00, 0x6a, 0x03, 0x2b, 0x01,
        ];
        let mut i = 0;
        while i < head.len() {
            r[i] = head[i];
            i += 1;
        }
        let minerals = [0x46, 0x1e, 0x14, 0x1e, 0x00, 0x00];
        let mut i = 0;
        while i < minerals.len() {
            r[0x3b + i] = minerals[i];
            i += 1;
        }
        r
    };

    fn new_earth() -> PlanetInfo<'static> {
        PlanetInfo {
            record: &NEW_EARTH,
            name: "New Earth".into(),
            is_moon: false,
            races: vec![],
            habitable: true,
            energy_shields: 0,
            space_stations: 0,
        }
    }

    #[test]
    fn new_earth_reads_like_the_original() {
        let info = new_earth();
        assert_eq!(info.line(Line::Name), "YOUR PLANET'S NAME:  New Earth");
        assert_eq!(info.line(Line::Population), "30000 people like you");
        assert_eq!(info.line(Line::Tax), "Normal");
        assert_eq!(info.line(Line::Tech), "Enhanced");
        assert_eq!(info.line(Line::Type), "Earth-like, colony");
        assert_eq!(info.line(Line::Diameter), "   874");
        assert_eq!(info.line(Line::Temperature), " 299");
        let minerals: Vec<_> = (0..6).map(|i| info.mineral(i)).collect();
        assert_eq!(minerals, ["YES", "YES", "YES", "YES", "NO ", "NO "]);
        assert_eq!(info.objects(), ["Colony", "2 Miner Droid"]);
    }
}
