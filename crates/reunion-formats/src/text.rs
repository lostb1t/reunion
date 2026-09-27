//! Encrypted text files (`TEXT/*.TXT`, `*.LOC`, ...).
//!
//! Lines are CR LF separated; each line is encrypted on its own. From
//! REUNION.PRG FUN_398f_0864, for the 1-based position `i` in a line:
//! `plain = ((cipher + 0x5780 - 0x4f * i) mod 0xe0) - 0x20`.
//! A `|` in the plain text is a line break on screen.

pub fn decrypt_line(line: &[u8]) -> Vec<u8> {
    line.iter()
        .enumerate()
        .map(|(i, &c)| {
            let i = i as i32 + 1;
            ((c as i32 + 0x5780 - 0x4f * i).rem_euclid(0xe0) - 0x20) as u8
        })
        .collect()
}

/// Decrypts every line of a text file. A trailing empty line is dropped.
pub fn decrypt_lines(data: &[u8]) -> Vec<Vec<u8>> {
    let mut lines: Vec<Vec<u8>> = data
        .split(|&b| b == b'\n')
        .map(|l| decrypt_line(l.strip_suffix(b"\r").unwrap_or(l)))
        .collect();
    if lines.last().is_some_and(|l| l.is_empty()) {
        lines.pop();
    }
    lines
}

#[cfg(test)]
mod tests {
    use super::*;

    fn encrypt_line(plain: &[u8]) -> Vec<u8> {
        plain
            .iter()
            .enumerate()
            .map(|(i, &p)| {
                let i = i as i32 + 1;
                // Inverse of decrypt_line for printable ASCII.
                ((p as i32 + 0x20 - 0x5780 + 0x4f * i).rem_euclid(0xe0)) as u8
            })
            .collect()
    }

    #[test]
    fn round_trips_a_line() {
        let plain = b"Your colony has been destroyed|on Apollo";
        assert_eq!(decrypt_line(&encrypt_line(plain)), plain);
    }

    #[test]
    fn splits_crlf_lines() {
        let mut file = encrypt_line(b"one");
        file.extend(b"\r\n");
        file.extend(encrypt_line(b"two"));
        file.extend(b"\r\n");
        assert_eq!(decrypt_lines(&file), vec![b"one".to_vec(), b"two".to_vec()]);
    }
}
