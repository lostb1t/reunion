//! Tables, icons and the font, read from the player's own game files.
//!
//! Loaded as one asset from `GRWAR/REUNION.PRG`; the loader also reads
//! ICON.ALL (icons), ICONMAIN.PIC (their palette) and CHARSET1.PIC (font).

use bevy::asset::io::Reader;
use bevy::asset::{AssetLoader, LoadContext};
use bevy::prelude::*;
use reunion_formats::exe::{
    ExeError, GameExe, RoomHotspot, SCREENS, SystemLayout, TERRAINS, UnitCategory,
};
use reunion_formats::icons::{ICON_HEIGHT, ICON_WIDTH, decode_icons};
use reunion_formats::map::{MapError, SurfaceMap};
use reunion_formats::pic::{self, PicError};
use reunion_formats::state::{GameState, StateError};

use crate::pic::rgba_image;

pub struct GameDataPlugin;

impl Plugin for GameDataPlugin {
    fn build(&self, app: &mut App) {
        app.init_asset::<GameData>()
            .init_asset::<PlanetMap>()
            .register_asset_loader(GameDataLoader)
            .register_asset_loader(PlanetMapLoader)
            .add_systems(Startup, load_game_data);
    }
}

/// Number of entries in the master label table.
const ACTIONS: u8 = 78;

#[derive(Asset, TypePath)]
pub struct GameData {
    /// Hover label per action id (index 0 is unused).
    pub labels: Vec<String>,
    /// Icon per action id, if it has one.
    pub action_icons: Vec<Option<Handle<Image>>>,
    /// Action ids shown in each icon bar set.
    pub icon_sets: Vec<Vec<u8>>,
    pub main_room: Vec<RoomHotspot>,
    /// Per original screen number (1-38): the action that opens it.
    pub screen_triggers: Vec<Option<u8>>,
    /// Planet type -> terrain number.
    pub terrain_for_type: Vec<u16>,
    /// Per terrain number (1-11).
    pub terrains: Vec<Terrain>,
    /// Commander candidates' four text lines (TEXT/SZ_FACE.RAW), 3 per category.
    pub commanders: Vec<[String; 4]>,
    /// Salaries, levels and skills for hiring commanders.
    pub hire: crate::commanders::HireTables,
    /// Star system names and moons, system n at index n - 1.
    pub star_systems: Vec<SystemLayout>,
    /// The state a new game starts from (REUNION.PRG + SAVE/INIT).
    pub new_game: GameState,
    pub font: Font,
    /// Slot list text colors on the disk screen (normal, selected): DISK.PIC
    /// palette entries 112-114 and 115-117, which FUN_405f_12e4 and _1317
    /// reach by adding -0x50 and -0x4d to the font pixels.
    pub disk_text: [[[u8; 4]; 3]; 2],
    /// What each unit type (index 1-5) carries.
    pub unit_categories: Vec<Vec<UnitCategory>>,
    /// Satellite kinds a satellite carrier carries.
    pub satellites: Vec<String>,
    /// Short star names (DS:0x5baf + 7 * system), system n at index n - 1.
    pub short_star_names: Vec<String>,
    /// What you and your staff say (screen 24).
    pub talk: StaffTalkTexts,
    /// Conversations with aliens (TEXT/KERDES<n>.AT questions and
    /// VALASZ<n>.AT answers), conversation n at index n; empty where the
    /// game has none.
    pub alien_talks: Vec<(Vec<String>, Vec<String>)>,
    /// Talks in the pub (TEXT/KERDES<n>.LOC and VALASZ<n>.LOC), person n at
    /// index n.
    pub pub_talks: Vec<(Vec<String>, Vec<String>)>,
    /// Per invention, INFO-BUY's title and six lines (TEXT/SZ_TALAL.RAW:
    /// 31-byte Pascal strings).
    pub descriptions: Vec<[String; 7]>,
    /// The executable, for strings read by only one screen.
    pub exe: GameExe,
    /// TEXT/MESSAGE.TXT and TEXT/KITALAL.TXT, for what happens over time.
    pub sim_texts: reunion_formats::sim::Texts,
}

