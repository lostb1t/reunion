//! Checks against a real copy of the game in `game/`. Skipped when it's absent,
//! since the repository contains no game data.

use std::path::PathBuf;

use reunion_formats::{ani, battle_view, exe::GameExe, icons, map::SurfaceMap, module, pic, sound, state::GameState, text};

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
    let systems = exe.star_systems().unwrap();
    assert_eq!(systems[0].name, "Amnesty");
    assert_eq!(systems[0].moons.len(), 7);
    assert_eq!(systems[0].moons[4], [13]); // New Earth's moon
    let map = SurfaceMap::decode(&map).unwrap();
    assert_eq!((map.width, map.height), (48, 48));
}

#[test]
fn unit_catalog_and_the_new_earth_base() {
    let (Some(prg), Some(init)) = (game_file("GRWAR/REUNION.PRG"), game_file("SAVE/INIT")) else {
        return;
    };
    let exe = GameExe::parse(prg).unwrap();
    let army = exe.unit_categories(1).unwrap();
    assert_eq!(army.len(), 2);
    assert_eq!(army[0].slot, 1);
    let names = |c: &reunion_formats::exe::UnitCategory| {
        c.kinds.iter().map(|k| k.name.clone()).collect::<Vec<_>>()
    };
    assert_eq!(names(&army[0]), ["Hunter", "Fighter", "Destroyer", "Cruiser"]);
    assert_eq!(army[0].kinds[0].capacity, [2, 1, 0, 0]);
    assert_eq!(army[0].kinds[0].name_at, (16, 176));
    assert_eq!(army[0].equipment[0].name, "Laser");
    assert_eq!(army[0].columns, 4);
    assert_eq!(army[1].slot, 2);
    assert_eq!(names(&army[1]), ["Trooper", "Tank", "Aircraft", "Miss tank"]);
    assert_eq!(army[1].columns, 3);
    assert_eq!(
        names(&exe.unit_categories(2).unwrap()[0]),
        ["Sloop", "Trade ship", "Piracy ship", "Galleon"]
    );
    assert_eq!(
        exe.satellite_names().unwrap(),
        ["Satellite", "Spy Sat", "Spy Ship", "Solar sat"]
    );

    use reunion_formats::state::{UnitList, unit};
    let mut state = GameState::new_game(&exe, &init, 1).unwrap();
    assert_eq!(state.unit_count(UnitList::Groups), 0);
    let base = state.unit(UnitList::Bases, 1).unwrap();
    assert_eq!(base[unit::TYPE], 5);
    assert_eq!(&base[unit::NAME + 1..unit::NAME + 17], b"New Earth forces");
    assert_eq!(base[unit::STATUS], 7);
    assert_eq!(state.add_group(), Some(1));
    let group = state.unit(UnitList::Groups, 1).unwrap();
    assert_eq!(&group[unit::NAME..unit::NAME + 10], b"\x09New Group");
    assert_eq!((group[unit::SYSTEM], group[unit::PLANET]), (1, 5));
}

#[test]
fn decodes_every_model() {
    let Some(_) = game_file("VECTORS/V1.VEC") else {
        return;
    };
    for n in 1..=35 {
        let data = game_file(&format!("VECTORS/V{n}.VEC")).unwrap();
        let model = reunion_formats::vec::decode(&data).unwrap_or_else(|e| panic!("V{n}: {e}"));
        assert!(!model.objects.is_empty(), "V{n}");
        for o in &model.objects {
            assert!(o.faces.iter().flatten().all(|&v| v < o.vertices.len()), "V{n}");
        }
    }
}

#[test]
fn colony_model_runs_on_new_earth() {
    let (Some(prg), Some(init)) = (game_file("GRWAR/REUNION.PRG"), game_file("SAVE/INIT")) else {
        return;
    };
    let exe = GameExe::parse(prg).unwrap();
    let original = GameState::new_game(&exe, &init, 1).unwrap();
    let mut state = original.clone();
    state.update_colony(&exe, (1, 5, 0), 5);
    // SAVE/INIT's own numbers are stale (1100 workers in a 900-worker
    // command centre), so check the model's invariants instead.
    for b in state.buildings() {
        let kind = exe.building_type(b[0]).unwrap();
        if b[8] != 0 {
            assert!(u16::from_le_bytes([b[9], b[10]]) <= kind.workers);
            assert!(b[13] <= 100);
        }
    }
    assert!(state.buildings().iter().filter(|b| b[8] != 0).count() > 10);
}

#[test]
#[ignore]
fn print_building_types() {
    let Some(prg) = game_file("GRWAR/REUNION.PRG") else { return };
    let exe = GameExe::parse(prg).unwrap();
    for t in 1..=25 {
        let k = exe.building_type(t).unwrap();
        println!("{t:2} {:14} inv {:2} cat {:2} workers {:5} energy {:6} prod {:5} cost {:6} prio {}", k.name, k.invention, k.category, k.workers, k.energy, k.production, k.cost, k.priority);
    }
}

