//! The original's cutscene programs (GRWAR/INTRO.PRG, CREDITS.PRG and
//! VICTORY.PRG), which START.EXE runs around the game: the company credits
//! and the intro before it, the victory sequence after winning.
//!
//! They share one small engine: a module plays and the pictures follow it,
//! waiting for an order position and row (FUN_119b_0000); pictures and
//! palette fades one step per 70 Hz frame (FUN_1212_*); animations
//! (INTRO/ANIM<n>.ANI) drawn frame over frame every so many milliseconds
//! (FUN_1116_01c1); 640 x 480 16-colour stills from five 100-row strips
//! (FUN_10d9_00f1). A script here is those calls, broken into [`Op`]s.
//! Any button skips the rest.

use std::collections::HashMap;

use bevy::prelude::*;
use bevy::sprite::Anchor;
use bevy_enhanced_input::prelude::*;
use reunion_formats::ani::Ani;
use reunion_formats::pic::Pic;

use crate::audio::{Music, MusicVoice, PlayMusic, StopMusic};
use crate::input::{Back, Click, Confirm};
use crate::pic::{Animation, INDEXED, IndexedPic, rgba_image};
use crate::screen::{GameScreen, place};
use crate::transition::GoTo;
use crate::upscale::CanvasOverlay;

pub struct CutscenePlugin;

impl Plugin for CutscenePlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(OnEnter(GameScreen::Cutscene), start)
            .add_systems(Update, run.run_if(in_state(GameScreen::Cutscene)))
            .add_systems(OnExit(GameScreen::Cutscene), stop)
            .add_observer(skip::<Confirm>)
            .add_observer(skip::<Back>)
            .add_observer(skip::<Click>);
    }
}

const VGA_HZ: f64 = 70.086;
const LORES: (usize, usize) = (320, 200);
const HIRES: (usize, usize) = (640, 480);

/// The cutscenes to play, in order; then the game goes on at `then`.
#[derive(Resource)]
pub struct Cutscenes {
    pub scripts: Vec<Vec<Op>>,
    pub then: GameScreen,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Fade {
    /// Black to the target palette (FUN_1212_00bb).
    In,
    /// The target palette to black (FUN_1212_01c2).
    Out,
    /// White to the target (FUN_1212_02d5).
    FromWhite,
    /// The target to white (FUN_1212_065b).
    ToWhite,
    /// The target halfway to white (FUN_1212_03fd).
    HalfWhiteOut,
    /// Halfway to white back to the target (FUN_1212_052c).
    HalfWhiteIn,
}

#[derive(Clone, Debug)]
pub enum Op {
    Music(String),
    /// Until the music reaches row `row` of order position `order` (1-based).
    WaitMusic { row: u16, order: u16 },
    /// 70 Hz frames.
    WaitFrames(u32),
    /// Restarts the millisecond clock (FUN_1342_0000).
    ResetClock,
    /// Until `ms` more of the clock have passed (FUN_1342_0016).
    WaitClock(u32),
    Fade(Fade, u32),
    /// Target and shown palette black (FUN_1212_0890).
    Blacken,
    /// A picture into the buffer; its palette becomes the pending one.
    LoadPic(String),
    /// Only a picture's palette, as the pending one (FUN_1212_1003).
    LoadPalette(String),
    /// The pending palette becomes the target (`Move(0xb90, 0x890)`).
    UsePalette,
    /// The buffer onto the screen.
    ShowBuffer,
    Clear,
    /// 320 x 200 (FUN_1212_113a).
    Lores,
    /// An animation: its frames come with [`Op::AnimFrame`].
    Anim(String),
    /// The animation's next frame drawn over the screen.
    AnimFrame,
    /// Frame `k` over the animation's first, into the buffer and onto the screen.
    AnimKeyFrame(usize),
    /// The shown palette `a / b` of the way from white to the pending one.
    Whiten(u32, u32),
    /// A 640 x 480 still from <prefix>1-5.PIC (e.g. INTROP/1X); its
    /// palette pending.
    Hires(String),
    /// The animation continues after frame `n`.
    AnimSkip(usize),
    /// The pending palette on screen at once (FUN_1212_0074).
    SetPalette,
    /// Pictures scroll up through the screen one after another, from and
    /// back to black, a row a frame (VICTORY.PRG FUN_10b4_0279).
    Roll(Vec<String>),
    /// The buffer on the screen moved by up to `n` pixels (FUN_1116_04ab).
    Shake(u8),
    /// A picture comes down over the screen, a row a frame (FUN_119b_0227).
    Slide(String),
    /// Past a planet and a ship over the scrolling stars (FUN_119b_0366).
    FlyBy { planet: String, ship: String },
    End,
}

/// Builds a script from the original's calls.
#[derive(Default)]
pub struct Script(pub Vec<Op>);

impl Script {
    fn push(&mut self, op: Op) -> &mut Self {
        self.0.push(op);
        self
    }

    pub fn music(&mut self, path: &str) -> &mut Self {
        self.push(Op::Music(path.into()))
    }

    pub fn wait(&mut self, row: u16, order: u16) -> &mut Self {
        self.push(Op::WaitMusic { row, order })
    }

    pub fn wait_frames(&mut self, n: u32) -> &mut Self {
        self.push(Op::WaitFrames(n))
    }

    pub fn fade(&mut self, fade: Fade, frames: u32) -> &mut Self {
        self.push(Op::Fade(fade, frames))
    }

    pub fn lores(&mut self) -> &mut Self {
        self.push(Op::Lores)
    }

    pub fn clear(&mut self) -> &mut Self {
        self.push(Op::Clear)
    }

    pub fn load(&mut self, path: &str) -> &mut Self {
        self.push(Op::LoadPic(path.into()))
    }

    pub fn show_buffer(&mut self) -> &mut Self {
        self.push(Op::ShowBuffer)
    }

    pub fn use_palette(&mut self) -> &mut Self {
        self.push(Op::UsePalette)
    }

    pub fn whiten(&mut self, a: u32, b: u32) -> &mut Self {
        self.push(Op::Whiten(a, b)).wait_frames(1)
    }