/// TEXT/KERDES1.SP (questions), RKERDES1.SP (their short forms),
/// VALASZ1.SP (answers) and AJANLAS.SP (reasons), line n at index n - 1.
pub struct StaffTalkTexts {
    pub questions: Vec<String>,
    pub short_questions: Vec<String>,
    pub answers: Vec<String>,
    pub reasons: Vec<String>,
}

/// A planet surface tile set: FELSZ<n>.PIC (static) and FANIM<n>.PIC (animated).
#[derive(Clone, Copy, Default)]
pub struct Terrain {
    pub habitable: bool,
    pub static_tiles: u16,
    pub fanim_rows: u16,
}

/// CHARSET1.PIC: 6x8 glyphs in 16x8 cells, 20 per row.
pub struct Font {
    order: Vec<u8>,
    pixels: Vec<u8>,
    width: usize,
}

pub const GLYPH_WIDTH: usize = 6;
pub const GLYPH_HEIGHT: usize = 8;

impl Font {
    /// Renders `text` padded or cut to `columns` characters. Glyph pixels are
    /// 0 (background), 1 and 2 and are colored with `colors`, like the original
    /// adds a color offset to them (REUNION.PRG FUN_405f_1056).
    pub fn render(&self, text: &str, columns: usize, colors: [[u8; 4]; 3]) -> Image {
        let width = columns * GLYPH_WIDTH;
        let mut rgba = vec![0u8; width * GLYPH_HEIGHT * 4];
        let space = self.glyph(b' ');
        let chars = text
            .bytes()
            .map(|c| self.glyph(c))
            .chain(std::iter::repeat(space));
        for (column, glyph) in chars.take(columns).enumerate() {
            let (gx, gy) = ((glyph % 20) * 16, (glyph / 20) * GLYPH_HEIGHT);
            for y in 0..GLYPH_HEIGHT {
                for x in 0..GLYPH_WIDTH {
                    let value = self
                        .pixels
                        .get((gy + y) * self.width + gx + x)
                        .copied()
                        .unwrap_or(0);
                    let out = (y * width + column * GLYPH_WIDTH + x) * 4;
                    rgba[out..out + 4].copy_from_slice(&colors[(value as usize).min(2)]);
                }
            }
        }
        rgba_image(width as u32, GLYPH_HEIGHT as u32, rgba)
    }

    fn glyph(&self, c: u8) -> usize {
        let lookup = |c: u8| self.order.iter().position(|&o| o == c);
        lookup(c)
            .or_else(|| lookup(c.to_ascii_uppercase()))
            .or_else(|| lookup(b' '))
            .unwrap_or(0)
    }
}

#[derive(Resource)]
pub struct GameDataHandle(pub Handle<GameData>);

/// Pictures that systems read pixels from to build other images. Holding the
/// handles keeps them loaded: a handle dropped right after `load` lets the
/// asset be freed as soon as it finishes loading.
#[derive(Resource)]
pub struct SharedPictures {
    pub text_strip: Handle<Image>,
    pub icon_frames: Handle<Image>,
    pub planet_frame: Handle<Image>,
}

fn load_game_data(mut commands: Commands, asset_server: Res<AssetServer>) {
    commands.insert_resource(GameDataHandle(asset_server.load("GRWAR/REUNION.PRG")));
    commands.insert_resource(SharedPictures {
        text_strip: asset_server.load("GRAFIKA/TEXT.PIC"),
        icon_frames: asset_server.load("ICON/ICONMAIN.PIC"),
        planet_frame: asset_server.load("GRAFIKA/DESIGNER.PIC"),
    });
}

#[derive(TypePath)]
struct GameDataLoader;