#[test]
fn simulation_runs_for_months() {
    use reunion_formats::sim::Texts;
    let (Some(prg), Some(init), Some(messages), Some(kitalal)) = (
        game_file("GRWAR/REUNION.PRG"),
        game_file("SAVE/INIT"),
        game_file("TEXT/MESSAGE.TXT"),
        game_file("TEXT/KITALAL.TXT"),
    ) else {
        return;
    };
    let exe = GameExe::parse(prg).unwrap();
    let texts = Texts::parse(&messages, &kitalal);
    assert_eq!(texts.inventions[4], "The Miner Station is invented");
    let mut state = GameState::new_game(&exe, &init, 7).unwrap();
    let pop = |s: &GameState| {
        let r = s.body(1, 5).unwrap();
        u32::from_le_bytes([r[0xd], r[0xe], r[0xf], r[0x10]])
    };
    // A mine being built on New Earth.
    let mine = exe.building_type(4).unwrap();
    let n = state.add_building(&mine, 4, (1, 5, 0), (1, 1), state.body(1, 5).unwrap()[0x15], |n| n / 2).unwrap();
    let (money, people) = (state.money(), pop(&state));
    let mut seed = 12345u32;
    let mut random = |n: u16| {
        seed = seed.wrapping_mul(0x0808_8405).wrapping_add(1);
        ((u64::from(seed) * u64::from(n)) >> 32) as u16
    };
    let mut reports = Vec::new();
    for _ in 0..24 * 60 {
        state.advance_hour();
        reports.extend(state.simulate_hour(&exe, &texts, &mut random));
    }
    println!("money {money} -> {}, people {people} -> {}", state.money(), pop(&state));
    println!("mine left {}", state.buildings()[n - 1][6]);
    for r in &reports {
        println!("{r:?}");
    }
    assert_eq!(state.buildings()[n - 1][6], 0, "the mine got built");
    assert!(state.money() > money, "taxes came in");
    assert!(pop(&state) > 1000);
}

#[test]
fn space_battle_runs_to_the_end() {
    use reunion_formats::{battle::SpaceBattle, state::UnitList};
    let (Some(prg), Some(init)) = (game_file("GRWAR/REUNION.PRG"), game_file("SAVE/INIT")) else {
        return;
    };
    let exe = GameExe::parse(prg).unwrap();
    let mut state = GameState::new_game(&exe, &init, 3).unwrap();
    // A group of 40 hunters with lasers at Jade, the Jaanosians' planet,
    // at war with them.
    let n = state.add_group().unwrap();
    let group = state.unit_mut(UnitList::Groups, n).unwrap();
    (group[0x13], group[0x14], group[0x15], group[0x16]) = (1, 7, 0, 2);
    group[0x1d..0x1f].copy_from_slice(&40i16.to_le_bytes());
    group[0x1f..0x21].copy_from_slice(&40i16.to_le_bytes());
    state.set_standing(2, reunion_formats::aliens::AT_WAR);
    let mut seed = 99u32;
    let mut random = |n: u16| {
        seed = seed.wrapping_mul(0x0808_8405).wrapping_add(1);
        ((u64::from(seed) * u64::from(n)) >> 32) as u16
    };
    let mut battle = SpaceBattle::new(&state, &exe, (1, 7, 0), &mut random).unwrap();
    assert_eq!(battle.flying[0], 40);
    assert!(battle.flying[1] > 0, "the Jaanosians' fleet and defences");
    let mut frames = 0;
    while !battle.over && frames < 200_000 {
        battle.frame(false, &mut random);
        frames += 1;
    }
    assert!(battle.over, "over after {frames} frames");
    for side in &battle.sides {
        for ship in side {
            assert!(ship.x < 200 && ship.y < 200, "{ship:?}");
        }
    }
    let losses = battle.finish(&mut state, &exe);
    println!("won {} in {frames} frames, losses {losses:?}", battle.won());
    let left = i16::from_le_bytes(state.unit(UnitList::Groups, n).unwrap()[0x1d..0x1f].try_into().unwrap());
    assert_eq!(i64::from(left) + i64::from(losses[0][0]), 40);
}

