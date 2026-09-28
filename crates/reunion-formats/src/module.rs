//! ProTracker `.MOD` music (INTRO, CREDITS, VICTORY): 31 instruments, 4
//! channels ("M.K."), and a player that renders it to stereo samples.
//!
//! ```text
//! 0     20   title
//! 20    930  31 instruments of 30 bytes: name (22), length in words (u16 BE),
//!            finetune (low nibble, signed), volume (0-64), loop start and
//!            loop length in words (u16 BE)
//! 950   1    song length (orders played)
//! 951   1    restart position (unused)
//! 952   128  order table: pattern numbers
//! 1080  4    b"M.K."
//! 1084  ..   patterns: 64 rows x 4 channels x 4 bytes
//!            (instrument hi nibble, period 12 bits, instrument lo nibble,
//!            effect 4 bits, parameter 8 bits)
//! ..    ..   instrument samples, signed 8 bits, in order
//! ```

use thiserror::Error;

#[derive(Debug, Error)]
pub enum ModError {
    #[error("not a 4-channel ProTracker module")]
    BadMagic,
    #[error("file too short")]
    Truncated,
}

const CHANNELS: usize = 4;
const ROWS: usize = 64;
const PATTERN_LEN: usize = ROWS * CHANNELS * 4;
/// The Amiga's PAL clock, for turning periods into frequencies.
const PAL_CLOCK: f64 = 7_093_789.2;

#[derive(Debug, Clone, Default)]
pub struct Instrument {
    pub name: String,
    /// Signed 8-bit samples.
    pub data: Vec<i8>,
    pub finetune: i8,
    pub volume: u8,
    pub loop_start: usize,
    /// 0 when the instrument doesn't loop.
    pub loop_len: usize,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Note {
    pub instrument: u8,
    pub period: u16,
    pub effect: u8,
    pub param: u8,
}

#[derive(Debug, Clone)]
pub struct Module {
    pub title: String,
    pub instruments: Vec<Instrument>,
    pub orders: Vec<u8>,
    /// Pattern rows, `[pattern][row * CHANNELS + channel]`.
    pub patterns: Vec<Vec<Note>>,
}

fn text(bytes: &[u8]) -> String {
    let end = bytes.iter().position(|&b| b == 0).unwrap_or(bytes.len());
    String::from_utf8_lossy(&bytes[..end]).trim_end().to_string()
}

impl Module {
    pub fn parse(data: &[u8]) -> Result<Self, ModError> {
        if data.get(1080..1084) != Some(b"M.K.".as_slice()) {
            return Err(ModError::BadMagic);
        }
        let word = |at: usize| usize::from(u16::from_be_bytes([data[at], data[at + 1]])) * 2;
        let song_len = usize::from(data[950]).clamp(1, 128);
        let orders = data[952..952 + song_len].to_vec();
        let pattern_count = usize::from(*data[952..1080].iter().max().unwrap_or(&0)) + 1;
        let mut at = 1084;
        let mut patterns = Vec::with_capacity(pattern_count);
        for _ in 0..pattern_count {
            let raw = data.get(at..at + PATTERN_LEN).ok_or(ModError::Truncated)?;
            patterns.push(
                raw.chunks_exact(4)
                    .map(|b| Note {
                        instrument: (b[0] & 0xf0) | (b[2] >> 4),
                        period: u16::from(b[0] & 0x0f) << 8 | u16::from(b[1]),
                        effect: b[2] & 0x0f,
                        param: b[3],
                    })
                    .collect(),
            );
            at += PATTERN_LEN;
        }
        let mut instruments = Vec::with_capacity(31);
        for i in 0..31 {
            let h = 20 + 30 * i;
            let len = word(h + 22);
            let sample = data.get(at..(at + len).min(data.len())).unwrap_or(&[]);
            at += len;
            let loop_start = word(h + 26);
            let loop_len = word(h + 28);
            instruments.push(Instrument {
                name: text(&data[h..h + 22]),
                data: sample.iter().map(|&b| b as i8).collect(),
                finetune: ((data[h + 24] & 0x0f) << 4) as i8 >> 4,
                volume: data[h + 25].min(64),
                loop_start,
                loop_len: if loop_len > 2 && loop_start < sample.len() {
                    loop_len.min(sample.len() - loop_start)
                } else {
                    0
                },
            });
        }
        Ok(Self {
            title: text(&data[..20]),
            instruments,
            orders,
            patterns,
        })
    }
}

const SINE: [u8; 32] = [
    0, 24, 49, 74, 97, 120, 141, 161, 180, 197, 212, 224, 235, 244, 250, 253, 255, 253, 250, 244,
    235, 224, 212, 197, 180, 161, 141, 120, 97, 74, 49, 24,
];

/// Vibrato and tremolo waveforms (E4x / E7x), `pos` 0-63.
fn wave(shape: u8, pos: u8) -> i32 {
    let pos = pos & 63;
    let value = match shape & 3 {
        // Ramp down.
        1 => 255 - i32::from(pos) * 8,
        // Square.
        2 => 255,
        _ => i32::from(SINE[usize::from(pos & 31)]),
    };
    if shape & 3 == 1 {
        value.clamp(-255, 255)
    } else if pos >= 32 {
        -value
    } else {
        value
    }
}

#[derive(Debug, Clone, Default)]
struct Channel {
    /// 1-based, 0 = none yet.
    instrument: usize,
    playing: bool,
    /// Position in the sample, in samples.
    pos: f64,
    period: i32,
    /// The period that sounds this tick (after vibrato and arpeggio).
    out_period: i32,
    volume: i32,
    out_volume: i32,
    finetune: i8,
    note: Note,
    porta_target: i32,
    porta_speed: i32,
    vibrato: (u8, u8, u8),
    vibrato_shape: u8,
    tremolo: (u8, u8, u8),
    tremolo_shape: u8,
    offset: usize,
    loop_row: usize,
    loop_count: u8,
    pan: f32,
}

/// Plays a [`Module`] into interleaved stereo `f32` samples.
pub struct Player {
    module: Module,
    rate: u32,
    channels: [Channel; CHANNELS],
    order: usize,
    row: usize,
    tick: u32,
    speed: u32,
    tempo: u32,
    /// Output samples left in the current tick.
    tick_left: usize,
    pattern_delay: u32,
    /// Row to jump to after this one: (order, row).
    jump: Option<(usize, usize)>,
    /// Whether the song starts over after its last order.
    pub looping: bool,
    finished: bool,
}

impl Player {
    pub fn new(module: Module, rate: u32) -> Self {
        let mut channels: [Channel; CHANNELS] = Default::default();
        // Amiga panning: left, right, right, left, not fully apart for headphones.
        for (i, c) in channels.iter_mut().enumerate() {
            c.pan = if i == 0 || i == 3 { 0.25 } else { 0.75 };
        }
        let mut player = Self {
            module,
            rate,
            channels,
            order: 0,
            row: 0,
            tick: 0,
            speed: 6,
            tempo: 125,
            tick_left: 0,
            pattern_delay: 0,
            jump: None,
            looping: true,
            finished: false,
        };
        player.start_row();
        player
    }

