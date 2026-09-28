//! COMMANDERS (screen 2): hiring pilots, builders, fighters and developers.
//!
//! From REUNION.PRG FUN_3fab_*: FACES.PIC (the screen table's background) at
//! row 49 and, per category, FACES<n>.PIC's three portraits at (1, 50). The
//! category icons pick the category (DS:0x899e), a portrait picks a candidate
//! (DS:0x899c), whose four lines from TEXT/SZ_FACE.RAW (4 x 41-byte Pascal
//! strings per candidate) and level are shown at x 10, y 164-188. HIRE MAN
//! pays the candidate's salary and makes them the category's commander
//! (DS:0x95b4 + 2 * category); the developer's skills feed RESEARCH-DESIGN.

use bevy::prelude::*;
use bevy::sprite::Anchor;
use reunion_formats::exe::GameExe;

use crate::audio::Sfx;
use crate::focus::{Activated, Hover, hotspot};
use crate::game::Game;
use crate::game_data::{GameData, GameDataHandle};
use crate::hud::{ActionUsed, CONTENT_Y, HoverLabel, YELLOW_TEXT};
use crate::screen::{GameScreen, picture, place};

pub struct CommandersPlugin;

impl Plugin for CommandersPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(OnEnter(GameScreen::Commanders), enter)
            .add_systems(
                Update,
                (update_portraits, update_texts).run_if(in_state(GameScreen::Commanders)),
            )
            .add_observer(pick_category)
            .add_observer(pick_candidate)
            .add_observer(hire);
    }
}

/// Category icons PILOTS, BUILDERS, FIGHTERS, DEVELOPERS: actions 50-53.
const FIRST_CATEGORY_ACTION: u8 = 50;
const HIRE_MAN: u8 = 40;
const DEVELOPERS: usize = 4;

/// Hired candidate per category (0 = nobody) and their current level.
const HIRED: u16 = 0x95b4;
const LEVEL: u16 = 0x95a2;
/// The developer's current skills (Math, Physics, Elect, A.Int).
const DEVELOPER_SKILLS: u16 = 0x95ac;
const MONEY_LOW: u16 = 0x95be;
const MONEY_HIGH: u16 = 0x95c0;

/// Candidate portraits: FUN_3fab_03ba's frames.
const PORTRAITS: [Rect; 3] = [
    Rect {
        min: Vec2::new(5.0, 50.0),
        max: Vec2::new(105.0, 160.0),
    },
    Rect {
        min: Vec2::new(110.0, 50.0),
        max: Vec2::new(210.0, 160.0),
    },
    Rect {
        min: Vec2::new(216.0, 50.0),
        max: Vec2::new(316.0, 160.0),
    },
];
const LINES_Y: [f32; 4] = [164.0, 172.0, 180.0, 188.0];
const LINE_COLUMNS: usize = 50;
const SCIENCES: [&str; 4] = ["Math", "Physics", "Elect", "A.Int"];

/// The category shown (1-4) and the chosen candidate (0 = none).
#[derive(Resource)]
struct View {
    category: usize,
    candidate: usize,
}

#[derive(Component)]
struct Portraits;

#[derive(Component)]
struct Candidate(usize);

#[derive(Component)]
struct Line(usize);

fn enter(mut commands: Commands, asset_server: Res<AssetServer>) {
    let scoped = DespawnOnExit(GameScreen::Commanders);
    commands.insert_resource(View {
        category: 1,
        candidate: 0,
    });
    commands.spawn((
        picture(
            asset_server.load("GRAFIKA/FACES.PIC"),
            Vec2::new(0.0, CONTENT_Y),
        ),
        scoped.clone(),
    ));
    commands.spawn((
        Portraits,
        Sprite::default(),
        Anchor::TOP_LEFT,
        place(Vec2::new(1.0, 50.0), 0.5),
        scoped.clone(),
    ));
    for (i, rect) in PORTRAITS.into_iter().enumerate() {
        commands.spawn((
            Candidate(i + 1),
            HoverLabel(String::new()),
            hotspot(rect, Hover::Outline),
            scoped.clone(),
        ));
    }
    for (i, y) in LINES_Y.into_iter().enumerate() {
        commands.spawn((
            Line(i),
            Sprite::default(),
            Anchor::TOP_LEFT,
            place(Vec2::new(10.0, y), 1.0),
            Visibility::Hidden,
            scoped.clone(),
        ));
    }
}