#[derive(Debug, thiserror::Error)]
enum GameDataError {
    #[error("io: {0}")]
    Io(#[from] std::io::Error),
    #[error("reading {0}: {1}")]
    Read(&'static str, String),
    #[error("REUNION.PRG: {0}")]
    Exe(#[from] ExeError),
    #[error("picture: {0}")]
    Pic(#[from] PicError),
    #[error("new game: {0}")]
    State(#[from] StateError),
}

impl AssetLoader for GameDataLoader {
    type Asset = GameData;
    type Settings = ();
    type Error = GameDataError;

    async fn load(
        &self,
        reader: &mut dyn Reader,
        _settings: &(),
        load_context: &mut LoadContext<'_>,
    ) -> Result<GameData, GameDataError> {
        let mut bytes = Vec::new();
        reader.read_to_end(&mut bytes).await?;
        let exe = GameExe::parse(bytes)?;

        let mut read = async |path: &'static str| {
            load_context
                .read_asset_bytes(path)
                .await
                .map_err(|e| GameDataError::Read(path, e.to_string()))
        };
        let icon_all = read("ICON/ICON.ALL").await?;
        let init = read("SAVE/INIT").await?;
        let faces = read("TEXT/SZ_FACE.RAW").await?;
        let descriptions = read("TEXT/SZ_TALAL.RAW").await?;
        let sim_texts =
            reunion_formats::sim::Texts::parse(&read("TEXT/MESSAGE.TXT").await?, &read("TEXT/KITALAL.TXT").await?);
        let mut text_lines = async |path: &'static str| -> Result<Vec<String>, GameDataError> {
            Ok(reunion_formats::text::decrypt_lines(&read(path).await?)
                .into_iter()
                .map(|l| l.into_iter().map(char::from).collect())
                .collect())
        };
        const ALIEN_TALKS: [(&str, &str); 10] = [
            ("TEXT/KERDES1.AT", "TEXT/VALASZ1.AT"),
            ("TEXT/KERDES2.AT", "TEXT/VALASZ2.AT"),
            ("TEXT/KERDES3.AT", "TEXT/VALASZ3.AT"),
            ("TEXT/KERDES4.AT", "TEXT/VALASZ4.AT"),
            ("TEXT/KERDES5.AT", "TEXT/VALASZ5.AT"),
            ("TEXT/KERDES6.AT", "TEXT/VALASZ6.AT"),
            ("TEXT/KERDES7.AT", "TEXT/VALASZ7.AT"),
            ("TEXT/KERDES8.AT", "TEXT/VALASZ8.AT"),
            ("TEXT/KERDES9.AT", "TEXT/VALASZ9.AT"),
            ("TEXT/KERDES10.AT", "TEXT/VALASZ10.AT"),
        ];
        let mut alien_talks = vec![(Vec::new(), Vec::new())];
        for (questions, answers) in ALIEN_TALKS {
            alien_talks.push((text_lines(questions).await.unwrap_or_default(), text_lines(answers).await.unwrap_or_default()));
        }
        const PUB_TALKS: [(&str, &str); 10] = [
            ("TEXT/KERDES1.LOC", "TEXT/VALASZ1.LOC"),
            ("TEXT/KERDES2.LOC", "TEXT/VALASZ2.LOC"),
            ("TEXT/KERDES3.LOC", "TEXT/VALASZ3.LOC"),
            ("TEXT/KERDES4.LOC", "TEXT/VALASZ4.LOC"),
            ("TEXT/KERDES5.LOC", "TEXT/VALASZ5.LOC"),
            ("TEXT/KERDES6.LOC", "TEXT/VALASZ6.LOC"),
            ("TEXT/KERDES7.LOC", "TEXT/VALASZ7.LOC"),
            ("TEXT/KERDES8.LOC", "TEXT/VALASZ8.LOC"),
            ("TEXT/KERDES9.LOC", "TEXT/VALASZ9.LOC"),
            ("TEXT/KERDES10.LOC", "TEXT/VALASZ10.LOC"),
        ];
        let mut pub_talks = vec![(Vec::new(), Vec::new())];
        for (questions, answers) in PUB_TALKS {
            pub_talks.push((text_lines(questions).await.unwrap_or_default(), text_lines(answers).await.unwrap_or_default()));
        }
        let talk = StaffTalkTexts {
            questions: text_lines("TEXT/KERDES1.SP").await?,
            short_questions: text_lines("TEXT/RKERDES1.SP").await?,
            answers: text_lines("TEXT/VALASZ1.SP").await?,
            reasons: text_lines("TEXT/AJANLAS.SP").await?,
        };
        let disk_palette = pic::decode(&read("GRAFIKA/DISK.PIC").await?)?.palette;
        let color = |i: usize| {
            let [r, g, b] = disk_palette[i];
            [r, g, b, 255]
        };
        let disk_text = [
            [color(112), color(113), color(114)],
            [color(115), color(116), color(117)],
        ];
        let icon_palette = pic::decode(&read("ICON/ICONMAIN.PIC").await?)?.palette;
        let charset = pic::decode(&read("GRAFIKA/CHARSET1.PIC").await?)?;

        let icons: Vec<_> = decode_icons(&icon_all)
            .into_iter()
            .enumerate()
            .map(|(i, pixels)| {
                let rgba = pixels
                    .iter()
                    .flat_map(|&p| {
                        let [r, g, b] = icon_palette[p as usize];
                        [r, g, b, 255]
                    })
                    .collect();
                load_context.add_labeled_asset(
                    format!("icon{i}"),
                    rgba_image(ICON_WIDTH as u32, ICON_HEIGHT as u32, rgba),
                )
            })
            .collect();

        let mut labels = vec![String::new()];
        let mut action_icons = vec![None];
        for action in 1..ACTIONS {
            labels.push(exe.label(action)?);
            // Icon 0 means the action has none (FUN_3a1b_00bf skips it).
            let icon = exe.icon_for(action)? as usize;
            action_icons.push((icon != 0).then(|| icons.get(icon).cloned()).flatten());
        }

        let unit_categories = (0..=5)
            .map(|t| exe.unit_categories(t))
            .collect::<Result<_, _>>()?;
        let satellites = exe.satellite_names()?;
        let short_star_names = (1..=8)
            .map(|s| exe.ds_string(0x5baf + 7 * s).unwrap_or_default())
            .collect();
        let pascal = |b: &[u8]| -> String {
            let n = (b[0] as usize).min(b.len() - 1);
            b[1..=n].iter().map(|&c| c as char).collect()
        };
        let descriptions = descriptions
            .chunks_exact(7 * 31)
            .map(|r| std::array::from_fn(|k| pascal(&r[k * 31..(k + 1) * 31])))
            .collect();
        Ok(GameData {
            talk,
            alien_talks,
            pub_talks,
            descriptions,
            unit_categories,
            satellites,
            short_star_names,
            labels,
            action_icons,
            // One icon bar set per screen, indexed by screen number.
            icon_sets: (0..=SCREENS)
                .map(|s| exe.icon_set(s))
                .collect::<Result<_, _>>()?,
            main_room: exe.main_room()?,
            star_systems: exe.star_systems()?,
            commanders: crate::commanders::parse_candidates(&faces),
            hire: crate::commanders::HireTables::read(&exe).ok_or(ExeError::OutOfRange)?,
            screen_triggers: (0..=SCREENS)
                .map(|s| exe.screen_trigger(s))
                .collect::<Result<_, _>>()?,
            terrain_for_type: (0..=12)
                .map(|t| exe.terrain_for_type(t))
                .collect::<Result<_, _>>()?,
            terrains: (0..=TERRAINS)
                .map(|t| {
                    let (_, fanim_rows) = exe.sheet_rows(t)?;
                    Ok(Terrain {
                        habitable: exe.habitable(t)?,
                        static_tiles: exe.static_tiles(t)?,
                        fanim_rows,
                    })
                })
                .collect::<Result<_, ExeError>>()?,
            // The original seeds its one random start value from the clock;
            // a fixed seed keeps new games reproducible for now.
            new_game: GameState::new_game(&exe, &init, 0)?,
            disk_text,
            font: Font {
                order: exe.charset_order()?,
                width: charset.width as usize,
                pixels: charset.pixels,
            },
            exe,
            sim_texts,
        })
    }

    fn extensions(&self) -> &[&str] {
        &["PRG", "prg"]
    }
}

/// A planet surface map (`MAP/*.MAP`).
#[derive(Asset, TypePath)]
pub struct PlanetMap(pub SurfaceMap);

#[derive(TypePath)]
struct PlanetMapLoader;

#[derive(Debug, thiserror::Error)]
enum PlanetMapError {
    #[error("io: {0}")]
    Io(#[from] std::io::Error),
    #[error("map: {0}")]
    Map(#[from] MapError),
}

impl AssetLoader for PlanetMapLoader {
    type Asset = PlanetMap;
    type Settings = ();
    type Error = PlanetMapError;

    async fn load(
        &self,
        reader: &mut dyn Reader,
        _settings: &(),
        _load_context: &mut LoadContext<'_>,
    ) -> Result<PlanetMap, PlanetMapError> {
        let mut bytes = Vec::new();
        reader.read_to_end(&mut bytes).await?;
        Ok(PlanetMap(SurfaceMap::decode(&bytes)?))
    }

    fn extensions(&self) -> &[&str] {
        &["MAP", "map"]
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use bevy::asset::AssetPlugin;

    use super::*;

    /// Loads the game data headless (no window, no GPU) and checks the images
    /// the HUD and planet screen draw stay available frame after frame.
    /// Skipped without a copy of the game in crates/reunion/assets.
    #[test]
    fn game_data_and_pictures_stay_loaded() {
        let root = concat!(env!("CARGO_MANIFEST_DIR"), "/assets");
        if !std::path::Path::new(root)
            .join("GRWAR/REUNION.PRG")
            .exists()
        {
            return;
        }
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, AssetPlugin::default()))
            .init_asset::<Image>()
            .add_plugins((crate::pic::PicPlugin, GameDataPlugin));

        let loaded = |app: &mut App| {
            let world = app.world();
            let handle = &world.resource::<GameDataHandle>().0;
            let pictures = world.resource::<SharedPictures>();
            let images = world.resource::<Assets<Image>>();
            world.resource::<Assets<GameData>>().get(handle).is_some()
                && [
                    &pictures.text_strip,
                    &pictures.icon_frames,
                    &pictures.planet_frame,
                ]
                .iter()
                .all(|h| images.get(*h).is_some())
        };
        for _ in 0..500 {
            app.update();
            if loaded(&mut app) {
                break;
            }
            std::thread::sleep(Duration::from_millis(10));
        }
        assert!(loaded(&mut app), "game data and pictures didn't load");

        // Keep running: nothing may be unloaded again.
        for _ in 0..200 {
            app.update();
        }
        assert!(loaded(&mut app), "pictures were unloaded");
        let world = app.world();
        let data = world
            .resource::<Assets<GameData>>()
            .get(&world.resource::<GameDataHandle>().0)
            .unwrap();
        let images = world.resource::<Assets<Image>>();
        // Every in-game screen has its icon bar set (PLANET MAIN is screen 20),
        // except the conversations with aliens, which have none.
        for screen in crate::screen::GameScreen::IN_GAME {
            if matches!(screen, crate::screen::GameScreen::AlienTalk | crate::screen::GameScreen::PubTalk) {
                continue;
            }
            let set = &data.icon_sets[screen.number().unwrap() as usize];
            assert!(!set.is_empty(), "{screen:?} has no icon bar set");
        }
        let icons: Vec<_> = data.action_icons.iter().flatten().collect();
        assert!(!icons.is_empty());
        let missing = icons.iter().filter(|h| images.get(**h).is_none()).count();
        assert_eq!(
            missing,
            0,
            "{missing} of {} icon images missing",
            icons.len()
        );
    }
}
