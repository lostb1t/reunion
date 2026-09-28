//! Choose Hero (GRAFIKA/CHOISE.PIC) and the chosen hero's picture.
//!
//! Original: REUNION.PRG FUN_321d_000d. Any click chooses; x < 160 picks hero 2,
//! otherwise hero 1. The id is stored at DS:0x9276 and the game then shows
//! "grafika\hero" + id, so the left half is HERO2 and the right half HERO1.
//!
//! Choosing plays SOUND/SELECT1 while the screen fades; the hero's picture
//! comes with SELECT2, and when that ends SELECT3 plays and the game starts.

use bevy::prelude::*;
use bevy_enhanced_input::prelude::*;

use crate::audio::{Sfx, SfxVoices, sfx_playing};
use crate::focus::{Activated, DefaultFocus, Hover, hotspot};
use crate::input::{Back, Click, Confirm};
use crate::screen::{GAME_HEIGHT, GAME_WIDTH, GameScreen, picture};
use crate::transition::GoTo;

pub struct HeroPlugin;

impl Plugin for HeroPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(OnEnter(GameScreen::ChooseHero), spawn_choose_hero)
            .add_systems(OnEnter(GameScreen::HeroIntro), spawn_hero_intro)
            .add_systems(Update, narrate.run_if(in_state(GameScreen::HeroIntro)))
            .add_observer(on_activated)
            .add_observer(on_back)
            .add_observer(continue_on::<Confirm>)
            .add_observer(continue_on::<Click>);
    }
}

/// The hero the player chose; the number matches the original's HERO<n>.PIC.
#[derive(Resource, Clone, Copy, PartialEq, Debug)]
pub struct Hero(pub u8);

/// Which hero a Choose Hero hotspot picks. Kept apart from the [`Hero`] resource:
/// in Bevy 0.19 one type can't safely be both a resource and a component.
#[derive(Component, Clone, Copy, PartialEq, Debug)]
struct HeroChoice(u8);

const SPLIT_X: f32 = 160.0;
/// The portrait fills rows 26-154 of CHOISE.PIC and "CHOOSE HERO" rows 166-178;
/// only the portrait part is dimmed, so the title stays readable.
const PORTRAIT: Rect = Rect {
    min: Vec2::ZERO,
    max: Vec2::new(GAME_WIDTH, 160.0),
};

fn spawn_choose_hero(mut commands: Commands, asset_server: Res<AssetServer>) {
    commands.spawn((
        picture(asset_server.load("GRAFIKA/CHOISE.PIC"), Vec2::ZERO),
        DespawnOnExit(GameScreen::ChooseHero),
    ));
    let halves = [
        (HeroChoice(2), Rect::new(0.0, 0.0, SPLIT_X, GAME_HEIGHT)),
        (
            HeroChoice(1),
            Rect::new(SPLIT_X, 0.0, GAME_WIDTH, GAME_HEIGHT),
        ),
    ];
    for (hero, rect) in halves {
        let mut half = commands.spawn((
            hotspot(rect, Hover::Spotlight { within: PORTRAIT }),
            hero,
            DespawnOnExit(GameScreen::ChooseHero),
        ));
        if hero == HeroChoice(2) {
            half.insert(DefaultFocus);
        }
    }
}

fn spawn_hero_intro(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    hero: Option<Res<Hero>>,
) {
    let Some(hero) = hero else { return };
    commands.spawn((
        picture(
            asset_server.load(format!("GRAFIKA/HERO{}.PIC", hero.0)),
            Vec2::ZERO,
        ),
        DespawnOnExit(GameScreen::HeroIntro),
    ));
}

fn on_activated(activated: On<Activated>, choices: Query<&HeroChoice>, mut commands: Commands) {
    let Ok(&HeroChoice(id)) = choices.get(activated.0) else {
        return;
    };
    commands.insert_resource(Hero(id));
    commands.insert_resource(Narration { step: 1, timer: Timer::from_seconds(NARRATION_MAX, TimerMode::Once) });
    commands.trigger(Sfx::named("select1"));
    commands.trigger(GoTo(GameScreen::HeroIntro));
}

/// Longest wait for a narration sample, in case it can't play.
const NARRATION_MAX: f32 = 8.0;

/// Which SELECT sample is playing.
#[derive(Resource)]
struct Narration {
    step: u8,
    timer: Timer,
}

/// FUN_321d_000d: each sample starts when the one before has ended.
fn narrate(
    time: Res<Time>,
    narration: Option<ResMut<Narration>>,
    voices: SfxVoices,
    mut commands: Commands,
) {
    let Some(mut narration) = narration else { return };
    let timed_out = narration.timer.tick(time.delta()).is_finished();
    if sfx_playing(&voices) && !timed_out {
        return;
    }
    narration.step += 1;
    narration.timer.reset();
    match narration.step {
        2 => commands.trigger(Sfx::named("select2")),
        _ => {
            commands.trigger(Sfx::named("select3"));
            commands.remove_resource::<Narration>();
            commands.trigger(GoTo(GameScreen::MainScreen));
        }
    }
}

fn on_back(
    _: On<Start<Back>>,
    screen: Res<State<GameScreen>>,
    editing: Option<Res<crate::text::TextEditing>>,
    popup: Option<Res<crate::popup::PopupOpen>>,
    mut commands: Commands,
) {
    if editing.is_some() || popup.is_some() {
        return;
    }
    let previous = match screen.get() {
        GameScreen::ChooseHero | GameScreen::LoadGame => GameScreen::MainMenu,
        GameScreen::HeroIntro => GameScreen::ChooseHero,
        // Like BACK TO M.SCREEN, which every in-game screen has in its icon bar.
        GameScreen::PlanetMain
        | GameScreen::PlanetInfo
        | GameScreen::Messages
        | GameScreen::DiskOperations
        | GameScreen::MainComputer
        | GameScreen::Research
        | GameScreen::Commanders
        | GameScreen::GalacticMap
        | GameScreen::Credits
        | GameScreen::ShipInfo
        | GameScreen::InfoBuy
        | GameScreen::StaffTalk
        | GameScreen::SpaceLocal => GameScreen::MainScreen,
        GameScreen::PubTalk => GameScreen::SpaceLocal,
        GameScreen::ResourceMine => GameScreen::PlanetMain,
        GameScreen::Colonize => GameScreen::PlanetInfo,
        GameScreen::CreateUnit
        | GameScreen::Group
        | GameScreen::ControlPanel
        | GameScreen::Transfer => GameScreen::ShipInfo,
        GameScreen::MainMenu
        | GameScreen::MainScreen
        | GameScreen::SpaceBattle
        | GameScreen::StoryScene
        | GameScreen::GroundSetup
        | GameScreen::GroundWar
        | GameScreen::AlienTalk
        | GameScreen::Cutscene => return,
    };
    commands.trigger(GoTo(previous));
}

/// Any confirm or click on the hero's picture starts the game.
fn continue_on<A: InputAction>(
    _: On<Start<A>>,
    screen: Res<State<GameScreen>>,
    mut commands: Commands,
) {
    if *screen.get() == GameScreen::HeroIntro {
        commands.remove_resource::<Narration>();
        commands.trigger(GoTo(GameScreen::MainScreen));
    }
}
