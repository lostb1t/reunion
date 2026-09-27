//! RESEARCH-DESIGN (screen 3): the 35 inventions and their research.
//!
//! From REUNION.PRG FUN_2d66_*: RESEARCH.PIC (the screen table's background)
//! at row 49 shows a 5 x 7 grid of computer drives, one per invention, at
//! (32 * column, 52 + 16 * row). Each available invention gets a drive from
//! CDS.PIC for its status (word 0x11 of its record): 1 can be developed,
//! 2 under development, 3 can be analysed, 4 under analysis, 5 done. Choosing
//! one starts or stops its project; only one runs at a time (DS:0x95a2). The
//! monitor shows the selected invention (DS:0x95a0), and the bottom line the
//! developer (DS:0x95bc) with the RESEARCH COMPUTER lamp.

use bevy::prelude::*;
use bevy::sprite::Anchor;

use crate::focus::{Activated, DefaultFocus, Hover, hotspot};
use crate::game::Game;
use crate::game_data::{GameData, GameDataHandle};
use crate::hud::{CONTENT_Y, HoverLabel, RED_TEXT, YELLOW_TEXT};
use crate::screen::{GameScreen, picture, place};

pub struct ResearchPlugin;

impl Plugin for ResearchPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(OnEnter(GameScreen::Research), enter)
            .add_systems(
                Update,
                (update_drives, update_texts).run_if(in_state(GameScreen::Research)),
            )
            .add_observer(choose);
    }
}

const INVENTIONS: usize = 35;
const COLUMNS: usize = 5;
const GRID_TOP: f32 = 52.0;
const CELL: Vec2 = Vec2::new(32.0, 16.0);

/// Record fields of an invention.
const STATUS: usize = 0x11;
const PROGRESS: usize = 0x15;
const REQUIREMENTS: usize = 0x31;

/// Selected invention (shown on the monitor) and the running project.
const SELECTED: u16 = 0x95a0;
const PROJECT: u16 = 0x95a2;
const PROJECT_WORKERS: u16 = 0x95aa;
const DEVELOPER: u16 = 0x95bc;
/// The developer's Math, Physics, Elect and A.Int skills.
const DEVELOPER_SKILLS: u16 = 0x95ac;
/// Invention 14 (Hyperspace drv) stays "Unknown" until this flag is set.
const HYPERSPACE_KNOWN: u16 = 0x5d62;
const HYPERSPACE: usize = 14;

/// CDS.PIC frame per status (FUN_2d66_0a1a): frame n is the 31 x 14 drive at
/// (32 * (n % 10), 16 * (n / 10) + 1), drawn one row into its cell.
fn drive_frame(status: u16) -> Option<Rect> {
    let frame = match status {
        1 => 10,
        2 => 19,
        3 => 30,
        4 => 39,
        5 => 0,
        _ => return None,
    };
    let min = Vec2::new(32.0 * (frame % 10) as f32, 16.0 * (frame / 10) as f32);
    Some(Rect::from_corners(min, min + Vec2::new(31.0, 14.0)))
}

fn status_text(status: u16) -> &'static str {
    match status {
        1 => "Can be developed",
        2 => "Under development",
        3 => "Can be analysed",
        4 => "Under analysis",
        5 => "Done",
        _ => "",
    }
}

/// The science labels (DS:0x5b4e, 0x5b56, 0x5b5e, 0x5b66) as FUN_2d66_03e9
/// pads them, with their label and value positions.
const REQUIREMENT_LINES: [(&str, Vec2, usize, f32); 4] = [
    ("Math :", Vec2::new(189.0, 139.0), 6, 224.0),
    ("Physics:", Vec2::new(235.0, 139.0), 8, 282.0),
    ("Elect:", Vec2::new(189.0, 149.0), 6, 224.0),
    ("A.Int  :", Vec2::new(235.0, 149.0), 8, 282.0),
];
/// The developer's skills at y 188 (FUN_2d66_074c: 100 + DS:0xc24 table).
const SKILL_X: [f32; 4] = [136.0, 203.0, 259.0, 296.0];

#[derive(Component)]
struct Drive(usize);

#[derive(Component)]
struct InventionSlot(usize);

#[derive(Component, Clone, Copy)]
enum Text {
    Title,
    Name,
    Status,
    Progress,
    Requirement(usize),
    RequirementValue(usize),
    Developer,
    Skill(usize),
}

impl Text {
    fn place(self) -> (Vec2, usize) {
        match self {
            Text::Title => (Vec2::new(190.0, 76.0), 18),
            Text::Name => (Vec2::new(200.0, 88.0), 16),
            Text::Status => (Vec2::new(190.0, 100.0), 18),
            Text::Progress => (Vec2::new(190.0, 112.0), 18),
            Text::Requirement(i) => (REQUIREMENT_LINES[i].1, REQUIREMENT_LINES[i].2),
            Text::RequirementValue(i) => (
                Vec2::new(REQUIREMENT_LINES[i].3, REQUIREMENT_LINES[i].1.y),
                1,
            ),
            Text::Developer => (Vec2::new(8.0, 188.0), 14),
            Text::Skill(i) => (Vec2::new(SKILL_X[i], 188.0), 1),
        }
    }
}

