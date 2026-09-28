//! The original 320x200 screen, mapped into Bevy's world space.
//!
//! Game code uses the original coordinates (origin top-left, y down) so values
//! from the decompiled game can be used as-is. The picture is stretched 1.2x
//! vertically like a 4:3 CRT did, and a wider window shows extra space at the
//! sides instead of distorting it.
//!
//! Widescreen: [`ViewBounds`] is the visible area in game coordinates, e.g.
//! x -32..352 on a 16:10 screen. Wide screens lay their interface out across
//! it and draw extension art (from the `art://` source) beside the original
//! pictures; 4:3 screens just leave the sides black.

use bevy::camera::ScalingMode;
use bevy::prelude::*;
use bevy::sprite::Anchor;

pub const GAME_WIDTH: f32 = 320.0;
pub const GAME_HEIGHT: f32 = 200.0;
/// VGA mode 13h pixels were displayed 1.2x taller than wide on 4:3 monitors.
pub const PIXEL_ASPECT: f32 = 1.2;

#[derive(States, Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum GameScreen {
    #[default]
    MainMenu,
    ChooseHero,
    HeroIntro,
    MainScreen,
    PlanetMain,
    PlanetInfo,
    Messages,
    DiskOperations,
    MainComputer,
    /// The disk screen opened from the main menu's LOAD GAME (screen 38:
    /// only LOAD and EXIT TO DOS).
    LoadGame,
    Research,
    Commanders,
    GalacticMap,
    Credits,
    ShipInfo,
    CreateUnit,
    Group,
    InfoBuy,
    ControlPanel,
    Transfer,
    StaffTalk,
    ResourceMine,
    Colonize,
    SpaceBattle,
    StoryScene,
    GroundSetup,
    GroundWar,
    GameEnd,
    SpaceLocal,
    PubTalk,
    AlienTalk,
}

impl GameScreen {
    /// Whether the screen uses the space beside the original 320 columns.
    /// Title screens, portraits and cutscenes stay 4:3.
    pub fn is_wide(self) -> bool {
        self.number().is_some()
    }

    /// The original's screen number (DS:0x95e6) for in-game screens; also
    /// the index of the screen's icon bar set.
    pub fn number(self) -> Option<u8> {
        match self {
            GameScreen::MainScreen => Some(1),
            GameScreen::PlanetInfo => Some(8),
            GameScreen::Messages => Some(11),
            GameScreen::DiskOperations => Some(12),
            GameScreen::PlanetMain => Some(20),
            GameScreen::MainComputer => Some(37),
            GameScreen::LoadGame => Some(38),
            GameScreen::Research => Some(3),
            GameScreen::Commanders => Some(2),
            GameScreen::GalacticMap => Some(7),
            GameScreen::Credits => Some(10),
            GameScreen::ShipInfo => Some(16),
            GameScreen::CreateUnit => Some(21),
            GameScreen::Group => Some(22),
            GameScreen::InfoBuy => Some(5),
            GameScreen::ControlPanel => Some(17),
            GameScreen::Transfer => Some(13),
            GameScreen::StaffTalk => Some(24),
            GameScreen::ResourceMine => Some(4),
            GameScreen::Colonize => Some(28),
            GameScreen::SpaceBattle => Some(29),
            GameScreen::StoryScene => Some(33),
            GameScreen::GroundSetup => Some(19),
            GameScreen::GroundWar => Some(32),
            GameScreen::SpaceLocal => Some(23),
            GameScreen::PubTalk => Some(25),
            GameScreen::AlienTalk => Some(34),
            _ => None,
        }
    }

    /// Screens that stop game time, like the original clearing DS:0x95f8.
    pub fn pauses_time(self) -> bool {
        matches!(
            self,
            GameScreen::DiskOperations
                | GameScreen::LoadGame
                | GameScreen::Colonize
                | GameScreen::CreateUnit
                | GameScreen::SpaceBattle
                | GameScreen::StoryScene
                | GameScreen::GroundSetup
                | GameScreen::GroundWar
                | GameScreen::GameEnd
                | GameScreen::AlienTalk
                | GameScreen::PubTalk
        )
    }

    pub fn from_number(number: u8) -> Option<Self> {
        Self::IN_GAME
            .into_iter()
            .find(|s| s.number() == Some(number))
    }