    pub fn finished(&self) -> bool {
        self.finished
    }

    /// Where the song is: order position and row, both 0-based.
    pub fn position(&self) -> (usize, usize) {
        (self.order, self.row)
    }

    fn tick_samples(&self) -> usize {
        (self.rate as usize * 5) / (self.tempo.max(32) as usize * 2)
    }

    fn tuned(&self, period: i32, finetune: i8) -> i32 {
        (f64::from(period) * 2f64.powf(-f64::from(finetune) / 96.0)).round() as i32
    }

    fn start_row(&mut self) {
        let pattern = usize::from(self.module.orders.get(self.order).copied().unwrap_or(0));
        let Some(rows) = self.module.patterns.get(pattern) else {
            self.finished = true;
            return;
        };
        let notes: [Note; CHANNELS] = std::array::from_fn(|c| rows[self.row * CHANNELS + c]);
        for (c, note) in notes.into_iter().enumerate() {
            self.start_note(c, note);
        }
        self.tick_left = self.tick_samples();
    }

    fn start_note(&mut self, c: usize, note: Note) {
        let (effect, param) = (note.effect, note.param);
        let (x, y) = (param >> 4, param & 0x0f);
        let delayed = effect == 0xe && x == 0xd && y > 0;
        {
            let ch = &mut self.channels[c];
            ch.note = note;
            if note.instrument > 0 {
                let instrument = &self.module.instruments[usize::from(note.instrument) - 1];
                ch.instrument = usize::from(note.instrument);
                ch.volume = i32::from(instrument.volume);
                ch.finetune = instrument.finetune;
            }
            if effect == 0xe && x == 5 {
                ch.finetune = ((y << 4) as i8) >> 4;
            }
        }
        if note.period > 0 && !delayed {
            let period = self.tuned(i32::from(note.period), self.channels[c].finetune);
            let ch = &mut self.channels[c];
            if effect == 3 || effect == 5 {
                ch.porta_target = period;
            } else {
                ch.period = period;
                ch.pos = 0.0;
                ch.playing = ch.instrument > 0;
                if ch.vibrato_shape < 4 {
                    ch.vibrato.2 = 0;
                }
                if ch.tremolo_shape < 4 {
                    ch.tremolo.2 = 0;
                }
                if effect == 9 {
                    if param > 0 {
                        ch.offset = usize::from(param) * 256;
                    }
                    ch.pos = ch.offset as f64;
                }
            }
        }
        let ch = &mut self.channels[c];
        match effect {
            3
                if param > 0 => {
                    ch.porta_speed = i32::from(param);
                }
            4 => {
                if x > 0 {
                    ch.vibrato.0 = x;
                }
                if y > 0 {
                    ch.vibrato.1 = y;
                }
            }
            7 => {
                if x > 0 {
                    ch.tremolo.0 = x;
                }
                if y > 0 {
                    ch.tremolo.1 = y;
                }
            }
            8 => ch.pan = f32::from(param) / 255.0,
            0xb => self.jump = Some((usize::from(param), 0)),
            0xc => ch.volume = i32::from(param.min(64)),
            0xd => {
                let row = usize::from(x) * 10 + usize::from(y);
                let order = self.jump.map_or(self.order + 1, |j| j.0);
                self.jump = Some((order, if row < ROWS { row } else { 0 }));
            }
            0xe => match x {
                1 => ch.period = (ch.period - i32::from(y)).max(113),
                2 => ch.period = (ch.period + i32::from(y)).min(856 * 2),
                4 => ch.vibrato_shape = y,
                6 => {
                    if y == 0 {
                        ch.loop_row = self.row;
                    } else {
                        if ch.loop_count == 0 {
                            ch.loop_count = y;
                        } else {
                            ch.loop_count -= 1;
                        }
                        if ch.loop_count > 0 {
                            self.jump = Some((self.order, ch.loop_row));
                        }
                    }
                }
                7 => ch.tremolo_shape = y,
                0xa => ch.volume = (ch.volume + i32::from(y)).min(64),
                0xb => ch.volume = (ch.volume - i32::from(y)).max(0),
                0xe => self.pattern_delay = u32::from(y),
                _ => {}
            },
            0xf => {
                if (1..32).contains(&param) {
                    self.speed = u32::from(param);
                } else if param >= 32 {
                    self.tempo = u32::from(param);
                }
            }
            _ => {}
        }
        ch.out_period = ch.period;
        ch.out_volume = ch.volume;
    }