    /// FUN_119b_0152: a picture into the buffer, then wait.
    pub fn load_and_wait(&mut self, row: u16, order: u16, path: &str) -> &mut Self {
        self.load(path).wait(row, order)
    }

    /// FUN_119b_018e: at the music's (row, order) the screen fades out over
    /// `fade_out` frames and the picture fades in over `fade_in`.
    pub fn show_pic(&mut self, fade_in: u32, fade_out: u32, row: u16, order: u16, path: &str) -> &mut Self {
        self.load_and_wait(row, order, path);
        if fade_out > 0 {
            self.fade(Fade::Out, fade_out);
        }
        self.show_buffer().use_palette();
        if fade_in > 0 {
            self.fade(Fade::In, fade_in);
        }
        self
    }

    /// FUN_1116_01c1: animation `path` from the music's (row, order), a frame
    /// every `delay` ms; `palette` brings its own (fading through black).
    #[allow(clippy::too_many_arguments)]
    pub fn anim(&mut self, palette: Option<&str>, delay: u32, row: u16, order: u16, path: &str, frames: usize, special: AnimSpecial) -> &mut Self {
        if let Some(p) = palette {
            self.push(Op::LoadPalette(p.into()));
        }
        self.push(Op::Anim(path.into()));
        self.wait(row, order);
        if special == AnimSpecial::WaitFirst {
            self.wait_frames(0x12);
        }
        if palette.is_some() {
            self.fade(Fade::Out, 5).push(Op::Blacken);
        }
        self.push(Op::ResetClock);
        self.first_frame(special);
        if palette.is_some() {
            self.use_palette().fade(Fade::In, 5);
        }
        for k in 2..=frames {
            if special == AnimSpecial::ShakeStart && k < 5 {
                self.push(Op::Shake(3));
            }
            if special == AnimSpecial::Flash && k == 0x33 {
                self.push(Op::Whiten(4, 5));
            }
            if special == AnimSpecial::Flash && k == 0x34 {
                self.push(Op::Whiten(3, 5));
            }
            self.push(Op::WaitClock(delay));
            match special {
                AnimSpecial::KeyFrames => self.push(Op::AnimKeyFrame(k)),
                _ => self.push(Op::AnimFrame),
            };
        }
        self.use_palette()
    }

    fn first_frame(&mut self, special: AnimSpecial) -> &mut Self {
        match special {
            AnimSpecial::KeyFrames => self.push(Op::AnimKeyFrame(1)),
            _ => self.push(Op::AnimFrame),
        }
    }

    /// FUN_1116_0391: like [`Script::anim`], in the frame order given.
    pub fn anim_ordered(&mut self, delay: u32, row: u16, order: u16, path: &str, frames: &[u8]) -> &mut Self {
        self.push(Op::Anim(path.into()));
        self.wait(row, order);
        self.push(Op::ResetClock).push(Op::AnimFrame);
        for &k in frames {
            self.push(Op::WaitClock(delay)).push(Op::AnimKeyFrame(usize::from(k)));
        }
        self.use_palette()
    }

    /// FUN_10d9_00f1: at (row0, order0) the screen fades out; the still is
    /// put up and fades in at (row, order). A `fade_in` of -1 flashes white.
    #[allow(clippy::too_many_arguments)]
    pub fn hires(&mut self, fade_in: i32, row: u16, order: u16, fade_out: u32, row0: u16, order0: u16, prefix: &str) -> &mut Self {
        self.wait(row0, order0);
        if fade_out > 0 {
            self.fade(Fade::Out, fade_out);
        }
        self.push(Op::Blacken).push(Op::Hires(prefix.into()));
        if order > 0 && row > 0 {
            self.wait(row, order);
        }
        self.use_palette();
        if fade_in > 0 {
            self.fade(Fade::In, fade_in as u32);
        }
        if fade_in == -1 {
            self.fade(Fade::In, 5).fade(Fade::HalfWhiteOut, 5).fade(Fade::FromWhite, 10);
        }
        self
    }

    /// VICTORY.PRG FUN_10b4_01ce: like [`Script::show_pic`], but the picture
    /// fades in at another point of the music.
    #[allow(clippy::too_many_arguments)]
    pub fn show_pic_then(&mut self, fade_in: u32, fade_out: u32, row: u16, order: u16, row0: u16, order0: u16, path: &str) -> &mut Self {
        self.load_and_wait(row0, order0, path);
        if fade_out > 0 {
            self.fade(Fade::Out, fade_out);
        }
        self.show_buffer().use_palette().wait(row, order);
        if fade_in > 0 {
            self.fade(Fade::In, fade_in);
        }
        self
    }

    /// CREDITS.PRG FUN_1000_0105: an animation a frame every `delay(k)`
    /// hundredths of a second; ANIM3 starts at frame 31, ANIM4 clears the
    /// screen first.
    pub fn credits_anim(&mut self, palette: bool, row: u16, order: u16, n: usize, frames: usize) -> &mut Self {
        if palette {
            self.push(Op::LoadPalette(format!("CREDITS/PAL{n}.PIC")));
        }
        self.push(Op::Anim(format!("CREDITS/ANIM{n}.ANI")));
        let start = if n == 3 { 30 } else { 0 };
        self.push(Op::AnimSkip(start));
        self.wait(row, order);
        if palette {
            self.fade(Fade::Out, 5).push(Op::Blacken);
        }
        if n == 4 {
            self.clear();
        }
        self.push(Op::ResetClock).push(Op::AnimFrame);
        if palette {
            self.use_palette().fade(Fade::In, 5);
        }
        for k in start + 2..=frames {
            let delay = match n {
                3 => (k % 2) * 2,
                4 => 5,
                _ => 3,
            };
            self.push(Op::WaitClock(10 * delay as u32)).push(Op::AnimFrame);
        }
        self.use_palette()
    }

    /// FUN_10d9_037f: fade out at (row, order).
    pub fn fade_out_at(&mut self, frames: u32, row: u16, order: u16) -> &mut Self {
        self.wait(row, order).fade(Fade::Out, frames).push(Op::Blacken)
    }