    /// All in-game screens (the ones with the icon bar and text strip).
    pub const IN_GAME: [GameScreen; 27] = [
        GameScreen::GroundSetup,
        GameScreen::GroundWar,
        GameScreen::SpaceLocal,
        GameScreen::PubTalk,
        GameScreen::StoryScene,
        GameScreen::AlienTalk,
        GameScreen::SpaceBattle,
        GameScreen::Colonize,
        GameScreen::ResourceMine,
        GameScreen::StaffTalk,
        GameScreen::ControlPanel,
        GameScreen::Transfer,
        GameScreen::InfoBuy,
        GameScreen::ShipInfo,
        GameScreen::CreateUnit,
        GameScreen::Group,
        GameScreen::Credits,
        GameScreen::GalacticMap,
        GameScreen::LoadGame,
        GameScreen::Commanders,
        GameScreen::Research,
        GameScreen::MainScreen,
        GameScreen::PlanetMain,
        GameScreen::PlanetInfo,
        GameScreen::Messages,
        GameScreen::DiskOperations,
        GameScreen::MainComputer,
    ];
}

/// Run condition: an in-game screen is showing.
pub fn in_game(screen: Res<State<GameScreen>>) -> bool {
    screen.get().number().is_some()
}

/// Play in the original 4:3 even on wide screens.
#[derive(Resource)]
pub struct Widescreen(pub bool);

/// Horizontal extent of the visible area in game coordinates, whole pixels.
#[derive(Resource, Clone, Copy, PartialEq, Debug)]
pub struct ViewBounds {
    pub left: f32,
    pub right: f32,
}

impl ViewBounds {
    pub fn width(self) -> f32 {
        self.right - self.left
    }
}

impl Default for ViewBounds {
    fn default() -> Self {
        Self {
            left: 0.0,
            right: GAME_WIDTH,
        }
    }
}

pub struct ScreenPlugin;

impl Plugin for ScreenPlugin {
    fn build(&self, app: &mut App) {
        app.init_state::<GameScreen>()
            .insert_resource(Widescreen(true))
            .init_resource::<ViewBounds>()
            .add_systems(Startup, spawn_camera)
            .add_systems(PreUpdate, update_view_bounds);
    }
}

fn update_view_bounds(
    projection: Single<&Projection, With<Camera2d>>,
    widescreen: Res<Widescreen>,
    screen: Res<State<GameScreen>>,
    mut bounds: ResMut<ViewBounds>,
) {
    let Projection::Orthographic(ortho) = *projection else {
        return;
    };
    let new = if widescreen.0 && screen.get().is_wide() {
        // The area is centered on the game's center; floor to whole pixels.
        let half = (ortho.area.width() / 2.0).floor().max(GAME_WIDTH / 2.0);
        ViewBounds {
            left: GAME_WIDTH / 2.0 - half,
            right: GAME_WIDTH / 2.0 + half,
        }
    } else {
        ViewBounds::default()
    };
    bounds.set_if_neq(new);
}

fn spawn_camera(mut commands: Commands) {
    commands.spawn((
        Camera2d,
        Projection::Orthographic(OrthographicProjection {
            scaling_mode: ScalingMode::AutoMin {
                min_width: GAME_WIDTH,
                min_height: GAME_HEIGHT * PIXEL_ASPECT,
            },
            ..OrthographicProjection::default_2d()
        }),
    ));
}

pub fn game_to_world(pos: Vec2) -> Vec2 {
    Vec2::new(
        pos.x - GAME_WIDTH / 2.0,
        (GAME_HEIGHT / 2.0 - pos.y) * PIXEL_ASPECT,
    )
}

pub fn world_to_game(pos: Vec2) -> Vec2 {
    Vec2::new(
        pos.x + GAME_WIDTH / 2.0,
        GAME_HEIGHT / 2.0 - pos.y / PIXEL_ASPECT,
    )
}

/// Transform that places a top-left anchored sprite at `pos` in game coordinates.
pub fn place(pos: Vec2, z: f32) -> Transform {
    Transform::from_translation(game_to_world(pos).extend(z)).with_scale(Vec3::new(
        1.0,
        PIXEL_ASPECT,
        1.0,
    ))
}

/// An original picture drawn at `pos` in game coordinates.
pub fn picture(image: Handle<Image>, pos: Vec2) -> impl Bundle {
    (Sprite::from_image(image), Anchor::TOP_LEFT, place(pos, 0.0))
}