    /// Effects that run on every tick after the first.
    fn tick_effects(&mut self, c: usize) {
        let tick = self.tick;
        let note = self.channels[c].note;
        if note.effect == 0xe && note.param >> 4 == 0xd && tick == u32::from(note.param & 0x0f) {
            // The delayed note starts now.
            self.start_note(c, Note { effect: 0, param: 0, ..note });
            return;
        }
        let (x, y) = (note.param >> 4, note.param & 0x0f);
        let ch = &mut self.channels[c];
        ch.out_period = ch.period;
        ch.out_volume = ch.volume;
        let volume_slide = |ch: &mut Channel| {
            ch.volume = if x > 0 {
                (ch.volume + i32::from(x)).min(64)
            } else {
                (ch.volume - i32::from(y)).max(0)
            };
            ch.out_volume = ch.volume;
        };
        let tone_porta = |ch: &mut Channel| {
            if ch.porta_target > 0 {
                if ch.period < ch.porta_target {
                    ch.period = (ch.period + ch.porta_speed).min(ch.porta_target);
                } else {
                    ch.period = (ch.period - ch.porta_speed).max(ch.porta_target);
                }
                ch.out_period = ch.period;
            }
        };
        let vibrato = |ch: &mut Channel| {
            let (speed, depth, pos) = ch.vibrato;
            ch.out_period = ch.period + wave(ch.vibrato_shape, pos) * i32::from(depth) / 128;
            ch.vibrato.2 = pos.wrapping_add(speed) & 63;
        };
        match note.effect {
            0 if note.param > 0 => {
                let step = [0, x, y][(tick % 3) as usize];
                ch.out_period = (f64::from(ch.period) / 2f64.powf(f64::from(step) / 12.0)) as i32;
            }
            1 => {
                ch.period = (ch.period - i32::from(note.param)).max(113);
                ch.out_period = ch.period;
            }
            2 => {
                ch.period = (ch.period + i32::from(note.param)).min(856 * 2);
                ch.out_period = ch.period;
            }
            3 => tone_porta(ch),
            4 => vibrato(ch),
            5 => {
                tone_porta(ch);
                volume_slide(ch);
            }
            6 => {
                vibrato(ch);
                volume_slide(ch);
            }
            7 => {
                let (speed, depth, pos) = ch.tremolo;
                ch.out_volume = (ch.volume + wave(ch.tremolo_shape, pos) * i32::from(depth) / 64).clamp(0, 64);
                ch.tremolo.2 = pos.wrapping_add(speed) & 63;
            }
            0xa => volume_slide(ch),
            0xe => match x {
                9 if y > 0 && tick.is_multiple_of(u32::from(y)) => ch.pos = 0.0,
                0xc if tick == u32::from(y) => {
                    ch.volume = 0;
                    ch.out_volume = 0;
                }
                _ => {}
            },
            _ => {}
        }
    }