fn enter(mut commands: Commands, asset_server: Res<AssetServer>, mut game: Option<ResMut<Game>>) {
    let scoped = DespawnOnExit(GameScreen::Research);
    commands.spawn((
        picture(
            asset_server.load("GRAFIKA/RESEARCH.PIC"),
            Vec2::new(0.0, CONTENT_Y),
        ),
        scoped.clone(),
    ));
    let cds: Handle<Image> = asset_server.load("GRAFIKA/CDS.PIC");
    for i in 0..INVENTIONS {
        let cell = Vec2::new(
            CELL.x * (i % COLUMNS) as f32,
            GRID_TOP + CELL.y * (i / COLUMNS) as f32,
        );
        commands.spawn((
            Drive(i + 1),
            Sprite {
                image: cds.clone(),
                ..default()
            },
            Anchor::TOP_LEFT,
            place(cell + Vec2::new(0.0, 1.0), 0.5),
            Visibility::Hidden,
            scoped.clone(),
        ));
        // FUN_2d66_0299: hotspots start 2 pixels left of the drive.
        let left = (cell.x - 2.0).max(0.0);
        let mut slot = commands.spawn((
            InventionSlot(i + 1),
            HoverLabel(String::new()),
            hotspot(
                Rect::new(left, cell.y, cell.x + CELL.x, cell.y + CELL.y),
                Hover::Outline,
            ),
            scoped.clone(),
        ));
        if i == 0 {
            slot.insert(DefaultFocus);
        }
    }
    let texts = [
        Text::Title,
        Text::Name,
        Text::Status,
        Text::Progress,
        Text::Developer,
    ]
    .into_iter()
    .chain((0..4).map(Text::Requirement))
    .chain((0..4).map(Text::RequirementValue))
    .chain((0..4).map(Text::Skill));
    for text in texts {
        let (pos, _) = text.place();
        commands.spawn((
            text,
            Sprite::default(),
            Anchor::TOP_LEFT,
            place(pos, 1.0),
            Visibility::Hidden,
            scoped.clone(),
        ));
    }
    // The RESEARCH COMPUTER lamp, OFF without a developer (FUN_2d66_074c).
    let developer = game.as_ref().and_then(|g| g.0.word(DEVELOPER)).unwrap_or(0);
    let lamp = if developer == 0 {
        [
            (Rect::new(18.0, 79.0, 29.0, 84.0), Vec2::new(76.0, 170.0)),
            (Rect::new(30.0, 79.0, 35.0, 84.0), Vec2::new(149.0, 170.0)),
        ]
    } else {
        [
            (Rect::new(0.0, 79.0, 11.0, 84.0), Vec2::new(76.0, 170.0)),
            (Rect::new(12.0, 79.0, 17.0, 84.0), Vec2::new(149.0, 170.0)),
        ]
    };
    for (rect, pos) in lamp {
        commands.spawn((
            Sprite {
                image: cds.clone(),
                rect: Some(rect),
                ..default()
            },
            Anchor::TOP_LEFT,
            place(pos, 0.5),
            scoped.clone(),
        ));
    }
    // The monitor starts on the running project (FUN_2d66_0029).
    if let Some(game) = game.as_mut() {
        let project = game.0.word(PROJECT).unwrap_or(0);
        game.0.set_word(SELECTED, project);
    }
}

fn status(game: &Game, invention: usize) -> u16 {
    game.0.inventions().get(invention - 1).map_or(0, |r| {
        u16::from_le_bytes([r.data[STATUS], r.data[STATUS + 1]])
    })
}

fn hidden_invention(game: &Game, invention: usize) -> bool {
    invention == HYPERSPACE && game.0.byte(HYPERSPACE_KNOWN) == Some(0)
}

fn update_drives(
    game: Option<Res<Game>>,
    mut drives: Query<(&Drive, &mut Sprite, &mut Visibility)>,
    mut slots: Query<(&InventionSlot, &mut HoverLabel)>,
    added: Query<(), Added<Drive>>,
) {
    let Some(game) = game else { return };
    if !game.is_changed() && added.is_empty() {
        return;
    }
    let inventions = game.0.inventions();
    for (Drive(i), mut sprite, mut visibility) in &mut drives {
        match drive_frame(status(&game, *i)) {
            Some(rect) => {
                sprite.rect = Some(rect);
                *visibility = Visibility::Inherited;
            }
            None => *visibility = Visibility::Hidden,
        }
    }
    for (InventionSlot(i), mut label) in &mut slots {
        let name = if status(&game, *i) < 1 {
            String::new()
        } else if hidden_invention(&game, *i) {
            "Unknown".to_string()
        } else {
            inventions[*i - 1].name.clone()
        };
        label.0 = name;
    }
}