    /// FUN_1116_04ab.
    pub fn shake(&mut self, amount: u8, delay: u32, times: u32) -> &mut Self {
        for _ in 0..times {
            self.push(Op::Shake(amount)).wait_frames(delay);
        }
        self
    }

    pub fn slide(&mut self, row: u16, order: u16, path: &str) -> &mut Self {
        self.wait(row, order).push(Op::Slide(path.into())).use_palette()
    }

    pub fn fly_by(&mut self, row: u16, order: u16, planet: &str, ship: &str) -> &mut Self {
        self.wait(row, order).push(Op::FlyBy { planet: planet.into(), ship: ship.into() })
    }

    pub fn end(&mut self) -> Vec<Op> {
        self.push(Op::End);
        std::mem::take(&mut self.0)
    }
}

/// Animations the intro treats specially (FUN_1116_01c1).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AnimSpecial {
    None,
    /// ANIM12 waits 18 frames first.
    WaitFirst,
    /// ANIM16 shakes over its first frames.
    ShakeStart,
    /// ANIM19 flashes at frames 51 and 52.
    Flash,
    /// ANIM7 is drawn as whole frames.
    KeyFrames,
}

/// Where a script is.
#[derive(Resource)]
struct Runner {
    ops: Vec<Op>,
    pc: usize,
    /// 70 Hz frames that may be spent.
    frames: f64,
    /// Milliseconds since [`Op::ResetClock`], and where the schedule is.
    clock: f64,
    schedule: f64,
    /// Progress within a multi-frame op.
    step: u32,
    size: (usize, usize),
    screen: Vec<u8>,
    buffer: Vec<u8>,
    /// Palettes: on screen, being faded to, and waiting (0x890 / 0xb90).
    shown: [[u8; 3]; 256],
    target: [[u8; 3]; 256],
    pending: [[u8; 3]; 256],
    anim: Option<Handle<Animation>>,
    anim_frame: usize,
    music: Option<Handle<Music>>,
    pictures: HashMap<String, Handle<IndexedPic>>,
    animations: HashMap<String, Handle<Animation>>,
    image: Handle<Image>,
    dirty: bool,
    skipped: bool,
    /// For the fly-by: its two pictures once loaded.
    fly: Option<(Pic, Pic)>,
    slide_from: Vec<u8>,
}

#[derive(Component)]
struct CutsceneSprite;

impl Runner {
    /// Starts `ops`, loading everything they need.
    fn new(ops: Vec<Op>, server: &AssetServer, image: Handle<Image>) -> Self {
        let mut pictures = HashMap::new();
        let mut animations = HashMap::new();
        let mut picture = |p: &str| {
            pictures
                .entry(p.to_string())
                .or_insert_with(|| server.load(format!("{p}#{INDEXED}")));
        };
        for op in &ops {
            match op {
                Op::LoadPic(p) | Op::LoadPalette(p) | Op::Slide(p) => picture(p),
                Op::FlyBy { planet, ship } => {
                    picture(planet);
                    picture(ship);
                }
                Op::Hires(prefix) => {
                    for k in 1..=5 {
                        picture(&format!("{prefix}{k}.PIC"));
                    }
                }
                Op::Roll(paths) => paths.iter().for_each(|p| picture(p)),
                Op::Anim(p) => {
                    animations.entry(p.clone()).or_insert_with(|| server.load(p.clone()));
                }
                _ => {}
            }
        }
        Self {
            ops,
            pc: 0,
            frames: 0.0,
            clock: 0.0,
            schedule: 0.0,
            step: 0,
            size: LORES,
            screen: vec![0; LORES.0 * LORES.1],
            buffer: vec![0; LORES.0 * LORES.1],
            shown: [[0; 3]; 256],
            target: [[0; 3]; 256],
            pending: [[0; 3]; 256],
            anim: None,
            anim_frame: 0,
            music: None,
            pictures,
            animations,
            image,
            dirty: true,
            skipped: false,
            fly: None,
            slide_from: Vec::new(),
        }
    }
}

/// The screen and an idle runner; [`run`] takes the scripts from [`Cutscenes`].
fn start(
    mut commands: Commands,
    cutscenes: Option<Res<Cutscenes>>,
    server: Res<AssetServer>,
    mut images: ResMut<Assets<Image>>,
    mut next: ResMut<NextState<GameScreen>>,
) {
    if cutscenes.is_none() {
        next.set(GameScreen::MainMenu);
        return;
    }
    let image = images.add(rgba_image(LORES.0 as u32, LORES.1 as u32, vec![0; LORES.0 * LORES.1 * 4]));
    commands.spawn((
        CutsceneSprite,
        Sprite {
            image: image.clone(),
            custom_size: Some(Vec2::new(320.0, 200.0)),
            ..default()
        },
        Anchor::TOP_LEFT,
        place(Vec2::ZERO, 5.0),
        DespawnOnExit(GameScreen::Cutscene),
    ));
    // The 640 x 480 stills, at the screen's resolution.
    commands.spawn((
        CutsceneSprite,
        CanvasOverlay(Rect::new(0.0, 0.0, 320.0, 200.0)),
        bevy::camera::visibility::RenderLayers::layer(1),
        Sprite::default(),
        Anchor::TOP_LEFT,
        Visibility::Hidden,
        DespawnOnExit(GameScreen::Cutscene),
    ));
    commands.insert_resource(Runner::new(Vec::new(), &server, image));
}

fn stop(mut commands: Commands) {
    commands.remove_resource::<Runner>();
    commands.trigger(StopMusic);
}

fn skip<A: InputAction>(_: On<Start<A>>, runner: Option<ResMut<Runner>>) {
    if let Some(mut runner) = runner {
        runner.skipped = true;
    }
}

/// Blends `from` to `to` by k / n per channel.
fn blend(from: [u8; 3], to: [u8; 3], k: u32, n: u32) -> [u8; 3] {
    let n = n.max(1);
    std::array::from_fn(|c| {
        let (a, b) = (i32::from(from[c]), i32::from(to[c]));
        (a + (b - a) * k as i32 / n as i32) as u8
    })
}

const WHITE: [u8; 3] = [255, 255, 255];
const BLACK: [u8; 3] = [0, 0, 0];

