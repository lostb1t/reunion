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

use crate::anim_player::{AnimPlayer, Delay, Segment, anim_player, position};
use crate::game_data::{GameData, GameDataHandle};
use crate::input::{Back, Click, Confirm};
use crate::pic::PALETTE;
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
            | GameScreen::Cutscene
            | GameScreen::AlienTalk
            | GameScreen::PubTalk
            | GameScreen::SpaceBattle
            | GameScreen::GroundSetup
            | GameScreen::GroundWar
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

fn show_scene(
    mut commands: Commands,
    showing: Option<Res<Showing>>,
    asset_server: Res<AssetServer>,
    handle: Res<GameDataHandle>,
    data: Res<Assets<GameData>>,
) {
    let n = showing.map_or(1, |s| s.0);
    let path = format!("PICS/PIC{n}.PIC");
    commands.spawn((
        Sprite::from_image(asset_server.load(&path)),
        Anchor::TOP_LEFT,
        place(Vec2::ZERO, 20.0),
        DespawnOnExit(GameScreen::StoryScene),
    ));
    // FUN_2e4b_0044: some pictures move, with their sounds.
    let segment = |main: u16, from, to, delay: Delay| Segment {
        animation: asset_server.load(format!("ANIM/MAIN{main}.ANI")),
        from,
        to,
        delay,
        sounds: Vec::new(),
        end_sound: None,
        looping: false,
    };
    let segments = match n {
        1 => vec![Segment { looping: true, ..segment(10, 2, 3, Delay::Fixed(10)) }],
        2 => vec![segment(14, 2, 71, Delay::Fixed(6))],
        9 => vec![
            Segment { sounds: vec![(5, "satrobb1")], end_sound: Some("satrobb2"), ..segment(8, 1, 34, Delay::Fixed(2)) },
            Segment { sounds: vec![(18, "satrobb3")], ..segment(9, 1, 41, Delay::PerFrame(|k| if k > 28 { 5 } else { 1 })) },
        ],
        10 => vec![Segment { sounds: vec![(10, "tractor")], ..segment(13, 1, 41, Delay::Fixed(5)) }],
        _ => Vec::new(),
    };
    if let (false, Some(data)) = (segments.is_empty(), data.get(&handle.0)) {
        let main = match n {
            1 => 10,
            2 => 14,
            9 => 8,
            _ => 13,
        };
        let at = position(&data.exe, main);
        commands.spawn((
            anim_player(AnimPlayer::new(segments, asset_server.load(format!("{path}#{PALETTE}"))), at, 21.0),
            DespawnOnExit(GameScreen::StoryScene),
        ));
    }
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