fn update_texts(
    game: Option<Res<Game>>,
    handle: Res<GameDataHandle>,
    data: Res<Assets<GameData>>,
    mut texts: Query<(&Text, &mut Sprite, &mut Visibility)>,
    added: Query<(), Added<Text>>,
    mut images: ResMut<Assets<Image>>,
) {
    let (Some(game), Some(data)) = (game, data.get(&handle.0)) else {
        return;
    };
    if !game.is_changed() && added.is_empty() {
        return;
    }
    let selected = game.0.word(SELECTED).unwrap_or(0) as usize;
    let inventions = game.0.inventions();
    let record = selected.checked_sub(1).and_then(|i| inventions.get(i));
    let developer = game.0.word(DEVELOPER).unwrap_or(0);
    for (text, mut sprite, mut visibility) in &mut texts {
        let content: Option<(String, bool)> = match (*text, record) {
            // FUN_2d66_074c: the developer (commander category 4) and skills.
            (Text::Developer, _) if developer == 0 => Some(("No developer".to_string(), false)),
            (Text::Developer, _) => {
                let name = crate::commanders::candidate_lines(data, 4, developer as usize)[0]
                    .trim_end()
                    .trim_end_matches(':')
                    .trim_end_matches("says")
                    .trim()
                    .to_string();
                Some((name, false))
            }
            (Text::Skill(_), _) if developer == 0 => Some(("-".to_string(), false)),
            (Text::Skill(i), _) => Some((
                game.0
                    .word(DEVELOPER_SKILLS + 2 * i as u16)
                    .unwrap_or(0)
                    .to_string(),
                false,
            )),
            (_, None) => None,
            (Text::Title, _) => Some(("Project name :".into(), false)),
            (_, Some(_)) if hidden_invention(&game, selected) => match text {
                Text::Name => Some(("Unknown".into(), false)),
                _ => None,
            },
            (Text::Name, Some(r)) => Some((r.name.clone(), false)),
            (Text::Status, _) => Some((status_text(status(&game, selected)).into(), false)),
            (Text::Progress, Some(r)) => match status(&game, selected) {
                2 | 4 if game.0.word(PROJECT_WORKERS) == Some(0) => {
                    Some(("No developer".into(), false))
                }
                2 | 4 => {
                    let progress =
                        i16::from_le_bytes([r.data[PROGRESS], r.data[PROGRESS + 1]]) / 100;
                    Some((format!("Completed: {progress}%"), false))
                }
                _ => None,
            },
            (Text::Requirement(i), _) if status(&game, selected) < 5 => {
                Some((REQUIREMENT_LINES[i].0.into(), false))
            }
            (Text::RequirementValue(i), Some(r)) if status(&game, selected) < 5 => {
                Some((r.data[REQUIREMENTS + i].to_string(), true))
            }
            _ => None,
        };
        match content {
            Some((content, red)) => {
                let columns = text.place().1;
                let colors = if red { RED_TEXT } else { YELLOW_TEXT };
                sprite.image = images.add(data.font.render(&content, columns, colors));
                *visibility = Visibility::Inherited;
            }
            None => *visibility = Visibility::Hidden,
        }
    }
}

/// FUN_2d66_0029: choosing an invention shows it and starts or stops its project.
fn choose(activated: On<Activated>, slots: Query<&InventionSlot>, game: Option<ResMut<Game>>) {
    let (Ok(InventionSlot(invention)), Some(mut game)) = (slots.get(activated.0), game) else {
        return;
    };
    let invention = *invention;
    let current = status(&game, invention);
    if current < 1 {
        return;
    }
    game.0.set_word(SELECTED, invention as u16);
    if hidden_invention(&game, invention) {
        return;
    }
    let project = game.0.word(PROJECT).unwrap_or(0) as usize;
    let set_status = |game: &mut Game, invention: usize, status: u16| {
        if let Some(record) = game.0.invention_mut(invention) {
            record[STATUS..STATUS + 2].copy_from_slice(&status.to_le_bytes());
        }
    };
    // Starting a project pauses the one that was running.
    let start = |game: &mut Game, running: u16| {
        if project != 0 && project != invention {
            match status(game, project) {
                2 => set_status(game, project, 1),
                4 => set_status(game, project, 3),
                _ => {}
            }
        }
        set_status(game, invention, running);
        game.0.set_word(PROJECT, invention as u16);
    };
    match current {
        1 => start(&mut game, 2),
        3 => start(&mut game, 4),
        2 => {
            set_status(&mut game, invention, 1);
            game.0.set_word(PROJECT, 0);
        }
        4 => {
            set_status(&mut game, invention, 3);
            game.0.set_word(PROJECT, 0);
        }
        _ => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn drives_come_from_the_cd_sheet() {
        assert_eq!(drive_frame(1), Some(Rect::new(0.0, 16.0, 31.0, 30.0)));
        assert_eq!(drive_frame(4), Some(Rect::new(288.0, 48.0, 319.0, 62.0)));
        assert_eq!(drive_frame(0), None);
    }
}