fn palette_step(runner: &mut Runner, fade: Fade, k: u32, n: u32) {
    let target = runner.target;
    for (shown, t) in runner.shown.iter_mut().zip(target) {
        *shown = match fade {
            Fade::In => blend(BLACK, t, k, n),
            Fade::Out => blend(t, BLACK, k, n),
            Fade::FromWhite => blend(WHITE, t, k, n),
            Fade::ToWhite => blend(t, WHITE, k, n),
            // k counts n down to n / 2, and back up.
            Fade::HalfWhiteOut => blend(WHITE, t, n - k, n),
            Fade::HalfWhiteIn => blend(WHITE, t, n / 2 + k, n),
        };
    }
    runner.dirty = true;
}

/// How many palette steps a fade takes (the originals count 0..=n).
fn fade_steps(fade: Fade, n: u32) -> u32 {
    match fade {
        Fade::HalfWhiteOut | Fade::HalfWhiteIn => n - n / 2 + 1,
        _ => n + 1,
    }
}

fn set_size(runner: &mut Runner, size: (usize, usize)) {
    runner.size = size;
    runner.screen = vec![0; size.0 * size.1];
    runner.dirty = true;
}

fn run(
    time: Res<Time>,
    runner: Option<ResMut<Runner>>,
    cutscenes: Option<ResMut<Cutscenes>>,
    pictures: Res<Assets<IndexedPic>>,
    animations: Res<Assets<Animation>>,
    musics: Res<Assets<Music>>,
    voices: Query<&AudioPlayer<Music>, With<MusicVoice>>,
    mut images: ResMut<Assets<Image>>,
    mut sprites: Query<(&mut Sprite, &mut Visibility, Has<CanvasOverlay>), With<CutsceneSprite>>,
    server: Res<AssetServer>,
    mut commands: Commands,
) {
    let (Some(mut runner), Some(mut cutscenes)) = (runner, cutscenes) else { return };
    let r = &mut *runner;
    let dt = time.delta_secs_f64();
    r.frames = (r.frames + dt * VGA_HZ).min(8.0);
    r.clock += dt * 1000.0;
    // A new request (even while one plays), or the first script.
    if (cutscenes.is_added() || r.ops.is_empty()) && !cutscenes.scripts.is_empty() {
        commands.trigger(StopMusic);
        let ops = cutscenes.scripts.remove(0);
        *r = Runner::new(ops, &server, r.image.clone());
    } else if r.ops.is_empty() {
        commands.trigger(GoTo(cutscenes.then));
        return;
    }
    // The next program, or the game.
    let mut finish = |r: &mut Runner, commands: &mut Commands| {
        commands.trigger(StopMusic);
        if cutscenes.scripts.is_empty() {
            r.pc = r.ops.len();
            r.skipped = false;
            commands.trigger(GoTo(cutscenes.then));
        } else {
            let ops = cutscenes.scripts.remove(0);
            *r = Runner::new(ops, &server, r.image.clone());
        }
    };
    if r.skipped {
        finish(r, &mut commands);
        return;
    }
    loop {
        let Some(op) = r.ops.get(r.pc).cloned() else {
            return;
        };
        let mut done = true;
        match op {
            Op::Music(path) => {
                commands.trigger(PlayMusic { path: path.clone(), looping: false });
                r.music = Some(server.load(path));
            }
            Op::WaitMusic { row, order } => {
                // Until the song has got there (or is over, or not playing).
                let playing = voices.iter().next().is_some();
                let position = r.music.as_ref().and_then(|h| musics.get(h)).map(|m| m.position());
                done = match position {
                    Some(Some((o, rw))) => (o + 1, rw + 1) >= (usize::from(order), usize::from(row)) || !playing,
                    Some(None) => true,
                    None => r.music.is_none(),
                };
                if !done {
                    r.frames = r.frames.min(1.0);
                }
            }
            Op::WaitFrames(n) => {
                while r.step < n && r.frames >= 1.0 {
                    r.frames -= 1.0;
                    r.step += 1;
                }
                done = r.step >= n;
            }
            Op::ResetClock => {
                r.clock = 0.0;
                r.schedule = 0.0;
            }
            Op::WaitClock(ms) => {
                done = r.clock >= r.schedule + f64::from(ms);
                if done {
                    r.schedule += f64::from(ms);
                }
            }
            Op::Fade(fade, n) => {
                let steps = fade_steps(fade, n);
                while r.step < steps && r.frames >= 1.0 {
                    r.frames -= 1.0;
                    palette_step(r, fade, r.step, n);
                    r.step += 1;
                }
                done = r.step >= steps;
            }
            Op::Blacken => {
                r.shown = [[0; 3]; 256];
                r.target = [[0; 3]; 256];
                r.dirty = true;
            }
            Op::LoadPic(path) => match r.pictures.get(&path).and_then(|h| pictures.get(h)) {
                Some(pic) => {
                    r.buffer = fit(&pic.0.pixels, usize::from(pic.0.width), usize::from(pic.0.height), LORES);
                    r.pending = pic.0.palette;
                }
                None => done = false,
            },
            Op::LoadPalette(path) => match r.pictures.get(&path).and_then(|h| pictures.get(h)) {
                Some(pic) => r.pending = pic.0.palette,
                None => done = false,
            },
            Op::UsePalette => r.target = r.pending,
            Op::ShowBuffer => {
                if r.size != LORES {
                    set_size(r, LORES);
                }
                r.screen.clone_from(&r.buffer);
                r.dirty = true;
            }
            Op::Clear => {
                r.screen.fill(0);
                r.dirty = true;
            }
            Op::Lores => set_size(r, LORES),
            Op::Anim(path) => {
                r.anim = r.animations.get(&path).cloned();
                r.anim_frame = 0;
            }
            Op::AnimFrame | Op::AnimKeyFrame(_) => match r.anim.as_ref().and_then(|h| animations.get(h)) {
                Some(ani) => {
                    let ani = &ani.0;
                    if r.size != LORES {
                        set_size(r, LORES);
                    }
                    if let Op::AnimKeyFrame(k) = op {
                        r.anim_frame = k;
                        r.buffer = fit(&ani.frame(k), usize::from(ani.width()), usize::from(ani.height()), LORES);
                        r.screen.clone_from(&r.buffer);
                    } else {
                        r.anim_frame = if r.anim_frame >= ani.chunks.len() { 1 } else { r.anim_frame + 1 };
                        draw_chunk(ani, r.anim_frame, &mut r.screen);
                    }
                    r.dirty = true;
                }
                None => done = false,
            },
            Op::AnimSkip(n) => r.anim_frame = n,
            Op::SetPalette => {
                r.shown = r.pending;
                r.dirty = true;
            }
            Op::Roll(paths) => {
                let loaded: Option<Vec<Vec<u8>>> = paths
                    .iter()
                    .map(|p| {
                        r.pictures.get(p).and_then(|h| pictures.get(h)).map(|p| {
                            fit(&p.0.pixels, usize::from(p.0.width), usize::from(p.0.height), LORES)
                        })
                    })
                    .collect();
                match loaded {
                    Some(mut pages) => {
                        let (w, h) = LORES;
                        let black = vec![0; w * h];
                        pages.insert(0, black.clone());
                        pages.push(black);
                        let total = (pages.len() as u32 - 1) * h as u32;
                        while r.step < total && r.frames >= 1.0 {
                            r.frames -= 1.0;
                            let (page, k) = ((r.step as usize) / h, (r.step as usize) % h + 1);
                            // The page scrolls up by k rows, the next one under it.
                            let top = (h - k) * w;
                            r.screen[..top].copy_from_slice(&pages[page][k * w..]);
                            r.screen[top..].copy_from_slice(&pages[page + 1][..k * w]);
                            r.step += 1;
                        }
                        r.dirty = true;
                        done = r.step >= total;
                    }
                    None => done = false,
                }
            }
            Op::Whiten(a, b) => {
                let pending = r.pending;
                for (shown, p) in r.shown.iter_mut().zip(pending) {
                    *shown = blend(WHITE, p, a, b);
                }
                r.dirty = true;
            }
            Op::Hires(prefix) => {
                let strips: Option<Vec<&Pic>> = (1..=5)
                    .map(|k| r.pictures.get(&format!("{prefix}{k}.PIC")).and_then(|h| pictures.get(h)).map(|p| &p.0))
                    .collect();
                match strips {
                    Some(strips) => {
                        let mut screen = Vec::with_capacity(HIRES.0 * HIRES.1);
                        for s in &strips {
                            screen.extend_from_slice(&s.pixels);
                        }
                        screen.resize(HIRES.0 * HIRES.1, 0);
                        r.pending = strips[0].palette;
                        r.size = HIRES;
                        r.screen = screen;
                        r.dirty = true;
                    }
                    None => done = false,
                }
            }
            Op::Shake(n) => {
                let n = usize::from(n.max(1));
                let (rx, ry) = (crate::game::random(n as u16) as usize, crate::game::random(n as u16) as usize);
                if r.size == LORES {
                    let (w, h) = LORES;
                    for y in 0..h - n {
                        for x in 0..w - n {
                            r.screen[(y + n / 2) * w + x + n / 2] = r.buffer[(y + ry) * w + x + rx];
                        }
                    }
                    r.dirty = true;
                }
            }
            Op::Slide(path) => match r.pictures.get(&path).and_then(|h| pictures.get(h)) {
                Some(pic) => {
                    let new = fit(&pic.0.pixels, usize::from(pic.0.width), usize::from(pic.0.height), LORES);
                    if r.step == 0 {
                        r.slide_from.clone_from(&r.buffer);
                        r.pending = pic.0.palette;
                    }
                    let w = LORES.0;
                    while r.step < 200 && r.frames >= 1.0 {
                        r.frames -= 1.0;
                        // k rows of the old picture are left at the bottom.
                        let k = 199 - r.step as usize;
                        let top = (LORES.1 - k) * w;
                        r.screen[..top].copy_from_slice(&new[k * w..]);
                        r.screen[top..].copy_from_slice(&r.slide_from[..k * w]);
                        r.step += 1;
                    }
                    r.dirty = true;
                    done = r.step >= 200;
                }
                None => done = false,
            },
            Op::FlyBy { planet, ship } => {
                if r.fly.is_none() {
                    let get = |p: &str| r.pictures.get(p).and_then(|h| pictures.get(h)).map(|p| p.0.clone());
                    match (get(&planet), get(&ship)) {
                        (Some(a), Some(b)) => {
                            r.fly = Some((a, b));
                            r.clock = 0.0;
                            r.schedule = 0.0;
                        }
                        _ => {
                            done = false;
                        }
                    }
                }
                if let Some((planet, ship)) = r.fly.clone() {
                    const STEPS: u32 = 0xa8;
                    while r.step < STEPS && r.clock >= r.schedule + 14.0 {
                        r.schedule += 14.0;
                        fly_step(r, &planet, &ship, r.step as usize);
                        r.step += 1;
                    }
                    done = r.step >= STEPS;
                    if done {
                        r.fly = None;
                    }
                }
            }
            Op::End => {
                finish(r, &mut commands);
                return;
            }
        }
        if !done {
            break;
        }
        r.pc += 1;
        r.step = 0;
    }
    if r.dirty {
        r.dirty = false;
        let (w, h) = r.size;
        let mut rgba = Vec::with_capacity(w * h * 4);
        for &p in &r.screen {
            let [cr, cg, cb] = r.shown[usize::from(p)];
            rgba.extend_from_slice(&[cr, cg, cb, 255]);
        }
        let current = images.get(&r.image).map(|i| (i.width() as usize, i.height() as usize));
        if current == Some((w, h)) {
            if let Some(mut image) = images.get_mut(&r.image) {
                image.data = Some(rgba);
            }
        } else {
            r.image = images.add(rgba_image(w as u32, h as u32, rgba));
        }
        // Stills on the screen-resolution sprite, the rest through the canvas.
        let hires = r.size == HIRES;
        for (mut sprite, mut visibility, overlay) in &mut sprites {
            if overlay == hires {
                if sprite.image != r.image {
                    sprite.image = r.image.clone();
                }
                visibility.set_if_neq(Visibility::Inherited);
            } else {
                visibility.set_if_neq(Visibility::Hidden);
            }
        }
    }
}

