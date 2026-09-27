//! `ICON/ICON.ALL`: the 40x24 action icons.
//!
//! A sequence of chunks, each a u16 LE length followed by that many bytes of
//! PCX-style RLE. REUNION.PRG FUN_3a1b_0864 decodes 960 (0x3c0) pixels from the
//! start of a chunk and adds 2 to every value: icons use ICONMAIN.PIC's palette,
//! which the game loads at palette index 2. The chunks decode to 961 bytes; the
//! last one is never drawn.

pub const ICON_WIDTH: usize = 40;
pub const ICON_HEIGHT: usize = 24;
const ICON_PIXELS: usize = ICON_WIDTH * ICON_HEIGHT;

/// Palette indices into ICONMAIN.PIC's palette, one `Vec` per icon.
pub fn decode_icons(data: &[u8]) -> Vec<Vec<u8>> {
    let mut icons = Vec::new();
    let mut rest = data;
    while let [lo, hi, tail @ ..] = rest {
        let len = u16::from_le_bytes([*lo, *hi]) as usize;
        let chunk = &tail[..len.min(tail.len())];
        icons.push(decode_rle(chunk, ICON_PIXELS));
        rest = &tail[chunk.len()..];
    }
    icons
}

fn decode_rle(data: &[u8], need: usize) -> Vec<u8> {
    let mut out = Vec::with_capacity(need);
    let mut bytes = data.iter();
    while out.len() < need {
        let Some(&b) = bytes.next() else { break };
        if b >= 0xC0 {
            let Some(&value) = bytes.next() else { break };
            out.extend(std::iter::repeat_n(value, (b & 0x3F) as usize));
        } else {
            out.push(b);
        }
    }
    out.resize(need, 0);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn splits_chunks_and_expands_runs() {
        // Two icons: one run filling it, one literal then padding.
        let mut data = vec![];
        let first: Vec<u8> = std::iter::repeat_n([0xFF, 5], 16).flatten().collect(); // 16 * 63 >= 960
        data.extend((first.len() as u16).to_le_bytes());
        data.extend(&first);
        data.extend(1u16.to_le_bytes());
        data.push(7);

        let icons = decode_icons(&data);
        assert_eq!(icons.len(), 2);
        assert!(icons[0].iter().all(|&p| p == 5));
        assert_eq!(icons[1][0], 7);
        assert_eq!(icons[1].len(), ICON_PIXELS);
    }
}
