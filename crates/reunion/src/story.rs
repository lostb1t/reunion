//! What the story shows: message boxes, story pictures (screen 33) and
//! conversations with aliens (screen 34), in the order they happened.
//!
//! Pictures and conversations wait until the message boxes before them are
//! closed and you're on an ordinary screen (not in a battle or another
//! scene).
//!
//! A story picture (REUNION.PRG FUN_2e4b_0044): PICS/PIC<n>, the whole
//! screen, until a button is pressed; then the main screen.

use std::collections::VecDeque;

use bevy::prelude::*;
use bevy::sprite::Anchor;
use bevy_enhanced_input::prelude::*;
use reunion_formats::story::Event;

use crate::input::{Back, Click, Confirm};
use crate::popup::{PopupOpen, ShowMessage};
use crate::screen::{GameScreen, place};
use crate::transition::GoTo;

pub struct StoryPlugin;

impl Plugin for StoryPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<StoryQueue>()
            .add_observer(queue)
            .add_systems(Update, play.run_if(crate::screen::in_game))
            .add_systems(OnEnter(GameScreen::StoryScene), show_scene)
            .add_observer(end_scene::<Confirm>)
            .add_observer(end_scene::<Back>)
            .add_observer(end_scene::<Click>);
    }
}

/// Pictures and conversations waiting their turn.
#[derive(Resource, Default)]
pub struct StoryQueue(VecDeque<Event>);

/// Something the story shows next.
#[derive(bevy::ecs::event::Event, Clone)]
pub struct Tell(pub Event);

/// The picture or conversation being shown.
#[derive(Resource, Clone, Copy)]
pub struct Showing(pub u8);

fn queue(tell: On<Tell>, mut queue: ResMut<StoryQueue>, mut commands: Commands) {
    match &tell.0 {
        Event::Message(text) => commands.trigger(ShowMessage { text: text.clone(), log: true }),
        // The Morgruls' call opens with picture 10.
        Event::Talk(6) => {
            queue.0.push_back(Event::Scene(10));
            queue.0.push_back(Event::Talk(6));
        }
        other => queue.0.push_back(other.clone()),
    }
}

/// Shows the next picture or conversation when nothing's in the way.
fn play(
    mut queue: ResMut<StoryQueue>,
    popup: Option<Res<PopupOpen>>,
    screen: Res<State<GameScreen>>,
    mut commands: Commands,
) {
    if queue.0.is_empty() || popup.is_some() {
        return;
    }
    let busy = matches!(
        screen.get(),
        GameScreen::StoryScene
            | GameScreen::AlienTalk
            | GameScreen::PubTalk
            | GameScreen::SpaceBattle
            | GameScreen::GroundSetup
            | GameScreen::GroundWar
            | GameScreen::GameEnd
            | GameScreen::Colonize
            | GameScreen::CreateUnit
            | GameScreen::DiskOperations
            | GameScreen::LoadGame
    );
    if busy {
        return;
    }
    match queue.0.pop_front() {
        Some(Event::Scene(n)) => {
            commands.insert_resource(Showing(n));
            commands.trigger(GoTo(GameScreen::StoryScene));
        }
        Some(Event::Talk(n)) => {
            commands.insert_resource(Showing(n));
            commands.trigger(GoTo(GameScreen::AlienTalk));
        }
        _ => {}
    }
}

fn show_scene(mut commands: Commands, showing: Option<Res<Showing>>, asset_server: Res<AssetServer>) {
    let n = showing.map_or(1, |s| s.0);
    commands.spawn((
        Sprite::from_image(asset_server.load(format!("PICS/PIC{n}.PIC"))),
        Anchor::TOP_LEFT,
        place(Vec2::ZERO, 20.0),
        DespawnOnExit(GameScreen::StoryScene),
    ));
}

fn end_scene<A: InputAction>(
    _: On<Start<A>>,
    screen: Res<State<GameScreen>>,
    popup: Option<Res<PopupOpen>>,
    mut commands: Commands,
) {
    if *screen.get() == GameScreen::StoryScene && popup.is_none() {
        commands.trigger(GoTo(GameScreen::MainScreen));
    }
}