/// A picture's pixels at `size`, cut or padded.
fn fit(pixels: &[u8], width: usize, height: usize, size: (usize, usize)) -> Vec<u8> {
    let mut out = vec![0; size.0 * size.1];
    for y in 0..height.min(size.1) {
        let n = width.min(size.0);
        out[y * size.0..y * size.0 + n].copy_from_slice(&pixels[y * width..y * width + n]);
    }
    out
}

/// Chunk `k` of an animation over the screen (FUN_116c_01b6).
fn draw_chunk(ani: &Ani, k: usize, screen: &mut [u8]) {
    let (w, h) = (usize::from(ani.width()), usize::from(ani.height()));
    let Some(chunk) = ani.chunks.get(k - 1) else { return };
    if (w, h) == LORES {
        chunk.draw(screen);
    } else {
        let mut pixels = fit(screen, LORES.0, LORES.1, (w, h));
        chunk.draw(&mut pixels);
        for y in 0..h.min(LORES.1) {
            let n = w.min(LORES.0);
            screen[y * LORES.0..y * LORES.0 + n].copy_from_slice(&pixels[y * w..y * w + n]);
        }
    }
}

/// One step of FUN_119b_0366: the stars scroll left two pixels, and the
/// planet, then the ship come in from the right.
fn fly_step(r: &mut Runner, planet: &Pic, ship: &Pic, step: usize) {
    let (w, h) = LORES;
    let offset = (2 * step) % w + 1;
    let planet_x = (((2 * step) as i32 - 0x50) * 5 / 4).max(0) as usize;
    let ship_x = (((2 * step) as i32 - 0xb0) * 2).max(0) as usize;
    let stars = r.buffer.clone();
    for y in 0..h {
        for x in 0..w {
            r.screen[y * w + x] = stars[y * w + (x + offset) % w];
        }
    }
    for (pic, shown) in [(planet, planet_x), (ship, ship_x)] {
        let pw = usize::from(pic.width);
        let shown = shown.min(w).min(pw);
        for y in 0..h.min(usize::from(pic.height)) {
            for x in 0..shown {
                let p = pic.pixels[y * pw + x];
                if p != 0 {
                    r.screen[y * w + w - shown + x] = p;
                }
            }
        }
    }
    r.dirty = true;
}

