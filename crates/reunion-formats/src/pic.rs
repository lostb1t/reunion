//! "SpidyGfx" `.PIC` images.
//!
//! ```text
//! 0     8    magic b"SpidyGfx"
//! 8     2    width  (u16 LE)
//! 10    2    height (u16 LE)
//! 12    ..   PCX-style RLE: byte >= 0xC0 repeats the next byte (b & 0x3F) times,
//!            any other byte is a literal pixel
//! -769  1    0x0C palette marker
//! -768  768  palette, 256 x RGB, 8 bits per channel
//! ```

use thiserror::Error;

const MAGIC: &[u8] = b"SpidyGfx";
const HEADER_LEN: usize = 12;
const PALETTE_LEN: usize = 768;
const PALETTE_MARKER: u8 = 0x0C;

#[derive(Debug, Error)]
pub enum PicError {
    #[error("not a SpidyGfx image")]
    BadMagic,
    #[error("file too short")]
    Truncated,
    #[error("palette marker missing")]
    NoPalette,
}

#[derive(Debug, Clone)]
pub struct Pic {
    pub width: u16,
    pub height: u16,
    /// One palette index per pixel, row-major.
    pub pixels: Vec<u8>,
    pub palette: [[u8; 3]; 256],
}

impl Pic {
    /// Expands the indexed pixels to RGBA8. Palette index 0 stays opaque, like on VGA.
    pub fn to_rgba(&self) -> Vec<u8> {
        self.pixels
            .iter()
            .flat_map(|&i| {
                let [r, g, b] = self.palette[i as usize];
                [r, g, b, 255]
            })
            .collect()
    }
}

pub fn decode(data: &[u8]) -> Result<Pic, PicError> {
    if !data.starts_with(MAGIC) {
        return Err(PicError::BadMagic);
    }
    if data.len() < HEADER_LEN + PALETTE_LEN + 1 {
        return Err(PicError::Truncated);
    }
    let width = u16::from_le_bytes([data[8], data[9]]);
    let height = u16::from_le_bytes([data[10], data[11]]);
    let palette_start = data.len() - PALETTE_LEN;
    if data[palette_start - 1] != PALETTE_MARKER {
        return Err(PicError::NoPalette);
    }

    let need = width as usize * height as usize;
    let mut pixels = Vec::with_capacity(need);
    let mut rle = data[HEADER_LEN..palette_start - 1].iter();
    while pixels.len() < need {
        let Some(&b) = rle.next() else { break };
        if b >= 0xC0 {
            let Some(&value) = rle.next() else { break };
            pixels.extend(std::iter::repeat_n(value, (b & 0x3F) as usize));
        } else {
            pixels.push(b);
        }
    }
    // A few files overrun by a run; clamp (or pad) to the declared size.
    pixels.resize(need, 0);

    let mut palette = [[0u8; 3]; 256];
    for (entry, rgb) in palette.iter_mut().zip(data[palette_start..].chunks_exact(3)) {
        entry.copy_from_slice(rgb);
    }

    Ok(Pic { width, height, pixels, palette })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn file(width: u16, height: u16, rle: &[u8]) -> Vec<u8> {
        let mut data = MAGIC.to_vec();
        data.extend(width.to_le_bytes());
        data.extend(height.to_le_bytes());
        data.extend(rle);
        data.push(PALETTE_MARKER);
        data.extend((0..=255u8).flat_map(|i| [i, i, i]));
        data
    }

    #[test]
    fn decodes_literals_and_runs() {
        let pic = decode(&file(4, 2, &[1, 0xC3, 7, 0xC4, 9])).unwrap();
        assert_eq!(pic.pixels, [1, 7, 7, 7, 9, 9, 9, 9]);
        assert_eq!(pic.palette[7], [7, 7, 7]);
    }

    #[test]
    fn clamps_overrun() {
        let pic = decode(&file(3, 1, &[0xC5, 2])).unwrap();
        assert_eq!(pic.pixels, [2, 2, 2]);
    }

    #[test]
    fn rejects_other_files() {
        assert!(matches!(decode(b"not an image at all"), Err(PicError::BadMagic)));
    }
}
