//! Planet surface maps (`MAP/MAP<type>_<variant>.MAP`).
//!
//! From REUNION.PRG FUN_2ef2_03e7: one byte width, one byte height, then one
//! tile number per cell, row by row. Tile `n` is 16x16 pixels at column
//! `n % 20`, row `n / 20` of the terrain's FELSZ sheet; numbers from the
//! terrain's static tile count up come from its animated FANIM sheet instead.

use thiserror::Error;

pub const TILE_SIZE: usize = 16;
pub const TILES_PER_ROW: usize = 20;

#[derive(Debug, Error)]
pub enum MapError {
    #[error("map is {0} bytes, expected {1}")]
    Length(usize, usize),
}

#[derive(Debug, Clone)]
pub struct SurfaceMap {
    pub width: usize,
    pub height: usize,
    tiles: Vec<u8>,
}

impl SurfaceMap {
    pub fn decode(data: &[u8]) -> Result<Self, MapError> {
        let [width, height, ..] = *data else {
            return Err(MapError::Length(data.len(), 2));
        };
        let (width, height) = (width as usize, height as usize);
        let expected = 2 + width * height;
        if data.len() != expected {
            return Err(MapError::Length(data.len(), expected));
        }
        Ok(Self {
            width,
            height,
            tiles: data[2..].to_vec(),
        })
    }

    pub fn tile(&self, x: usize, y: usize) -> Option<u8> {
        (x < self.width && y < self.height).then(|| self.tiles[y * self.width + x])
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_rows() {
        let map = SurfaceMap::decode(&[3, 2, 1, 2, 3, 4, 5, 6]).unwrap();
        assert_eq!((map.width, map.height), (3, 2));
        assert_eq!(map.tile(0, 1), Some(4));
        assert_eq!(map.tile(3, 0), None);
    }

    #[test]
    fn rejects_short_files() {
        assert!(SurfaceMap::decode(&[3, 2, 1]).is_err());
    }
}