/// Frames of INTRO/ANIM<n>.ANI (INTRO.PRG DS:0x41 + 7 * n).
const INTRO_FRAMES: [usize; 25] = [
    0, 6, 0, 26, 42, 16, 51, 31, 28, 0, 5, 13, 102, 187, 91, 120, 18, 123, 85, 52, 29, 42, 17, 47, 75,
];
/// The frame order of ANIM11 (DS:0x23-0x44).
const ANIM11_ORDER: [u8; 34] = [
    2, 3, 2, 3, 2, 3, 2, 4, 1, 4, 1, 1, 3, 3, 2, 1, 4, 5, 3, 4, 5, 6, 7, 8, 9, 10, 2, 6, 9, 2, 4, 3, 9, 10,
];

/// INTRO.PRG FUN_1000_0285: the rebels' story, scene by scene with its music.
pub fn intro() -> Vec<Op> {
    use AnimSpecial as A;
    let anim = |s: &mut Script, palette: bool, delay: u32, row: u16, order: u16, n: usize| {
        let special = match n {
            7 => A::KeyFrames,
            12 => A::WaitFirst,
            16 => A::ShakeStart,
            19 => A::Flash,
            _ => A::None,
        };
        let pal = format!("INTRO/PAL{n}.PIC");
        s.anim(palette.then_some(pal.as_str()), delay, row, order, &format!("INTRO/ANIM{n}.ANI"), INTRO_FRAMES[n], special);
    };
    let mut s = Script::default();
    // The fleet (INTRO1-4).
    s.music("INTRO/INTRO1.MOD").lores();
    s.show_pic(0x46, 5, 1, 1, "INTRO/PAL20.PIC");
    anim(&mut s, false, 0x54, 0xf, 1, 0x14);
    anim(&mut s, false, 0x54, 3, 2, 0x15);
    s.fade_out_at(0x1e, 0x10, 2);
    s.hires(100, 0x1e, 2, 10, 1, 1, "INTROP/1X");
    s.hires(100, 0x36, 3, 0x46, 8, 3, "INTROP/2X");
    s.hires(100, 0x24, 4, 0x46, 0x19, 4, "INTROP/3X");
    s.hires(100, 0x38, 6, 0x8c, 2, 6, "INTROP/4X");
    s.wait(0x30, 7);
    s.music("INTRO/INTRO2.MOD");
    s.hires(100, 1, 2, 0x1e, 1, 0, "INTROP/5X");
    s.hires(100, 0x20, 3, 0x46, 0xf, 3, "INTROP/6X");
    s.wait(0x20, 5);
    s.music("INTRO/INTRO3.MOD");
    s.hires(300, 0x12, 1, 0x1e, 1, 0, "INTROP/7X");
    s.hires(5, 1, 3, 0x46, 0x28, 2, "INTROP/8X");
    s.hires(5, 1, 4, 0x46, 0x32, 3, "INTROP/9X");
    s.hires(5, 1, 5, 0x46, 0x32, 4, "INTROP/10X");
    s.hires(100, 0x20, 5, 0x46, 0x14, 5, "INTROP/10_2X");
    s.hires(100, 0x3a, 5, 0x46, 0x2d, 5, "INTROP/11X");
    s.wait(0xe, 7);
    s.music("INTRO/INTRO4.MOD");
    s.hires(0x14, 1, 2, 0x1e, 1, 0, "INTROP/12X");
    s.hires(100, 1, 4, 0x46, 0x10, 3, "INTROP/13X");
    s.hires(100, 1, 6, 0x46, 0x28, 5, "INTROP/14X");
    s.hires(-1, 1, 8, 0x46, 0xc, 7, "INTROP/15X");
    s.fade_out_at(0x46, 0x28, 8);
    s.wait(0x3c, 8);
    // The rebels.
    s.lores().music("INTRO/REBEL.MOD");
    s.show_pic(5, 5, 1, 1, "INTRO/STARS.PIC");
    s.fly_by(6, 1, "INTRO/PLANET.PIC", "INTRO/SHIP.PIC");
    s.show_pic(5, 5, 0x40, 2, "INTRO/PAL12.PIC");
    anim(&mut s, false, 0x9b, 0x10, 3, 0xc);
    anim(&mut s, false, 0x29, 0x38, 4, 0xd);
    s.show_pic(5, 5, 0x18, 6, "INTRO/BASE.PIC");
    s.show_pic(0x28, 99, 0x2e, 6, "INTRO/PAL15.PIC");
    anim(&mut s, false, 0x2a, 8, 8, 0xf);
    anim(&mut s, false, 0x32, 5, 9, 0xe);
    s.wait(6, 10).fade(Fade::Out, 0xb4);
    // The base.
    s.lores().music("INTRO/BASE1.MOD");
    s.show_pic(0xff, 5, 10, 1, "INTRO/BASE0.PIC");
    s.slide(0x20, 1, "INTRO/BASE.PIC");
    s.show_pic(5, 5, 1, 2, "INTRO/PAL1.PIC");
    s.show_pic(5, 5, 1, 3, "INTRO/BASE.PIC");
    s.show_pic(5, 5, 10, 3, "INTRO/PAL11X.PIC");
    s.show_pic(5, 5, 1, 4, "INTRO/PAL1.PIC");
    anim(&mut s, false, 0x2a, 0x37, 4, 1);
    anim(&mut s, false, 0x2a, 0x3f, 4, 1);
    s.load_and_wait(1, 5, "INTRO/PAL1.PIC");
    s.shake(3, 1, 0xf);
    s.show_pic(5, 5, 0xc, 5, "INTRO/BASE.PIC");
    s.wait(0x14, 5).fade(Fade::ToWhite, 2).fade(Fade::HalfWhiteIn, 2);
    s.wait(0x18, 5).fade(Fade::ToWhite, 2).fade(Fade::HalfWhiteIn, 2);
    s.show_pic(5, 5, 0x28, 5, "INTRO/FOLYOSO.PIC");
    s.wait(0x29, 5);
    s.shake(3, 1, 0x14);
    s.show_pic(5, 5, 1, 6, "INTRO/BASE.PIC");
    anim(&mut s, true, 0x54, 1, 7, 3);
    s.show_pic(5, 5, 0x20, 7, "INTRO/MUSZER.PIC");
    s.show_pic(5, 5, 1, 8, "INTRO/BASE.PIC");
    s.show_pic(5, 5, 1, 9, "INTRO/TRACE.PIC");
    anim(&mut s, true, 0x46, 10, 9, 4);
    s.show_pic(5, 5, 10, 10, "INTRO/PAL5.PIC");
    anim(&mut s, false, 0x40, 0x1e, 10, 5);
    s.show_pic(5, 5, 1, 0xb, "INTRO/BASE.PIC");
    anim(&mut s, true, 0x46, 5, 0xc, 6);
    anim(&mut s, true, 0x4a, 1, 0xd, 7);
    anim(&mut s, true, 0x46, 0x24, 0xd, 8);
    s.fade(Fade::HalfWhiteOut, 10).fade(Fade::FromWhite, 0x46);
    s.wait(0x26, 0xe);
    // The alarm.
    s.lores().music("INTRO/BASE2.MOD");
    s.show_pic(5, 5, 0x20, 2, "INTRO/PAL11.PIC");
    s.wait(0x36, 2);
    s.shake(3, 1, 0xf);
    s.wait(0x16, 3);
    s.shake(3, 1, 0xf);
    s.show_pic(5, 5, 1, 4, "INTRO/PAL10.PIC");
    anim(&mut s, false, 0x2a, 0x27, 4, 10);
    s.show_pic(5, 5, 1, 5, "INTRO/PAL11.PIC");
    s.wait(0x16, 5);
    s.shake(3, 1, 0xf);
    s.anim_ordered(0x78, 0x1c, 6, "INTRO/ANIM11.ANI", &ANIM11_ORDER);
    s.show_pic(0, 0, 0, 0, "INTRO/MON1.PIC");
    s.wait_frames(0x28);
    s.show_pic(0, 0, 0, 0, "INTRO/MON2.PIC");
    s.wait_frames(0x28);
    s.show_pic(0, 0, 0, 0, "INTRO/MON3.PIC");
    s.wait(6, 7);
    // The tunnel.
    s.lores().music("INTRO/TUNNEL.MOD");
    s.show_pic(0x46, 5, 1, 1, "INTRO/PAL16.PIC");
    anim(&mut s, false, 0x2a, 1, 3, 0x10);
    anim(&mut s, false, 0x2a, 4, 4, 0x17);
    anim(&mut s, false, 0x2a, 4, 4, 0x18);
    anim(&mut s, false, 0x2a, 2, 5, 0x12);
    anim(&mut s, false, 0x2a, 0x2d, 5, 0x13);
    s.whiten(2, 5).whiten(1, 5).whiten(0, 5);
    s.load("INTRO/ATTACK.PIC").show_buffer();
    s.wait(0x14, 6).use_palette().fade(Fade::FromWhite, 100);
    s.wait(0x40, 6).fade(Fade::Out, 100);
    // The attack (INTRO5-7).
    s.music("INTRO/INTRO5.MOD");
    s.hires(100, 0x20, 1, 0, 0, 0, "INTROP/16X");
    s.hires(-1, 1, 6, 0x46, 0x22, 5, "INTROP/17X");
    s.hires(0x46, 1, 8, 0x46, 0x20, 7, "INTROP/18X");
    s.fade_out_at(0x46, 0x28, 8);
    s.wait(0x3f, 8);
    s.music("INTRO/INTRO6.MOD");
    s.hires(10, 1, 2, 0, 0, 0, "INTROP/19X");
    s.hires(0x14, 0x20, 2, 0x14, 0x18, 2, "INTROP/20X");
    s.hires(0x46, 0x28, 3, 0x46, 10, 3, "INTROP/21X");
    s.fade_out_at(0x46, 0x10, 4);
    s.wait(0x18, 4);
    s.music("INTRO/INTRO7.MOD").lores();
    s.show_pic(0x46, 5, 1, 1, "INTRO/PAL22.PIC");
    anim(&mut s, false, 0x46, 0x3f, 1, 0x16);
    s.fade_out_at(0x46, 1, 3).clear();
    s.show_pic(0x8c, 5, 0x12, 3, "INTRO/LEADER.PIC");
    s.show_pic(0x46, 5, 1, 7, "INTRO/GABOR.PIC");
    s.fade_out_at(0x46, 0x3f, 7);
    s.end()
}