fn update_portraits(
    view: Option<Res<View>>,
    asset_server: Res<AssetServer>,
    mut portraits: Single<&mut Sprite, With<Portraits>>,
    added: Query<(), Added<Portraits>>,
) {
    let Some(view) = view else { return };
    if !view.is_changed() && added.is_empty() {
        return;
    }
    // FUN_3fab_0321: rows 1-109 of FACES<category> at (1, 50).
    portraits.image = asset_server.load(format!("GRAFIKA/FACES{}.PIC", view.category));
    portraits.rect = Some(Rect::new(1.0, 1.0, 320.0, 110.0));
}

/// A candidate's four text lines, as stored.
pub fn candidate_lines(data: &GameData, category: usize, candidate: usize) -> [String; 4] {
    data.commanders
        .get((category - 1) * 3 + candidate - 1)
        .cloned()
        .unwrap_or_default()
}

/// Parses TEXT/SZ_FACE.RAW: per candidate four 41-byte Pascal strings.
pub fn parse_candidates(raw: &[u8]) -> Vec<[String; 4]> {
    raw.chunks_exact(4 * 41)
        .map(|record| {
            std::array::from_fn(|i| {
                let field = &record[i * 41..(i + 1) * 41];
                let len = (field[0] as usize).min(40);
                field[1..1 + len].iter().map(|&b| b as char).collect()
            })
        })
        .collect()
}

fn update_texts(
    view: Option<Res<View>>,
    game: Option<Res<Game>>,
    handle: Res<GameDataHandle>,
    data: Res<Assets<GameData>>,
    mut lines: Query<(&Line, &mut Sprite, &mut Visibility)>,
    mut candidates: Query<(&Candidate, &mut HoverLabel)>,
    mut images: ResMut<Assets<Image>>,
) {
    let (Some(view), Some(game), Some(data)) = (view, game, data.get(&handle.0)) else {
        return;
    };
    let tables = &data.hire;
    if !view.is_changed() && !game.is_changed() {
        return;
    }
    for (Candidate(c), mut label) in &mut candidates {
        // The name from "<name> says:".
        let first = &candidate_lines(data, view.category, *c)[0];
        label.0 = first
            .trim_end()
            .trim_end_matches(':')
            .trim_end_matches("says")
            .trim_end_matches("Says")
            .trim()
            .to_string();
    }
    let texts = if view.candidate == 0 {
        None
    } else {
        Some(texts_for(
            &game,
            data,
            tables,
            view.category,
            view.candidate,
        ))
    };
    for (Line(i), mut sprite, mut visibility) in &mut lines {
        match texts.as_ref().map(|t| &t[*i]) {
            Some(text) if !text.is_empty() => {
                sprite.image = images.add(data.font.render(text, LINE_COLUMNS, YELLOW_TEXT));
                *visibility = Visibility::Inherited;
            }
            _ => *visibility = Visibility::Hidden,
        }
    }
}

/// FUN_3fab_048f's four lines for a candidate.
fn texts_for(
    game: &Game,
    data: &GameData,
    tables: &HireTables,
    category: usize,
    candidate: usize,
) -> [String; 4] {
    let state = &game.0;
    let stored = candidate_lines(data, category, candidate);
    let hired = state.word(HIRED + 2 * category as u16).unwrap_or(0) as usize;
    let is_hired = candidate == hired;
    let level = if is_hired {
        state.word(LEVEL + 2 * category as u16).unwrap_or(0) as u32
    } else {
        tables.level(category, candidate)
    };
    let third = if category == DEVELOPERS {
        let skills: Vec<u32> = (0..4)
            .map(|i| {
                if is_hired {
                    state.word(DEVELOPER_SKILLS + 2 * i as u16).unwrap_or(0) as u32
                } else {
                    tables.skill(candidate, i)
                }
            })
            .collect();
        let mut line = "Level:  ".to_string();
        for (name, value) in SCIENCES.iter().zip(skills) {
            line.push_str(&format!("{name}: {value}  "));
        }
        line
    } else {
        format!("My scholarly level:  {level}")
    };
    let fourth = if candidate <= hired {
        if is_hired {
            "I work for you"
        } else {
            "I'm no better than your advisor"
        }
        .to_string()
    } else {
        stored[3].clone()
    };
    [stored[0].clone(), stored[1].clone(), third, fourth]
}

fn pick_category(action: On<ActionUsed>, view: Option<ResMut<View>>, mut commands: Commands) {
    let Some(mut view) = view else { return };
    if let Some(category) = action
        .0
        .checked_sub(FIRST_CATEGORY_ACTION)
        .filter(|c| *c < 4)
    {
        // The category's name, or a click when it's already shown.
        let sound = ["pilots", "builders", "fighters", "develope"][category as usize];
        let changed = view.category != category as usize + 1;
        commands.trigger(Sfx::named(if changed { sound } else { "x" }));
        view.category = category as usize + 1;
        view.candidate = 0;
    }
}

