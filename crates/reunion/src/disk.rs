//! DISK OPERATIONS (screen 12): saving and loading.
//!
//! From REUNION.PRG FUN_1a3e_0008 and its helpers: DISK.PIC at row 49; twelve
//! slots listed at x 16, y 64 + 9 * (slot - 1), 19 characters wide, the
//! selected one in highlight colors. A slot's name is the first 20 bytes of
//! `SAVE/SPIDYSAV.<n>` (a Pascal string), or "Empty---------------" when there
//! is no file. LOAD needs a used slot; SAVE writes the slot; both return to the
//! main screen. Time stops on this screen.
//!
//! LOAD GAME in the main menu opens the same screen as screen 38, whose icon
//! bar only has LOAD and EXIT TO DOS (FUN_1a3e_0008 picks set 0x26 then).
//!
//! Saves use the original format, so they work in both this and the DOS game.
//! The original asks for a name when saving; this names the save after the
//! game date for now, which works without a keyboard.

use std::path::PathBuf;

use bevy::prelude::*;
use bevy::sprite::Anchor;
use reunion_formats::state::{GameState, SAVE_NAME_LEN};

use crate::focus::{Activated, Hover, hotspot};
use crate::game::Game;
use crate::game_data::{GameData, GameDataHandle};
use crate::hud::{ActionUsed, CONTENT_Y, HoverLabel};
use crate::screen::{GameScreen, picture, place};
use crate::transition::GoTo;

pub struct DiskPlugin;

impl Plugin for DiskPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(OnEnter(GameScreen::DiskOperations), enter)
            .add_systems(OnEnter(GameScreen::LoadGame), enter)
            .add_systems(
                Update,
                update_slot_texts.run_if(
                    in_state(GameScreen::DiskOperations).or_else(in_state(GameScreen::LoadGame)),
                ),
            )
            .add_observer(select_slot)
            .add_observer(press_button)
            .add_observer(use_action);
    }
}

const SLOTS: u8 = 12;
const LIST_X: f32 = 16.0;
const LIST_Y: f32 = 64.0;
const ROW_HEIGHT: f32 = 9.0;
const LIST_COLUMNS: usize = 19;
/// The slot list's click area (hotspot 25 in FUN_1a3e_103b), one row per slot here.
const LIST_LEFT: f32 = 11.0;
const LIST_RIGHT: f32 = 141.0;
const EMPTY_SLOT: &str = "Empty---------------";

const LOAD: u8 = 47;
const SAVE: u8 = 48;
const EXIT_TO_DOS: u8 = 73;

/// The CD player buttons (FUN_1a3e_103b): x, y, width, height, label.
const BUTTONS: [(f32, f32, f32, f32, &str); 4] = [
    (273.0, 155.0, 16.0, 13.0, "Music 1"),
    (290.0, 155.0, 16.0, 13.0, "Music 2"),
    (273.0, 169.0, 34.0, 13.0, "Stop music"),
    (273.0, 183.0, 34.0, 13.0, "Effects on/off"),
];

/// Save slot names (None for empty) and the selected slot, 1-based.
#[derive(Resource)]
struct Slots {
    names: Vec<Option<String>>,
    selected: u8,
}

#[derive(Component)]
struct SlotRow(u8);

#[derive(Component)]
struct SlotText(u8);

#[derive(Component)]
struct CdButton;

/// `SAVE/SPIDYSAV.<n>` in the game folder, like the original.
fn save_path(slot: u8) -> PathBuf {
    game_folder().join("SAVE").join(format!("SPIDYSAV.{slot}"))
}

fn game_folder() -> PathBuf {
    bevy::asset::io::file::FileAssetReader::get_base_path().join("assets")
}

/// The name stored at the start of a save file, if the slot is used.
fn read_slot_name(slot: u8) -> Option<String> {
    let data = std::fs::read(save_path(slot)).ok()?;
    let len = (*data.first()? as usize).min(SAVE_NAME_LEN - 1);
    Some(data.get(1..1 + len)?.iter().map(|&b| b as char).collect())
}

fn enter(mut commands: Commands, asset_server: Res<AssetServer>, screen: Res<State<GameScreen>>) {
    let scoped = DespawnOnExit(*screen.get());
    commands.insert_resource(Slots {
        names: (1..=SLOTS).map(read_slot_name).collect(),
        selected: 1,
    });
    commands.spawn((
        picture(
            asset_server.load("GRAFIKA/DISK.PIC"),
            Vec2::new(0.0, CONTENT_Y),
        ),
        scoped.clone(),
    ));
    for slot in 1..=SLOTS {
        let y = LIST_Y + ROW_HEIGHT * (slot - 1) as f32;
        commands.spawn((
            SlotText(slot),
            Sprite::default(),
            Anchor::TOP_LEFT,
            place(Vec2::new(LIST_X, y), 1.5),
            scoped.clone(),
        ));
        commands.spawn((
            SlotRow(slot),
            HoverLabel("Select slot".to_string()),
            hotspot(
                Rect::new(LIST_LEFT, y, LIST_RIGHT, y + ROW_HEIGHT),
                Hover::Outline,
            ),
            scoped.clone(),
        ));
    }
    for (x, y, width, height, label) in BUTTONS {
        commands.spawn((
            CdButton,
            HoverLabel(label.to_string()),
            hotspot(Rect::new(x, y, x + width, y + height), Hover::Outline),
            scoped.clone(),
        ));
    }
}