/// CREDITS.PRG FUN_1000_0389: Steal the Sky presents, Grandslam, the title.
pub fn company_credits() -> Vec<Op> {
    let mut s = Script::default();
    s.music("CREDITS/STEAL2.MOD").lores();
    s.credits_anim(true, 1, 1, 3, 165);
    s.wait(0x20, 2);
    s.load("CREDITS/PRESENTS.PIC").fade(Fade::Out, 5).show_buffer();
    s.use_palette().fade(Fade::FromWhite, 0x14);
    s.show_pic(0x46, 5, 1, 3, "CREDITS/GAMEBY.PIC");
    s.hires(0x46, 1, 4, 5, 0x2b, 3, "CREDITS/A");
    s.credits_anim(true, 0x2d, 4, 4, 41);
    s.fade(Fade::ToWhite, 4);
    s.load("CREDITS/REUNION.PIC").show_buffer().use_palette().fade(Fade::FromWhite, 10);
    s.show_pic(0x46, 0xc3, 1, 8, "CREDITS/PAL10.PIC");
    // The names, one after another.
    for (row, order, n) in [
        (0x16, 9, 10), (0x35, 9, 11), (0x17, 10, 12), (0x36, 10, 13), (0x16, 0xb, 14), (0x36, 0xb, 15),
        (0x16, 0xc, 16), (0x37, 0xc, 17), (0x17, 0xd, 18), (0x37, 0xd, 19), (0x17, 0xe, 20), (0x37, 0xe, 21),
        (0x38, 0xf, 22), (0x1c, 0x10, 23), (0x3a, 0x10, 24), (0x17, 0x11, 25), (0x37, 0x11, 26), (0x17, 0x12, 27),
    ] {
        s.credits_anim(false, row, order, n, 30);
    }
    s.load("CREDITS/PAL10.PIC").wait(1, 0x14);
    s.fade(Fade::ToWhite, 5).show_buffer().use_palette().fade(Fade::HalfWhiteIn, 5);
    s.wait(0x3f, 0x25);
    s.end()
}

