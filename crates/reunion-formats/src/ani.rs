//! "SpidyAnim" `.ANI` animations (ANIM, INTRO, CREDITS, VICTORY, WAR/WAR,
//! WAR/GRWAR): a series of chunks, the first a whole picture and the rest
//! changes to draw over it (REUNION.PRG FUN_405f_0af8, FUN_3c64_057c).
//!
//! ```text
//! chunk:
//! 0     2    length - 13 (u16 LE)
//! 2     9    b"SpidyAnim"
//! 11    2    width (u16 LE)
//! 13    2    height (u16 LE)
//! 15    ..   pixels, row by row, until width * height are covered:
//!            b < 0x80        one pixel b
//!            0x80, n (u16)   skip n pixels
//!            0x81-0xbf       skip b & 0x3f pixels
//!            0xc0, n (u16), v   n pixels v
//!            0xc1-0xff, v    b & 0x3f pixels v
//! ```
//!
//! Pixel values are palette entries of the picture the animation plays over,
//! which the original loads 0x40 up (see `pic`).

use thiserror::Error;

const MAGIC: &[u8] = b"SpidyAnim";
const HEADER_LEN: usize = 13;

#[derive(Debug, Error)]
pub enum AniError {
    #[error("not a SpidyAnim animation")]
    BadMagic,
    #[error("file too short")]
    Truncated,
}

#[derive(Debug, Clone)]
pub struct Chunk {
    pub width: u16,
    pub height: u16,
    data: Vec<u8>,
}

impl Chunk {
    /// Draws the chunk into a `width * height` picture of palette indices.
    /// Skipped pixels keep what was there.
    pub fn draw(&self, pixels: &mut [u8]) {
        let mut left = usize::from(self.width) * usize::from(self.height);
        let (mut at, mut i) = (0usize, 0usize);
        let data = &self.data;
        let byte = |i: usize| data.get(i).copied().unwrap_or(0);
        let word = |i: usize| usize::from(byte(i)) | usize::from(byte(i + 1)) << 8;
        let mut fill = |at: usize, n: usize, v: u8| {
            let end = (at + n).min(pixels.len());
            if at < end {
                pixels[at..end].fill(v);
            }
        };
        while left > 0 && i < data.len() {
            let b = byte(i);
            i += 1;
            let n = match b {
                0..=0x7f => {
                    fill(at, 1, b);
                    1
                }
                0x80 => {
                    i += 2;
                    word(i - 2)
                }
                0x81..=0xbf => usize::from(b & 0x3f),
                0xc0 => {
                    let (n, v) = (word(i), byte(i + 2));
                    i += 3;
                    fill(at, n, v);
                    n
                }
                _ => {
                    let n = usize::from(b & 0x3f);
                    fill(at, n, byte(i));
                    i += 1;
                    n
                }
            };
            at += n;
            left = left.saturating_sub(n);
        }
    }
}

#[derive(Debug, Clone)]
pub struct Ani {
    pub chunks: Vec<Chunk>,
}

impl Ani {
    pub fn parse(data: &[u8]) -> Result<Self, AniError> {
        let mut chunks = Vec::new();
        let mut at = 0;
        while at + 2 <= data.len() {
            let len = usize::from(u16::from_le_bytes([data[at], data[at + 1]])) + HEADER_LEN;
            let chunk = data.get(at + 2..at + 2 + len).ok_or(AniError::Truncated)?;
            if !chunk.starts_with(MAGIC) {
                return Err(AniError::BadMagic);
            }
            chunks.push(Chunk {
                width: u16::from_le_bytes([chunk[9], chunk[10]]),
                height: u16::from_le_bytes([chunk[11], chunk[12]]),
                data: chunk[HEADER_LEN..].to_vec(),
            });
            at += 2 + len;
        }
        if chunks.is_empty() {
            return Err(AniError::Truncated);
        }
        Ok(Self { chunks })
    }

    pub fn width(&self) -> u16 {
        self.chunks[0].width
    }

    pub fn height(&self) -> u16 {
        self.chunks[0].height
    }

    /// Frame `k` (1-based) as FUN_3c64_0496 draws it: the first chunk with
    /// chunk `k` over it.
    pub fn frame(&self, k: usize) -> Vec<u8> {
        let mut pixels = vec![0; usize::from(self.width()) * usize::from(self.height())];
        self.chunks[0].draw(&mut pixels);
        if k > 1
            && let Some(chunk) = self.chunks.get(k - 1)
        {
            chunk.draw(&mut pixels);
        }
        pixels
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn chunk(width: u16, height: u16, pixels: &[u8]) -> Vec<u8> {
        let mut data = ((pixels.len()) as u16).to_le_bytes().to_vec();
        data.extend_from_slice(MAGIC);
        data.extend_from_slice(&width.to_le_bytes());
        data.extend_from_slice(&height.to_le_bytes());
        data.extend_from_slice(pixels);
        data
    }

    #[test]
    fn draws_runs_skips_and_literals() {
        // 4x2: a run of 3 fives, a literal 7, a long run of 4 twos.
        let mut file = chunk(4, 2, &[0xc3, 5, 7, 0xc0, 4, 0, 2]);
        // Frame 2: skip 1, a 9, skip 6 (long form).
        file.extend(chunk(4, 2, &[0x81, 9, 0x80, 6, 0]));
        let ani = Ani::parse(&file).unwrap();
        assert_eq!(ani.frame(1), [5, 5, 5, 7, 2, 2, 2, 2]);
        assert_eq!(ani.frame(2), [5, 9, 5, 7, 2, 2, 2, 2]);
    }
}