fn update_slot_texts(
    slots: Option<Res<Slots>>,
    handle: Res<GameDataHandle>,
    data: Res<Assets<GameData>>,
    mut texts: Query<(&SlotText, &mut Sprite)>,
    added: Query<(), Added<SlotText>>,
    mut images: ResMut<Assets<Image>>,
) {
    let (Some(slots), Some(data)) = (slots, data.get(&handle.0)) else {
        return;
    };
    if !slots.is_changed() && added.is_empty() {
        return;
    }
    for (SlotText(slot), mut sprite) in &mut texts {
        let name = slots.names[*slot as usize - 1]
            .as_deref()
            .unwrap_or(EMPTY_SLOT);
        let colors = data.disk_text[usize::from(*slot == slots.selected)];
        sprite.image = images.add(data.font.render(name, LIST_COLUMNS, colors));
    }
}

fn select_slot(activated: On<Activated>, rows: Query<&SlotRow>, slots: Option<ResMut<Slots>>) {
    if let (Ok(SlotRow(slot)), Some(mut slots)) = (rows.get(activated.0), slots) {
        slots.selected = *slot;
    }
}

fn press_button(activated: On<Activated>, buttons: Query<&HoverLabel, With<CdButton>>) {
    if let Ok(label) = buttons.get(activated.0) {
        info!("{}: there is no sound yet", label.0);
    }
}

fn use_action(
    action: On<ActionUsed>,
    screen: Res<State<GameScreen>>,
    slots: Option<ResMut<Slots>>,
    game: Option<ResMut<Game>>,
    mut exit: MessageWriter<AppExit>,
    mut commands: Commands,
) {
    if !matches!(
        screen.get(),
        GameScreen::DiskOperations | GameScreen::LoadGame
    ) {
        return;
    }
    let Some(mut slots) = slots else { return };
    let slot = slots.selected;
    match action.0 {
        LOAD => match load(slot) {
            Ok(state) => {
                info!("loaded slot {slot}");
                commands.insert_resource(Game(state));
                commands.trigger(GoTo(GameScreen::MainScreen));
            }
            Err(error) => warn!("can't load slot {slot}: {error}"),
        },
        SAVE => {
            let Some(mut game) = game else { return };
            let [year, month, day, hour] = game.0.date();
            let name = format!("{year}-{month}-{day}-{hour}");
            game.0.name = pascal_name(&name);
            match save(slot, &game.0) {
                Ok(()) => {
                    info!("saved slot {slot} as {name}");
                    slots.names[slot as usize - 1] = Some(name);
                    commands.trigger(GoTo(GameScreen::MainScreen));
                }
                Err(error) => warn!("can't save slot {slot}: {error}"),
            }
        }
        EXIT_TO_DOS if cfg!(target_arch = "wasm32") => info!("no DOS to exit to"),
        EXIT_TO_DOS => {
            exit.write(AppExit::Success);
        }
        other => info!("action {other} is not implemented yet"),
    }
}

fn load(slot: u8) -> Result<GameState, String> {
    let data = std::fs::read(save_path(slot)).map_err(|e| e.to_string())?;
    GameState::from_save(&data).map_err(|e| e.to_string())
}

fn save(slot: u8, state: &GameState) -> std::io::Result<()> {
    let path = save_path(slot);
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    // Write to a temporary file first so a failed write can't destroy an old save.
    let temporary = path.with_extension("tmp");
    std::fs::write(&temporary, state.to_save())?;
    std::fs::rename(temporary, path)
}

/// A save name as the original stores it: a Pascal string in 20 bytes.
fn pascal_name(name: &str) -> [u8; SAVE_NAME_LEN] {
    let mut out = [0u8; SAVE_NAME_LEN];
    let bytes: Vec<u8> = name.bytes().take(SAVE_NAME_LEN - 1).collect();
    out[0] = bytes.len() as u8;
    out[1..1 + bytes.len()].copy_from_slice(&bytes);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_are_pascal_strings() {
        let name = pascal_name("2927-8-14-3");
        assert_eq!(name[0], 11);
        assert_eq!(&name[1..12], b"2927-8-14-3");
        assert_eq!(pascal_name(&"x".repeat(40))[0], 19);
    }
}