/// VICTORY.PRG FUN_1000_0161: the way home, and the credits.
pub fn victory() -> Vec<Op> {
    let mut s = Script::default();
    s.music("VICTORY/ENDSEQ.MOD").lores();
    s.show_pic(0x28, 1, 1, 1, "VICTORY/TX1.PIC");
    s.show_pic(0x28, 0x78, 1, 2, "VICTORY/TX2.PIC");
    s.show_pic_then(0x46, 0x50, 0x19, 3, 1, 3, "VICTORY/PAL1.PIC");
    s.anim(None, 0x4a, 0x1e, 3, "VICTORY/ANIM1.ANI", 19, AnimSpecial::None);
    s.load("VICTORY/RETURN.PIC").fade(Fade::ToWhite, 10);
    s.show_buffer().use_palette().fade(Fade::FromWhite, 10);
    s.show_pic(0x28, 0x78, 0x28, 4, "VICTORY/TX3.PIC");
    s.show_pic(0x46, 5, 1, 5, "VICTORY/EARTH.PIC");
    s.show_pic(0x46, 5, 1, 6, "VICTORY/END.PIC");
    s.show_pic(0x46, 5, 1, 8, "VICTORY/TX4.PIC");
    s.show_pic(0x46, 5, 0x20, 8, "VICTORY/TX5.PIC");
    s.wait(0x32, 8).fade(Fade::Out, 0x46);
    s.load("VICTORY/CR1.PIC").clear().push(Op::SetPalette);
    s.push(Op::Roll(vec!["VICTORY/CR1.PIC".into(), "VICTORY/CR2.PIC".into(), "VICTORY/CR3.PIC".into()]));
    s.use_palette().wait(0x3a, 0xb);
    s.end()
}

/// REUNION.PRG FUN_3abd_17dd (screen 35): you've lost. GRAFIKA/DEATHSZ1
/// (lost in a ground war) or 2, your hero's end, and the last picture.
pub fn death(in_ground_war: bool, hero: u16) -> Vec<Op> {
    let mut s = Script::default();
    s.music("ANIM/FAILURE.SPD").lores();
    let first = if in_ground_war { 1 } else { 2 };
    s.load(&format!("GRAFIKA/DEATHSZ{first}.PIC")).show_buffer().use_palette().fade(Fade::In, 0x8c);
    s.wait_frames(0x2d0).fade(Fade::Out, 0x46);
    s.load(&format!("GRAFIKA/DEATH{hero}.PIC")).show_buffer().use_palette().fade(Fade::In, 0x8c);
    if hero == 2 {
        // ANIM/MAIN15, 4 frames round and round (frame over frame).
        s.push(Op::Anim("ANIM/MAIN15.ANI".into())).push(Op::AnimSkip(1));
        for _ in 0..0xed {
            s.push(Op::AnimFrame).wait_frames(7);
        }
    } else {
        s.wait_frames(0x67c);
    }
    s.fade(Fade::Out, 0x46);
    s.load("GRAFIKA/DEATH.PIC").show_buffer().wait_frames(0x46).use_palette().fade(Fade::In, 0x46);
    s.push(Op::Anim("ANIM/MAIN16.ANI".into())).push(Op::AnimSkip(1));
    for _ in 0..0x3d {
        s.push(Op::AnimFrame).wait_frames(12);
    }
    s.fade(Fade::Out, 0x118);
    s.end()
}
