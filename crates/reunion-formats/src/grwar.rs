//! Scrambled `GRWAR/*.PIC` animation scripts.
//!
//! From REUNION.PRG FUN_4265_000c: stored byte-reversed with 0x2F subtracted
//! from every byte, so decoding reverses and adds 0x2F back.

pub fn descramble(data: &[u8]) -> Vec<u8> {
    data.iter().rev().map(|b| b.wrapping_add(0x2F)).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reverses_and_adds() {
        assert_eq!(descramble(&[0xD1, 0xD2, 0xD3]), [0x02, 0x01, 0x00]);
    }
}
