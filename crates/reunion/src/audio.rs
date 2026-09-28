//! Sound effects, speech and music from the original files.
//!
//! The original plays one sample at a time on the Sound Blaster
//! (FUN_431a_031a and friends): a new sound cuts off the one playing.
//!
//! Music is ProTracker modules: `*.MOD` in the cutscene programs, and in the
//! game `ANIM/*.SPD` (FUN_424a_000f). A background track loops (DS:0x95fe:
//! NO, MAIN1 or MAIN2, picked on the disk screen, FUN_424a_0137); some
//! screens play their own once: CHOISE choosing a hero, SPACE and EARTH in
//! battles, TALK with aliens, ATV<n> with story pictures.
//!
//! Play a sound with `commands.trigger(Sfx::named("okay"))`: the name is a
//! file in `SOUND/` without `.SMP`. [`Sfx::war`] and [`Sfx::ground`] are the
//! numbered battle sounds in `SOUND2/WARSND<n>` and `SOUND3/GRSND<n>`.

use std::collections::HashMap;
use std::sync::Arc;
use std::sync::atomic::{AtomicU32, Ordering};
use std::time::Duration;

use bevy::asset::io::Reader;
use bevy::asset::{AssetLoader, LoadContext};
use bevy::audio::{AddAudioSource, ChannelCount, Decodable, SampleRate, Source};
use bevy::prelude::*;
use crate::focus::Activated;
use reunion_formats::module::{self, ModError, Module};
use reunion_formats::sound::{Smp, SmpError};

pub struct SoundPlugin;

impl Plugin for SoundPlugin {
    fn build(&self, app: &mut App) {
        app.add_audio_source::<Pcm>()
            .add_audio_source::<Music>()
            .register_asset_loader(SmpLoader)
            .register_asset_loader(ModLoader)
            .init_resource::<Sounds>()
            .insert_resource(SoundSettings { speech: true })
            .insert_resource(GameMusic { background: 1, playing: None })
            .add_systems(Update, game_music)
            .add_observer(play_sfx)
            .add_observer(click_sound)
            .add_observer(play_music)
            .add_observer(stop_music);
    }
}

/// What the player hears, like the original's setup (DS:0x9600 = 2 for speech).
#[derive(Resource)]
pub struct SoundSettings {
    /// Icons say what they do instead of clicking.
    pub speech: bool,
}

/// The game's music (not the cutscenes').
#[derive(Resource)]
pub struct GameMusic {
    /// DS:0x95fe: 0 NO, 1 MAIN1, 2 MAIN2.
    pub background: u8,
    playing: Option<&'static str>,
}

/// Story picture n's track (the main loop, screen 33): ATV1-5.
fn story_track(n: u8) -> Option<&'static str> {
    Some(match n {
        1 => "ATV1",
        2 | 4 => "ATV5",
        3 => "ATV2",
        5 => "ATV4",
        6 => "ATV3",
        _ => return None,
    })
}

fn game_music(
    screen: Res<State<crate::screen::GameScreen>>,
    showing: Option<Res<crate::story::Showing>>,
    mut music: ResMut<GameMusic>,
    mut commands: Commands,
) {
    use crate::screen::GameScreen as S;
    let screen = *screen.get();
    if screen == S::Cutscene {
        // The cutscenes have their own; the game's starts again after.
        music.playing = None;
        return;
    }
    // A screen's own track (played once), "" for silence, or the background.
    let special = match screen {
        S::ChooseHero => Some("CHOISE"),
        // The narration after choosing (FUN_321d_000d stops the music).
        S::HeroIntro => Some(""),
        S::SpaceBattle => Some("SPACE"),
        S::GroundSetup | S::GroundWar => Some("EARTH"),
        S::AlienTalk => Some("TALK"),
        S::StoryScene => showing.and_then(|s| story_track(s.0)),
        _ => None,
    };
    let (track, looping) = match special {
        Some(track) => (track, false),
        None => (["NO", "MAIN1", "MAIN2"][usize::from(music.background.min(2))], true),
    };
    if music.playing == Some(track) {
        return;
    }
    music.playing = Some(track);
    if track.is_empty() {
        commands.trigger(StopMusic);
    } else {
        commands.trigger(PlayMusic { path: format!("ANIM/{track}.SPD"), looping });
    }
}

/// Plays a sound effect, cutting off the one playing.
#[derive(Event, Clone, Debug)]
pub struct Sfx(pub String);

impl Sfx {
    /// `SOUND/<name>.SMP`.
    pub fn named(name: &str) -> Self {
        Self(format!("SOUND/{}.SMP", name.to_uppercase()))
    }