fn pick_candidate(
    activated: On<Activated>,
    candidates: Query<&Candidate>,
    view: Option<ResMut<View>>,
) {
    if let (Ok(Candidate(c)), Some(mut view)) = (candidates.get(activated.0), view) {
        view.candidate = *c;
    }
}

/// Salaries, levels and developer skills from REUNION.PRG's data segment.
pub struct HireTables {
    /// 32-bit salary per category and candidate: DS:0x578c + 12 * cat + 4 * cand.
    salaries: [[u32; 3]; 4],
    /// DS:0x57c4 + 6 * cat + 2 * cand.
    levels: [[u32; 3]; 4],
    /// DS:0x57eb + 4 * cand + skill (1-based skill).
    skills: [[u32; 4]; 3],
}

impl HireTables {
    pub fn read(exe: &GameExe) -> Option<Self> {
        let word = |a: u16| {
            exe.ds_bytes(a, 2)
                .map(|b| u16::from_le_bytes([b[0], b[1]]) as u32)
        };
        let byte = |a: u16| exe.ds_bytes(a, 1).map(|b| b[0] as u32);
        let mut tables = HireTables {
            salaries: [[0; 3]; 4],
            levels: [[0; 3]; 4],
            skills: [[0; 4]; 3],
        };
        for cat in 1..=4u16 {
            for cand in 1..=3u16 {
                let at = 0x578c + 12 * cat + 4 * cand;
                tables.salaries[cat as usize - 1][cand as usize - 1] =
                    word(at)? | word(at + 2)? << 16;
                tables.levels[cat as usize - 1][cand as usize - 1] =
                    word(0x57c4 + 6 * cat + 2 * cand)?;
            }
        }
        for cand in 1..=3u16 {
            for skill in 1..=4u16 {
                tables.skills[cand as usize - 1][skill as usize - 1] =
                    byte(0x57eb + 4 * cand + skill)?;
            }
        }
        Some(tables)
    }

    pub fn salary(&self, category: usize, candidate: usize) -> u32 {
        self.salaries[category - 1][candidate - 1]
    }

    pub fn level(&self, category: usize, candidate: usize) -> u32 {
        self.levels[category - 1][candidate - 1]
    }

    pub fn skill(&self, candidate: usize, skill: usize) -> u32 {
        self.skills[candidate - 1][skill]
    }
}

/// FUN_3fab_0032's HIRE MAN: only someone better than the current commander,
/// and only if you can pay the salary. The old commander's level goes back.
fn hire(
    action: On<ActionUsed>,
    screen: Res<State<GameScreen>>,
    view: Option<Res<View>>,
    handle: Res<GameDataHandle>,
    data: Res<Assets<GameData>>,
    game: Option<ResMut<Game>>,
    mut commands: Commands,
) {
    if action.0 != HIRE_MAN || *screen.get() != GameScreen::Commanders {
        return;
    }
    let (Some(view), Some(data), Some(mut game)) = (view, data.get(&handle.0), game) else {
        return;
    };
    let tables = &data.hire;
    let (category, candidate) = (view.category, view.candidate);
    if candidate == 0 {
        info!("choose someone to hire first");
        return;
    }
    let state = &mut game.0;
    let hired = state.word(HIRED + 2 * category as u16).unwrap_or(0) as usize;
    if candidate <= hired {
        info!("{} is no better than your commander", candidate);
        commands.trigger(Sfx::named("hiba"));
        return;
    }
    let salary = tables.salary(category, candidate);
    if state.money() < salary {
        info!("not enough money to hire");
        commands.trigger(Sfx::named("hiba"));
        return;
    }
    commands.trigger(Sfx::named("welcome"));
    let money = state.money() - salary;
    state.set_word(MONEY_LOW, money as u16);
    state.set_word(MONEY_HIGH, (money >> 16) as u16);
    state.set_word(HIRED + 2 * category as u16, candidate as u16);
    state.set_word(
        LEVEL + 2 * category as u16,
        tables.level(category, candidate) as u16,
    );
    if category == DEVELOPERS {
        for skill in 0..4 {
            state.set_word(
                DEVELOPER_SKILLS + 2 * skill as u16,
                tables.skill(candidate, skill) as u16,
            );
        }
    }
    info!("hired candidate {candidate} for {salary} Cr");
}