    fn next_tick(&mut self) {
        self.tick += 1;
        if self.tick >= self.speed * (1 + self.pattern_delay) {
            self.tick = 0;
            self.pattern_delay = 0;
            self.next_row();
            if !self.finished {
                self.start_row();
            }
            return;
        }
        if !self.tick.is_multiple_of(self.speed) {
            for c in 0..CHANNELS {
                self.tick_effects(c);
            }
        }
        self.tick_left = self.tick_samples();
    }

    fn next_row(&mut self) {
        if let Some((order, row)) = self.jump.take() {
            if order <= self.order && !self.looping && order != self.order {
                self.finished = true;
                return;
            }
            self.order = order;
            self.row = row;
        } else {
            self.row += 1;
            if self.row >= ROWS {
                self.row = 0;
                self.order += 1;
            }
        }
        if self.order >= self.module.orders.len() {
            if self.looping {
                self.order = 0;
                self.channels.iter_mut().for_each(|c| c.loop_count = 0);
            } else {
                self.finished = true;
            }
        }
    }

    /// Fills `out` with interleaved stereo samples; silence once finished.
    pub fn render(&mut self, out: &mut [f32]) {
        for frame in out.chunks_exact_mut(2) {
            if self.finished {
                frame.fill(0.0);
                continue;
            }
            while self.tick_left == 0 {
                self.next_tick();
                if self.finished {
                    break;
                }
            }
            self.tick_left = self.tick_left.saturating_sub(1);
            let (mut left, mut right) = (0.0f32, 0.0f32);
            for ch in &mut self.channels {
                if !ch.playing || ch.instrument == 0 || ch.out_period <= 0 {
                    continue;
                }
                let instrument = &self.module.instruments[ch.instrument - 1];
                let data = &instrument.data;
                let end = if instrument.loop_len > 0 {
                    instrument.loop_start + instrument.loop_len
                } else {
                    data.len()
                };
                if ch.pos >= end as f64 {
                    if instrument.loop_len > 0 {
                        ch.pos = instrument.loop_start as f64
                            + (ch.pos - end as f64) % instrument.loop_len as f64;
                    } else {
                        ch.playing = false;
                        continue;
                    }
                }
                let i = ch.pos as usize;
                let frac = (ch.pos - i as f64) as f32;
                let next = if i + 1 < end {
                    data[i + 1]
                } else if instrument.loop_len > 0 {
                    data[instrument.loop_start]
                } else {
                    0
                };
                let s = f32::from(data[i]) * (1.0 - frac) + f32::from(next) * frac;
                let s = s / 128.0 * ch.out_volume as f32 / 64.0;
                left += s * (1.0 - ch.pan);
                right += s * ch.pan;
                ch.pos += PAL_CLOCK / (f64::from(ch.out_period) * 2.0) / f64::from(self.rate);
            }
            frame[0] = left * 0.5;
            frame[1] = right * 0.5;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tiny_module() -> Vec<u8> {
        let mut data = vec![0u8; 1084 + PATTERN_LEN + 64];
        data[..4].copy_from_slice(b"test");
        // Instrument 1: 64 samples, full volume, looping all of it.
        data[20 + 22..20 + 24].copy_from_slice(&32u16.to_be_bytes());
        data[20 + 25] = 64;
        data[20 + 28..20 + 30].copy_from_slice(&32u16.to_be_bytes());
        data[950] = 1;
        data[1080..1084].copy_from_slice(b"M.K.");
        // Row 0, channel 0: instrument 1, period 428 (C-2).
        data[1084..1088].copy_from_slice(&[0x01, 0xac, 0x10, 0x00]);
        for (i, b) in data[1084 + PATTERN_LEN..].iter_mut().enumerate() {
            *b = if i < 32 { 100 } else { 156 }; // a square wave
        }
        data
    }

    #[test]
    fn parses_and_plays() {
        let module = Module::parse(&tiny_module()).unwrap();
        assert_eq!(module.title, "test");
        assert_eq!(module.instruments[0].data.len(), 64);
        assert_eq!(module.instruments[0].loop_len, 64);
        assert_eq!(module.patterns[0][0].period, 428);
        let mut player = Player::new(module, 44100);
        let mut out = vec![0.0; 2048];
        player.render(&mut out);
        assert!(out.iter().any(|&s| s.abs() > 0.1));
    }

    #[test]
    fn stops_at_the_end_when_not_looping() {
        let module = Module::parse(&tiny_module()).unwrap();
        let mut player = Player::new(module, 8000);
        player.looping = false;
        // One pattern: 64 rows x 6 ticks x 8000 * 2.5 / 125 samples = 61440.
        let mut out = vec![0.0; 2 * 70000];
        player.render(&mut out);
        assert!(player.finished());
    }
}