#[test]
fn ground_battle_runs_to_the_end() {
    use reunion_formats::{ground::GroundBattle, state::UnitList};
    let (Some(prg), Some(init)) = (game_file("GRWAR/REUNION.PRG"), game_file("SAVE/INIT")) else {
        return;
    };
    let exe = GameExe::parse(prg).unwrap();
    let mut state = GameState::new_game(&exe, &init, 5).unwrap();
    let n = state.add_group().unwrap();
    let group = state.unit_mut(UnitList::Groups, n).unwrap();
    (group[0x13], group[0x14], group[0x15], group[0x16]) = (1, 7, 0, 1);
    for (k, count) in [(0, 60i16), (1, 20)] {
        let at = 0x45 + 10 * k;
        group[at..at + 2].copy_from_slice(&count.to_le_bytes());
        group[at + 2..at + 4].copy_from_slice(&count.to_le_bytes());
    }
    state.set_standing(2, reunion_formats::aliens::AT_WAR);
    let mut battle = GroundBattle::new(&state, &exe, (1, 7, 0), true);
    println!("forces {:?}", battle.forces);
    assert!(!battle.units[0].is_empty());
    battle.deploy();
    let mut seed = 7u32;
    let mut random = |n: u16| {
        seed = seed.wrapping_mul(0x0808_8405).wrapping_add(1);
        ((u64::from(seed) * u64::from(n)) >> 32) as u16
    };
    for i in 0..battle.units[0].len() {
        if let Some(t) = (0..battle.units[1].len()).next() {
            battle.order_attack(i, t);
        }
    }
    let mut frames = 0;
    while !battle.over && frames < 100_000 {
        battle.frame(false, &mut random);
        frames += 1;
    }
    println!("over {} after {frames}: won {}", battle.over, battle.won());
    assert!(battle.over);
    let losses = battle.finish(&mut state, &exe);
    println!("losses {losses:?}");
}

#[test]
fn long_game_runs() {
    use reunion_formats::sim::Texts;
    let (Some(prg), Some(init), Some(messages), Some(kitalal)) = (
        game_file("GRWAR/REUNION.PRG"),
        game_file("SAVE/INIT"),
        game_file("TEXT/MESSAGE.TXT"),
        game_file("TEXT/KITALAL.TXT"),
    ) else {
        return;
    };
    let exe = GameExe::parse(prg).unwrap();
    let texts = Texts::parse(&messages, &kitalal);
    let mut state = GameState::new_game(&exe, &init, 11).unwrap();
    // Hire everyone well, so the story moves.
    for (at, v) in [(0x95b6u16, 3u16), (0x95b8, 3), (0x95ba, 3), (0x95bc, 3), (0x95a4, 80), (0x95a6, 80), (0x95a8, 80), (0x95aa, 80)] {
        state.set_word(at, v);
    }
    let mut seed = 1u32;
    let mut random = |n: u16| {
        seed = seed.wrapping_mul(0x0808_8405).wrapping_add(1);
        ((u64::from(seed) * u64::from(n)) >> 32) as u16
    };
    let mut events = 0;
    for _ in 0..20_000 {
        state.advance_hour();
        events += state.simulate_hour(&exe, &texts, &mut random).len();
        state.travel_hour();
        let (e, _) = state.aliens_hour(&exe, &texts, &mut random);
        events += e.len();
    }
    println!("{events} events, money {}", state.money());
}

fn files_with_extension(extension: &str) -> Vec<PathBuf> {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../game");
    let mut files: Vec<PathBuf> = std::fs::read_dir(&root)
        .into_iter()
        .flatten()
        .flatten()
        .filter(|e| e.path().is_dir())
        .flat_map(|dir| std::fs::read_dir(dir.path()).into_iter().flatten().flatten())
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|e| e.eq_ignore_ascii_case(extension)))
        .collect();
    files.sort();
    files
}

#[test]
fn decodes_every_sound() {
    let files = files_with_extension("smp");
    if files.is_empty() {
        return;
    }
    for path in &files {
        let smp = sound::Smp::parse(&std::fs::read(path).unwrap())
            .unwrap_or_else(|e| panic!("{}: {e}", path.display()));
        assert!((4000..=45000).contains(&smp.sample_rate), "{}", path.display());
    }
    assert_eq!(files.len(), 154);
}

#[test]
fn plays_every_module() {
    // The cutscenes' .MOD files and the game's ANIM/*.SPD tracks (not the
    // 7-byte SAVE/SETUP.SPD).
    let mut files = files_with_extension("mod");
    files.extend(files_with_extension("spd").into_iter().filter(|p| p.metadata().is_ok_and(|m| m.len() > 2000)));
    if files.is_empty() {
        return;
    }
    for path in &files {
        let song = module::Module::parse(&std::fs::read(path).unwrap())
            .unwrap_or_else(|e| panic!("{}: {e}", path.display()));
        let mut player = module::Player::new(song, 22050);
        // Twenty seconds.
        let mut out = vec![0.0; 2 * 22050 * 20];
        player.render(&mut out);
        let peak = out.iter().fold(0.0f32, |m, s| m.max(s.abs()));
        let rms = (out.iter().map(|s| s * s).sum::<f32>() / out.len() as f32).sqrt();
        println!("{}: peak {peak:.2} rms {rms:.3}", path.display());
        assert!(out.iter().all(|s| s.is_finite()));
        // NO.SPD is the "music off" track: one empty pattern.
        let quiet = path.file_name().is_some_and(|n| n.eq_ignore_ascii_case("NO.SPD"));
        assert!(quiet || rms > 0.01, "{} is silent", path.display());
    }
    assert_eq!(files.len(), 14 + 13);
}

