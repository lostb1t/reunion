//! Tables read from the player's own `GRWAR/REUNION.PRG`.
//!
//! Addresses are `segment:offset` as Ghidra shows them, with the program loaded
//! at segment 0x1000; the data segment (DS) is 0x4a81. Only the English
//! version this was reverse engineered from is supported.

use thiserror::Error;

const EXPECTED_LEN: usize = 288_992;
const LOAD_SEGMENT: u16 = 0x1000;
const DS: u16 = 0x4a81;

/// Master label table: 18-byte Pascal strings, indexed by action id.
const LABELS: u16 = 0x488a;
const LABEL_STRIDE: usize = 0x12;
/// Action id -> icon number in ICON.ALL.
const ICON_FOR_ACTION: u16 = 0x4e06;
/// Icon bar sets: 25-byte records, a count then up to 12 action ids.
const ICON_SETS: u16 = 0x5107;
const ICON_SET_STRIDE: usize = 0x19;
/// Screen records, 25 bytes from DS:0x5106 + 25 * screen: the action that
/// opens the screen (signed, -1 for none), then its icon bar set.
const SCREEN_TRIGGERS: u16 = 0x5106;
pub const SCREENS: u8 = 38;
/// Planet type -> terrain (FELSZ/FANIM/RADAR/EPUL number), words.
const TERRAIN_FOR_TYPE: u16 = 0x1a38;
/// Per terrain, words: static tiles in FELSZ, FELSZ height, FANIM height.
const STATIC_TILES: u16 = 0x1810;
const FELSZ_ROWS: u16 = 0x17e4;
const FANIM_ROWS: u16 = 0x17fa;
pub const TERRAINS: u16 = 11;
/// Characters in CHARSET1.PIC glyph order.
const CHARSET_ORDER: u16 = 0x5bee;

/// Fixed main-room hotspots from FUN_3abd_08e0: x, y, width, height and the
/// offset of the name in code segment 0x3abd. y is in screen coordinates.
const MAIN_ROOM: [(u16, u16, u16, u16, u16); 8] = [
    (0x2a, 0x47, 0x26, 0x3c, 0x88a),
    (0x00, 0x47, 0x2a, 0x4f, 0x89a),
    (0xa3, 0x31, 0x1e, 0x36, 0x8a2),
    (0xff, 0x31, 0x41, 0x58, 0x8ad),
    (0x00, 0x31, 0x50, 0x16, 0x8b6),
    (0x51, 0x31, 0x30, 0x31, 0x8be),
    (0xc2, 0x31, 0x35, 0x52, 0x8ca),
    (0x49, 0x9d, 0xbe, 0x2b, 0x8d4),
];
const MAIN_ROOM_SEGMENT: u16 = 0x3abd;

#[derive(Debug, Error)]
pub enum ExeError {
    #[error("not the supported REUNION.PRG (expected the English DOS version)")]
    Unsupported,
    #[error("table outside the file")]
    OutOfRange,
}

/// A clickable area of the main room.
#[derive(Debug, Clone, PartialEq)]
pub struct RoomHotspot {
    pub x: u16,
    pub y: u16,
    pub width: u16,
    pub height: u16,
    pub label: String,
}

pub struct GameExe {
    data: Vec<u8>,
    header_len: usize,
}

impl GameExe {
    pub fn parse(data: Vec<u8>) -> Result<Self, ExeError> {
        if data.len() != EXPECTED_LEN || !data.starts_with(b"MZ") {
            return Err(ExeError::Unsupported);
        }
        let header_len = u16::from_le_bytes([data[8], data[9]]) as usize * 16;
        Ok(Self { data, header_len })
    }

    fn at(&self, segment: u16, offset: u16) -> Result<usize, ExeError> {
        let pos = self.header_len + (segment - LOAD_SEGMENT) as usize * 16 + offset as usize;
        if pos < self.data.len() {
            Ok(pos)
        } else {
            Err(ExeError::OutOfRange)
        }
    }

