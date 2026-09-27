//! Checks against a real copy of the game in `game/`. Skipped when it's absent,
//! since the repository contains no game data.

use std::path::PathBuf;

use reunion_formats::{exe::GameExe, icons, map::SurfaceMap, pic, state::GameState, text};

fn game_file(path: &str) -> Option<Vec<u8>> {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../game");
    std::fs::read(root.join(path)).ok()
}

#[test]
fn decodes_every_picture() {
    let Some(_) = game_file("GRAFIKA/MAIN.PIC") else {
        return;
    };
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../game");
    let mut decoded = 0;
    for dir in std::fs::read_dir(&root)
        .unwrap()
        .flatten()
        .filter(|e| e.path().is_dir())
    {
        for file in std::fs::read_dir(dir.path()).unwrap().flatten() {
            let path = file.path();
            if path
                .extension()
                .is_some_and(|e| e.eq_ignore_ascii_case("pic"))
            {
                let data = std::fs::read(&path).unwrap();
                if data.starts_with(b"SpidyGfx") {
                    pic::decode(&data).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
                    decoded += 1;
                }
            }
        }
    }
    assert_eq!(decoded, 458);
}

#[test]
fn reads_exe_tables() {
    let Some(data) = game_file("GRWAR/REUNION.PRG") else {
        return;
    };
    let exe = GameExe::parse(data).unwrap();
    assert_eq!(exe.label(1).unwrap(), "DISK OPERATIONS");
    assert_eq!(
        exe.icon_set(1).unwrap(),
        [23, 2, 13, 34, 41, 4, 14, 45, 74, 1]
    );
    assert_eq!(exe.icon_for(41).unwrap(), 37);
    assert_eq!(exe.charset_order().unwrap().len(), 78);
    let room = exe.main_room().unwrap();
    assert_eq!(room[7].label, "PLANET MAIN");
    assert_eq!(
        (room[7].x, room[7].y, room[7].width, room[7].height),
        (73, 157, 190, 43)
    );
}

#[test]
fn decodes_icons_and_text() {
    let Some(icon_all) = game_file("ICON/ICON.ALL") else {
        return;
    };
    assert_eq!(icons::decode_icons(&icon_all).len(), 68);
    let messages = text::decrypt_lines(&game_file("TEXT/MESSAGE.TXT").unwrap());
    assert!(messages[2].starts_with(b"You cannot transfer the Miner Station"));
}

#[test]
fn builds_a_new_game_that_round_trips() {
    let (Some(exe), Some(init)) = (game_file("GRWAR/REUNION.PRG"), game_file("SAVE/INIT")) else {
        return;
    };
    let exe = GameExe::parse(exe).unwrap();
    let state = GameState::new_game(&exe, &init, 42).unwrap();

    let save = state.to_save();
    assert_eq!(save.len(), reunion_formats::state::SAVE_LEN);
    assert!(GameState::from_save(&save).unwrap() == state);

    let systems = state.star_systems();
    let first: Vec<_> = systems
        .iter()
        .map(|s| s.bodies[0].name.trim_end())
        .collect();
    assert_eq!(
        first,
        [
            "Amnesty 1",
            "Phoenix 1",
            "Mirach 1",
            "Antares 1",
            "Orionis 1",
            "Lyrae 1",
            "Rigel 1",
            "Mercury"
        ]
    );
    assert_eq!(systems.iter().map(|s| s.bodies.len()).sum::<usize>(), 142);
    assert!(systems[0].bodies.iter().any(|b| b.name == "New Earth"));

    assert_eq!(state.inventions()[0].name, "Nuclear gen");
    assert_eq!(state.races().last().unwrap().name, "Earthlings");
    assert_eq!(state.buildings().len(), 23);
    assert_eq!(state.money(), 120_000);
    assert_eq!(state.date(), [2927, 8, 13, 23]);
    assert!((6000..6400).contains(&state.word(0x5d58).unwrap()));
}

#[test]
fn reads_planet_surfaces() {
    let (Some(exe), Some(map)) = (game_file("GRWAR/REUNION.PRG"), game_file("MAP/MAP1_0.MAP"))
    else {
        return;
    };
    let exe = GameExe::parse(exe).unwrap();
    // New Earth is planet type 1: terrain 1, 80 static tiles, 64-row FELSZ, 192-row FANIM.
    assert_eq!(exe.terrain_for_type(1).unwrap(), 1);
    assert_eq!(exe.static_tiles(1).unwrap(), 80);
    assert_eq!(exe.sheet_rows(1).unwrap(), (64, 192));
    assert_eq!(exe.screen_trigger(20).unwrap(), Some(41));
    assert_eq!(exe.screen_trigger(14).unwrap(), None);
    let map = SurfaceMap::decode(&map).unwrap();
    assert_eq!((map.width, map.height), (48, 48));
}