    /// `SOUND2/WARSND<n>.SMP`, the space battle's sounds.
    pub fn war(n: u8) -> Self {
        Self(format!("SOUND2/WARSND{n}.SMP"))
    }

    /// `SOUND3/GRSND<n>.SMP`, the ground war's sounds.
    pub fn ground(n: u8) -> Self {
        Self(format!("SOUND3/GRSND{n}.SMP"))
    }
}

/// Starts a module (e.g. `INTRO/INTRO1.MOD`), replacing the music playing.
#[derive(Event, Clone, Debug)]
pub struct PlayMusic {
    pub path: String,
    pub looping: bool,
}

/// Stops the music.
#[derive(Event, Clone, Debug)]
pub struct StopMusic;

/// The sound a hotspot makes when activated (`SOUND/<name>.SMP`).
#[derive(Component, Clone, Copy)]
pub struct ClickSound(pub &'static str);

fn click_sound(activated: On<Activated>, sounds: Query<&ClickSound>, mut commands: Commands) {
    if let Ok(sound) = sounds.get(activated.0) {
        commands.trigger(Sfx::named(sound.0));
    }
}

/// Loaded sounds, kept so they aren't reloaded every time.
#[derive(Resource, Default)]
struct Sounds(HashMap<String, Handle<Pcm>>);

#[derive(Component)]
pub struct SfxVoice;

/// Whether a sound effect is still playing (or loading to play).
pub fn sfx_playing(voices: &SfxVoices) -> bool {
    !voices.is_empty()
}

/// For [`sfx_playing`].
pub type SfxVoices<'w, 's> = Query<'w, 's, (), With<SfxVoice>>;

#[derive(Component)]
pub struct MusicVoice;

fn play_sfx(
    sfx: On<Sfx>,
    voices: Query<Entity, With<SfxVoice>>,
    server: Res<AssetServer>,
    mut sounds: ResMut<Sounds>,
    mut commands: Commands,
) {
    for voice in &voices {
        commands.entity(voice).despawn();
    }
    let handle = sounds
        .0
        .entry(sfx.0.clone())
        .or_insert_with(|| server.load(sfx.0.clone()))
        .clone();
    commands.spawn((SfxVoice, AudioPlayer(handle), PlaybackSettings::DESPAWN));
}

fn play_music(
    music: On<PlayMusic>,
    voices: Query<Entity, With<MusicVoice>>,
    server: Res<AssetServer>,
    musics: Res<Assets<Music>>,
    mut commands: Commands,
) {
    for voice in &voices {
        commands.entity(voice).despawn();
    }
    let handle: Handle<Music> = server.load(music.path.clone());
    // Played before: not over, it's starting again.
    if let Some(loaded) = musics.get(&handle) {
        loaded.position.store(0, Ordering::Relaxed);
    }
    let settings = if music.looping {
        PlaybackSettings::LOOP
    } else {
        PlaybackSettings::DESPAWN
    };
    commands.spawn((MusicVoice, AudioPlayer(handle), settings));
}

fn stop_music(_: On<StopMusic>, voices: Query<Entity, With<MusicVoice>>, mut commands: Commands) {
    for voice in &voices {
        commands.entity(voice).despawn();
    }
}

/// A decoded sample, mono.
#[derive(Asset, TypePath, Clone)]
pub struct Pcm {
    rate: u32,
    samples: Arc<[f32]>,
}

impl Decodable for Pcm {
    type Decoder = PcmSource;

    fn decoder(&self) -> PcmSource {
        PcmSource {
            pcm: self.clone(),
            pos: 0,
        }
    }
}

pub struct PcmSource {
    pcm: Pcm,
    pos: usize,
}

impl Iterator for PcmSource {
    type Item = f32;

    fn next(&mut self) -> Option<f32> {
        let sample = self.pcm.samples.get(self.pos).copied();
        self.pos += 1;
        sample
    }
}

impl Source for PcmSource {
    fn current_span_len(&self) -> Option<usize> {
        Some(self.pcm.samples.len().saturating_sub(self.pos))
    }

    fn channels(&self) -> ChannelCount {
        ChannelCount::MIN
    }

    fn sample_rate(&self) -> SampleRate {
        SampleRate::new(self.pcm.rate).unwrap_or(SampleRate::MIN)
    }

    fn total_duration(&self) -> Option<Duration> {
        Some(Duration::from_secs_f64(
            self.pcm.samples.len() as f64 / f64::from(self.pcm.rate.max(1)),
        ))
    }
}

/// Output rate of the music player.
const MUSIC_RATE: u32 = 44100;

/// A ProTracker module, played by [`module::Player`].
#[derive(Asset, TypePath, Clone)]
pub struct Music {
    module: Arc<Module>,
    /// Where the playing copy is: order << 8 | row (0-based), or
    /// [`MUSIC_OVER`] once it has ended. For cutscenes that wait for the music.
    pub position: Arc<AtomicU32>,
}

/// [`Music::position`] after the song's end.
pub const MUSIC_OVER: u32 = u32::MAX;

impl Music {
    /// Order position and row (0-based), `None` once it's over.
    pub fn position(&self) -> Option<(usize, usize)> {
        match self.position.load(Ordering::Relaxed) {
            MUSIC_OVER => None,
            p => Some(((p >> 8) as usize, (p & 0xff) as usize)),
        }
    }
}

impl Decodable for Music {
    type Decoder = MusicSource;

    fn decoder(&self) -> MusicSource {
        self.position.store(0, Ordering::Relaxed);
        MusicSource {
            // Bevy's Loop mode repeats the source; the player itself stops at the end.
            player: {
                let mut player = module::Player::new((*self.module).clone(), MUSIC_RATE);
                player.looping = false;
                player
            },
            buffer: vec![0.0; 2048],
            pos: 2048,
            position: self.position.clone(),
        }
    }
}

pub struct MusicSource {
    player: module::Player,
    buffer: Vec<f32>,
    pos: usize,
    position: Arc<AtomicU32>,
}

impl Iterator for MusicSource {
    type Item = f32;

    fn next(&mut self) -> Option<f32> {
        if self.pos == self.buffer.len() {
            if self.player.finished() {
                self.position.store(MUSIC_OVER, Ordering::Relaxed);
                return None;
            }
            self.player.render(&mut self.buffer);
            let (order, row) = self.player.position();
            self.position.store((order as u32) << 8 | row as u32, Ordering::Relaxed);
            self.pos = 0;
        }
        let sample = self.buffer[self.pos];
        self.pos += 1;
        Some(sample)
    }
}

impl Source for MusicSource {
    fn current_span_len(&self) -> Option<usize> {
        None
    }

    fn channels(&self) -> ChannelCount {
        ChannelCount::new(2).unwrap_or(ChannelCount::MIN)
    }

    fn sample_rate(&self) -> SampleRate {
        SampleRate::new(MUSIC_RATE).unwrap_or(SampleRate::MIN)
    }

    fn total_duration(&self) -> Option<Duration> {
        None
    }
}

#[derive(TypePath)]
struct SmpLoader;

#[derive(Debug, thiserror::Error)]
enum SmpLoadError {
    #[error("io: {0}")]
    Io(#[from] std::io::Error),
    #[error("smp: {0}")]
    Smp(#[from] SmpError),
}

impl AssetLoader for SmpLoader {
    type Asset = Pcm;
    type Settings = ();
    type Error = SmpLoadError;

    async fn load(
        &self,
        reader: &mut dyn Reader,
        _settings: &(),
        _load_context: &mut LoadContext<'_>,
    ) -> Result<Pcm, SmpLoadError> {
        let mut bytes = Vec::new();
        reader.read_to_end(&mut bytes).await?;
        let smp = Smp::parse(&bytes)?;
        Ok(Pcm {
            rate: smp.sample_rate,
            samples: smp.to_f32().into(),
        })
    }

    fn extensions(&self) -> &[&str] {
        &["SMP", "smp"]
    }
}

#[derive(TypePath)]
struct ModLoader;

#[derive(Debug, thiserror::Error)]
enum ModLoadError {
    #[error("io: {0}")]
    Io(#[from] std::io::Error),
    #[error("mod: {0}")]
    Mod(#[from] ModError),
}

impl AssetLoader for ModLoader {
    type Asset = Music;
    type Settings = ();
    type Error = ModLoadError;

    async fn load(
        &self,
        reader: &mut dyn Reader,
        _settings: &(),
        _load_context: &mut LoadContext<'_>,
    ) -> Result<Music, ModLoadError> {
        let mut bytes = Vec::new();
        reader.read_to_end(&mut bytes).await?;
        Ok(Music {
            module: Arc::new(Module::parse(&bytes)?),
            position: Arc::new(AtomicU32::new(0)),
        })
    }

    fn extensions(&self) -> &[&str] {
        // The game's own tracks are modules too (ANIM/*.SPD).
        &["MOD", "mod", "SPD", "spd"]
    }
}