    fn pascal_string(&self, segment: u16, offset: u16) -> Result<String, ExeError> {
        let pos = self.at(segment, offset)?;
        let len = self.data[pos] as usize;
        let bytes = self
            .data
            .get(pos + 1..pos + 1 + len)
            .ok_or(ExeError::OutOfRange)?;
        Ok(bytes.iter().map(|&b| b as char).collect())
    }

    /// Initial bytes of the data segment at `offset`, if the executable
    /// initializes them (uninitialized memory isn't in the file).
    pub fn ds_bytes(&self, offset: u16, len: usize) -> Option<&[u8]> {
        let start = self.at(DS, offset).ok()?;
        self.data.get(start..start + len)
    }

    /// Hover label of an action, without the padding.
    pub fn label(&self, action: u8) -> Result<String, ExeError> {
        let offset = LABELS + (action as usize * LABEL_STRIDE) as u16;
        Ok(self.pascal_string(DS, offset)?.trim_end().to_string())
    }

    pub fn icon_for(&self, action: u8) -> Result<u8, ExeError> {
        Ok(self.data[self.at(DS, ICON_FOR_ACTION + action as u16)?])
    }

    /// Action ids of an icon bar set, in display order.
    pub fn icon_set(&self, set: u8) -> Result<Vec<u8>, ExeError> {
        let pos = self.at(DS, ICON_SETS + (set as usize * ICON_SET_STRIDE) as u16)?;
        let count = (self.data[pos] as usize).min(12);
        Ok(self.data[pos + 1..pos + 1 + count].to_vec())
    }

    fn ds_word(&self, offset: u16) -> Result<u16, ExeError> {
        let pos = self.at(DS, offset)?;
        let bytes = self.data.get(pos..pos + 2).ok_or(ExeError::OutOfRange)?;
        Ok(u16::from_le_bytes([bytes[0], bytes[1]]))
    }

    /// The action that opens `screen` (1-based), if any.
    pub fn screen_trigger(&self, screen: u8) -> Result<Option<u8>, ExeError> {
        let pos = self.at(DS, SCREEN_TRIGGERS + screen as u16 * ICON_SET_STRIDE as u16)?;
        let trigger = self.data[pos] as i8;
        Ok((trigger > 0).then_some(trigger as u8))
    }

    /// Terrain number of a planet type (planet record byte 0x15).
    pub fn terrain_for_type(&self, planet_type: u8) -> Result<u16, ExeError> {
        self.ds_word(TERRAIN_FOR_TYPE + planet_type as u16 * 2)
    }

    /// How many tiles of a terrain are static (in FELSZ) rather than animated.
    pub fn static_tiles(&self, terrain: u16) -> Result<u16, ExeError> {
        self.ds_word(STATIC_TILES + terrain * 2)
    }

    /// Heights of a terrain's FELSZ and FANIM sheets (0 for no FANIM).
    pub fn sheet_rows(&self, terrain: u16) -> Result<(u16, u16), ExeError> {
        Ok((
            self.ds_word(FELSZ_ROWS + terrain * 2)?,
            self.ds_word(FANIM_ROWS + terrain * 2)?,
        ))
    }

    /// Characters in the order of the glyphs in CHARSET1.PIC.
    pub fn charset_order(&self) -> Result<Vec<u8>, ExeError> {
        Ok(self.pascal_string(DS, CHARSET_ORDER)?.bytes().collect())
    }

    pub fn main_room(&self) -> Result<Vec<RoomHotspot>, ExeError> {
        MAIN_ROOM
            .iter()
            .map(|&(x, y, width, height, name)| {
                Ok(RoomHotspot {
                    x,
                    y,
                    width,
                    height,
                    label: self.pascal_string(MAIN_ROOM_SEGMENT, name)?,
                })
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_other_files() {
        assert!(matches!(
            GameExe::parse(b"MZ".to_vec()),
            Err(ExeError::Unsupported)
        ));
    }
}
