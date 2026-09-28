//! In-game animations (ANIM/MAIN<n>.ANI, REUNION.PRG FUN_3abd_0c05 /
//! _0e5d): each frame's changes drawn over the last (FUN_3c64_0516), in
//! the colours of the picture they play over, at their place from
//! DS:0x55d7 + 7 * n.

use std::collections::VecDeque;

use bevy::prelude::*;
use bevy::sprite::Anchor;

use crate::audio::Sfx;
use crate::pic::{Animation, Palette, rgba_image};
use crate::screen::place;

pub struct AnimPlayerPlugin;

impl Plugin for AnimPlayerPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Update, play);
    }
}

const VGA_HZ: f64 = 70.086;

/// 70 Hz frames after frame k.
#[derive(Clone, Copy)]
pub enum Delay {
    Fixed(u32),
    PerFrame(fn(usize) -> u32),
}

impl Delay {
    fn after(self, k: usize) -> u32 {
        match self {
            Delay::Fixed(n) => n,
            Delay::PerFrame(f) => f(k),
        }
    }
}

/// Frames `from..=to` of an animation, `delay` apart.
pub struct Segment {
    pub animation: Handle<Animation>,
    pub from: usize,
    pub to: usize,
    pub delay: Delay,
    /// Sounds (`SOUND/<name>`) as frames come up.
    pub sounds: Vec<(usize, &'static str)>,
    /// A sound when the segment is over.
    pub end_sound: Option<&'static str>,
    /// Round and round instead of ending.
    pub looping: bool,
}

#[derive(Component)]
pub struct AnimPlayer {
    pub segments: VecDeque<Segment>,
    pub palette: Handle<Palette>,
    /// The part of the frame shown, if not all of it (FUN_3abd_0c70 draws
    /// the inside only).
    pub crop: Option<Rect>,
    frame: usize,
    wait: f64,
    pixels: Vec<u8>,
    size: (usize, usize),
    image: Option<Handle<Image>>,
}

impl AnimPlayer {
    pub fn new(segments: Vec<Segment>, palette: Handle<Palette>) -> Self {
        Self {
            segments: segments.into(),
            palette,
            crop: None,
            frame: 0,
            wait: 0.0,
            pixels: Vec::new(),
            size: (0, 0),
            image: None,
        }
    }

    pub fn with_crop(mut self, crop: Rect) -> Self {
        self.crop = Some(crop);
        self
    }
}

/// An animation's place on the screen (DS:0x55db, 0x55dd).
pub fn position(exe: &reunion_formats::exe::GameExe, n: u16) -> Vec2 {
    let b = exe.ds_bytes(0x55d7 + 7 * n, 7).unwrap_or(&[0; 7]);
    Vec2::new(f32::from(u16::from_le_bytes([b[4], b[5]])), f32::from(b[6]))
}

fn play(
    time: Res<Time>,
    mut players: Query<(&mut AnimPlayer, &mut Sprite)>,
    animations: Res<Assets<Animation>>,
    palettes: Res<Assets<Palette>>,
    mut images: ResMut<Assets<Image>>,
    mut commands: Commands,
) {
    let frames = time.delta_secs_f64() * VGA_HZ;
    for (mut player, mut sprite) in &mut players {
        let player = &mut *player;
        let Some(palette) = palettes.get(&player.palette) else { continue };
        player.wait -= frames;
        let mut changed = false;
        while player.wait <= 0.0 {
            let Some(segment) = player.segments.front() else { break };
            let Some(ani) = animations.get(&segment.animation) else { break };
            let ani = &ani.0;
            let size = (usize::from(ani.width()), usize::from(ani.height()));
            if player.size != size {
                // Starts from the first frame, which is the picture underneath.
                player.size = size;
                player.pixels = ani.frame(1);
            }
            if player.frame == 0 {
                player.frame = segment.from;
            }
            let k = player.frame;
            if k > segment.to || k > ani.chunks.len() {
                if let Some(sound) = segment.end_sound {
                    commands.trigger(Sfx::named(sound));
                }
                if segment.looping {
                    player.frame = 1;
                    continue;
                }
                player.segments.pop_front();
                player.frame = 0;
                continue;
            }
            if let Some(chunk) = ani.chunks.get(k - 1) {
                chunk.draw(&mut player.pixels);
                changed = true;
            }
            for &(at, sound) in &segment.sounds {
                if at == k {
                    commands.trigger(Sfx::named(sound));
                }
            }
            player.wait += f64::from(segment.delay.after(k));
            player.frame += 1;
        }
        if changed {
            let rgba: Vec<u8> = player
                .pixels
                .iter()
                .flat_map(|&p| {
                    let [r, g, b] = palette.0[usize::from(p)];
                    [r, g, b, 255]
                })
                .collect();
            let (w, h) = player.size;
            if let Some(mut image) = player.image.as_ref().and_then(|i| images.get_mut(i)) {
                image.data = Some(rgba);
            } else {
                let handle = images.add(rgba_image(w as u32, h as u32, rgba));
                sprite.image = handle.clone();
                sprite.rect = player.crop;
                player.image = Some(handle);
            }
        }
    }
}

/// An animation player at `at` in game coordinates, drawn at `z`.
pub fn anim_player(player: AnimPlayer, at: Vec2, z: f32) -> impl Bundle {
    (player, Sprite::default(), Anchor::TOP_LEFT, place(at, z))
}
