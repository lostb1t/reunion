//! `.SMP` sound effects and speech (SOUND, SOUND2, SOUND3): a Creative Voice
//! (`.VOC`) data block without the file header.
//!
//! ```text
//! 0     1    block type 1 (sound data)
//! 1     3    block length (u24 LE), counting the next two bytes
//! 4     1    Sound Blaster time constant: rate = 1000000 / (256 - tc)
//! 5     1    packing, 0 = 8-bit unsigned PCM
//! 6     ..   samples, mono
//! -1    1    block type 0 (end)
//! ```

use thiserror::Error;

#[derive(Debug, Error)]
pub enum SmpError {
    #[error("not a sound data block")]
    BadBlock,
    #[error("file too short")]
    Truncated,
    #[error("unsupported packing {0}")]
    Packed(u8),
}

#[derive(Debug, Clone)]
pub struct Smp {
    pub sample_rate: u32,
    /// 8-bit unsigned samples, 128 is silence.
    pub samples: Vec<u8>,
}

impl Smp {
    pub fn parse(data: &[u8]) -> Result<Self, SmpError> {
        let header = data.get(..6).ok_or(SmpError::Truncated)?;
        if header[0] != 1 {
            return Err(SmpError::BadBlock);
        }
        let len = u32::from_le_bytes([header[1], header[2], header[3], 0]) as usize;
        let (time_constant, packing) = (header[4], header[5]);
        if packing != 0 {
            return Err(SmpError::Packed(packing));
        }
        let samples = data
            .get(6..4 + len)
            .ok_or(SmpError::Truncated)?
            .to_vec();
        Ok(Self {
            sample_rate: 1_000_000 / (256 - u32::from(time_constant)),
            samples,
        })
    }

    /// The samples as floats in -1..1.
    pub fn to_f32(&self) -> Vec<f32> {
        self.samples
            .iter()
            .map(|&s| (f32::from(s) - 128.0) / 128.0)
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_a_block() {
        // tc 0xad: 1000000 / 83 = 12048 Hz.
        let data = [1, 5, 0, 0, 0xad, 0, 128, 255, 0, 0];
        let smp = Smp::parse(&data).unwrap();
        assert_eq!(smp.sample_rate, 12048);
        assert_eq!(smp.samples, [128, 255, 0]);
    }
}