#[test]
fn decodes_every_animation() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../game");
    let mut files = files_with_extension("ani");
    for sub in ["WAR/WAR", "WAR/GRWAR"] {
        files.extend(
            std::fs::read_dir(root.join(sub))
                .into_iter()
                .flatten()
                .flatten()
                .map(|e| e.path())
                .filter(|p| p.extension().is_some_and(|e| e.eq_ignore_ascii_case("ani"))),
        );
    }
    if files.is_empty() {
        return;
    }
    for path in &files {
        let ani = ani::Ani::parse(&std::fs::read(path).unwrap())
            .unwrap_or_else(|e| panic!("{}: {e}", path.display()));
        for k in 1..=ani.chunks.len() {
            ani.frame(k);
        }
    }
    assert_eq!(files.len(), 98);
}

#[test]
fn battle_view_plays_every_clip() {
    let (Some(exe), Some(def)) = (game_file("GRWAR/REUNION.PRG"), game_file("WAR/ANIM/ANIM.DEF")) else {
        return;
    };
    let exe = GameExe::parse(exe).unwrap();
    let mut seed = 7u32;
    let mut random = |n: u16| {
        seed = seed.wrapping_mul(1_103_515_245).wrapping_add(12345);
        ((seed >> 16) % u32::from(n.max(1))) as u16
    };
    let (mut view, first) = battle_view::BattleView::new(&def, &exe, &mut random);
    let mut seen = std::collections::HashSet::new();
    let mut events = first;
    for _ in 0..200_000 {
        for event in &events {
            if let battle_view::ViewEvent::Draw { animation, frame } = *event {
                let place = exe.clip_placement(animation);
                assert!((1..=place.frames).contains(&frame), "SA{animation} frame {frame}");
                assert!(u32::from(place.x) + u32::from(place.width) <= 320);
                seen.insert(animation);
            }
        }
        events = view.frame(&mut random);
    }
    // Clip 1 can only open a battle; the other 24 all come around.
    assert!(seen.len() >= 24);
}

#[test]
fn pilot_raids_a_convoy() {
    use reunion_formats::{pub_people::PILOT, sim::Texts, story::Event};
    let (Some(prg), Some(init), Some(messages), Some(kitalal), Some(pirate)) = (
        game_file("GRWAR/REUNION.PRG"),
        game_file("SAVE/INIT"),
        game_file("TEXT/MESSAGE.TXT"),
        game_file("TEXT/KITALAL.TXT"),
        game_file("TEXT/PIRATE.TXT"),
    ) else {
        return;
    };
    let exe = GameExe::parse(prg).unwrap();
    let texts = Texts::parse(&messages, &kitalal).with_pirate(&pirate);
    assert!(texts.pirate[10].contains("Lyrae"));
    let mut state = GameState::new_game(&exe, &init, 1).unwrap();
    let mut seed = 5u32;
    let mut random = |n: u16| {
        seed = seed.wrapping_mul(0x0808_8405).wrapping_add(1);
        ((u64::from(seed) * u64::from(n)) >> 32) as u16
    };
    state.start_offers();
    let mut hours = 0;
    let mut announced = 0;
    while state.current_offer() == 0 && hours < 24 * 400 {
        state.advance_hour();
        announced += state
            .pub_hour(&exe, &texts, &mut random)
            .iter()
            .filter(|e| matches!(e, Event::Message(m) if texts.pirate.contains(m)))
            .count();
        hours += 1;
    }
    let offer = state.current_offer();
    assert!(offer > 0, "no convoy after {hours} hours");
    assert!(announced > 0, "its message went to the log");
    // The pilot turns up in the pub later in the story.
    state.set_byte(0x11f + 27 * 9 + 0x0f, 2);
    state.send_person(PILOT, 3, offer as u8 + 10);
    let mut events = Vec::new();
    for _ in 0..3 {
        events.extend(state.pub_hour(&exe, &texts, &mut random));
    }
    let offer_after = state.trade_offer(offer);
    assert_eq!(offer_after.until[0], 0, "the convoy is gone");
    assert!(events.iter().any(|e| matches!(e, Event::Message(m) if !m.is_empty())));
    println!("{offer}: {:?} {events:?}", state.trade_offer(offer));
}
