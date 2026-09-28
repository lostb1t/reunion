//! The space battle's right half (REUNION.PRG FUN_20d4_0985, _0a85, _0c44):
//! clips of battle footage while the fight goes on.
//!
//! WAR/ANIM/ANIM.DEF holds 25 clips of 140 steps (4 bytes each): the
//! animation (WAR/WAR/SA<n>.ANI), which of its frames, a sound
//! (SOUND2/WARSND<n>, 0 for none) and how many more frames to hold it; a
//! hold of -1 ends the clip. Where each animation goes is in DS:0x555b
//! (5 bytes per animation: frames, width, height, x, y).
//!
//! The battle opens on a random clip. A finished clip plays again as often
//! as DS:0x569a says, then another one comes that hasn't been shown yet
//! (all again once each has been): the panel closes in from the edges in
//! black, 10 pixels a frame, and after 10-19 frames the new clip starts.

use crate::exe::GameExe;
use crate::sim::Random;

pub const CLIPS: usize = 25;
const STEPS: usize = 140;
const STEP_LEN: usize = 4;
/// The panel: x 160-319, y 49-199.
pub const PANEL: (i32, i32, i32, i32) = (160, 319, 49, 199);

/// Where an animation of the battle clips goes (DS:0x555b + 5 * n).
#[derive(Debug, Clone, Copy, Default)]
pub struct Placement {
    pub frames: u8,
    pub width: u8,
    pub height: u8,
    pub x: u8,
    pub y: u8,
}

impl GameExe {
    pub fn clip_placement(&self, animation: u8) -> Placement {
        let Some(b) = self.ds_bytes(0x555b + 5 * u16::from(animation), 5) else {
            return Placement::default();
        };
        Placement {
            frames: b[0],
            width: b[1],
            height: b[2],
            x: b[3],
            y: b[4],
        }
    }

    /// How many times clip `clip` plays again (DS:0x569a).
    fn clip_repeats(&self, clip: usize) -> u16 {
        self.ds_bytes(0x569a + 2 * clip as u16, 2)
            .map_or(0, |b| u16::from_le_bytes([b[0], b[1]]))
    }
}

/// What the panel should show this frame.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ViewEvent {
    /// Frame `frame` (1-based) of WAR/WAR/SA<animation>.ANI at its placement.
    Draw { animation: u8, frame: u8 },
    Sound(u8),
    /// Black over this screen rectangle (x, y, width, height).
    Fill(i32, i32, i32, i32),
}

pub struct BattleView {
    clips: Vec<Vec<[u8; STEP_LEN]>>,
    repeats: Vec<u16>,
    clip: usize,
    /// 1-based, like DS:0x7794.
    step: usize,
    hold: i32,
    repeats_left: u16,
    /// Clips 2-25 not shown yet (DS:0x77f6), bit n for clip n.
    unseen: u32,
    /// The closing panel: left, right, top, bottom (DS:0x77ee-0x77f4).
    wipe: Option<(i32, i32, i32, i32)>,
}

const ALL_CLIPS: u32 = ((1 << (CLIPS + 1)) - 1) & !0b11;

impl BattleView {
    /// FUN_20d4_0c44: the first clip, and what it shows first.
    pub fn new(anim_def: &[u8], exe: &GameExe, random: Random) -> (Self, Vec<ViewEvent>) {
        let clips = anim_def
            .chunks(STEPS * STEP_LEN)
            .take(CLIPS)
            .map(|c| c.chunks_exact(STEP_LEN).map(|s| [s[0], s[1], s[2], s[3]]).collect())
            .collect();
        let mut view = Self {
            clips,
            repeats: (0..=CLIPS).map(|c| exe.clip_repeats(c)).collect(),
            clip: 0,
            step: 1,
            hold: 0,
            repeats_left: 0,
            unseen: ALL_CLIPS,
            wipe: None,
        };
        let first = usize::from(random(CLIPS as u16)) + 1;
        view.start(first);
        view.unseen &= !(1 << first);
        let events = view.show_step();
        (view, events)
    }

    fn start(&mut self, clip: usize) {
        self.clip = clip;
        self.step = 1;
        self.hold = 0;
        self.repeats_left = self.repeats.get(clip).copied().unwrap_or(0);
    }

    fn entry(&self) -> Option<[u8; STEP_LEN]> {
        self.clips.get(self.clip - 1)?.get(self.step - 1).copied()
    }

    /// Draws the current step and plays its sound; sets its hold.
    fn show_step(&mut self) -> Vec<ViewEvent> {
        let mut events = Vec::new();
        let Some([animation, frame, sound, hold]) = self.entry() else {
            return events;
        };
        if animation != 0 {
            events.push(ViewEvent::Draw { animation, frame });
            if sound != 0 {
                events.push(ViewEvent::Sound(sound));
            }
        }
        self.hold = i32::from(hold as i8);
        events
    }

    /// FUN_20d4_0a85, once a frame while the battle goes on.
    pub fn frame(&mut self, random: Random) -> Vec<ViewEvent> {
        let mut events = self.wipe_step();
        if self.hold != 0 {
            self.hold -= 1;
            return events;
        }
        self.step += 1;
        let ended = self.entry().is_none_or(|e| e[3] == 0xff);
        if ended {
            if self.unseen == 0 {
                self.unseen = ALL_CLIPS;
            }
            let next = if self.repeats_left == 0 {
                let mut clip = usize::from(random(CLIPS as u16)) + 1;
                while self.unseen & (1 << clip) == 0 {
                    clip = usize::from(random(CLIPS as u16 - 1)) + 2;
                }
                self.unseen &= !(1 << clip);
                clip
            } else {
                self.repeats_left -= 1;
                self.clip
            };
            if next != self.clip {
                let (left, right, top, bottom) = PANEL;
                self.wipe = Some((left, right, top, bottom));
                self.start(next);
                self.hold = i32::from(random(10)) + 10;
                return events;
            }
            self.step = 1;
        }
        events.extend(self.show_step());
        events
    }

    /// The panel closing in: 10-pixel black bands at its edges.
    fn wipe_step(&mut self) -> Vec<ViewEvent> {
        let Some((left, right, top, bottom)) = self.wipe else {
            return Vec::new();
        };
        let (width, height) = (right - left + 1, bottom - top + 1);
        if width < 21 || height < 21 {
            self.wipe = None;
            return vec![ViewEvent::Fill(left, top, width, height)];
        }
        self.wipe = Some((left + 10, right - 10, top + 10, bottom - 10));
        vec![
            ViewEvent::Fill(left, top, width, 10),
            ViewEvent::Fill(left, bottom - 10, width, 11),
            ViewEvent::Fill(left, top, 10, height),
            ViewEvent::Fill(right - 10, top, 11, height),
        ]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn all_clips_are_2_to_25() {
        assert_eq!(ALL_CLIPS.count_ones(), 24);
        assert_eq!(ALL_CLIPS & 0b11, 0);
        assert_ne!(ALL_CLIPS & (1 << 25), 0);
    }
}
