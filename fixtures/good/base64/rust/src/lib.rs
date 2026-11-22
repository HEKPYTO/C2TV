const ALPHABET: &[u8; 64] =
    b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";

pub fn encode(data: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity((data.len() + 2) / 3 * 4);
    for chunk in data.chunks(3) {
        let bits = ((chunk[0] as u32) << 16)
            | ((chunk.get(1).copied().unwrap_or(0) as u32) << 8)
            | (chunk.get(2).copied().unwrap_or(0) as u32);
        out.push(ALPHABET[((bits >> 18) & 63) as usize]);
        out.push(ALPHABET[((bits >> 12) & 63) as usize]);
        out.push(if chunk.len() > 1 {
            ALPHABET[((bits >> 6) & 63) as usize]
        } else {
            b'='
        });
        out.push(if chunk.len() > 2 {
            ALPHABET[(bits & 63) as usize]
        } else {
            b'='
        });
    }
    out
}

fn val(c: u8) -> Option<u8> {
    match c {
        b'A'..=b'Z' => Some(c - b'A'),
        b'a'..=b'z' => Some(c - b'a' + 26),
        b'0'..=b'9' => Some(c - b'0' + 52),
        b'+' => Some(62),
        b'/' => Some(63),
        _ => None,
    }
}

pub fn decode(data: &[u8]) -> Option<Vec<u8>> {
    if data.len() % 4 != 0 {
        return None;
    }
    let mut out = Vec::with_capacity(data.len() / 4 * 3);
    for chunk in data.chunks(4) {
        let mut v = [0u8; 4];
        let mut pad = 0usize;
        let mut seen_pad = false;
        for (i, &c) in chunk.iter().enumerate() {
            if seen_pad {
                if c != b'=' {
                    return None;
                }
                pad += 1;
            } else if c == b'=' {
                seen_pad = true;
                pad = 1;
            } else {
                v[i] = val(c)?;
            }
        }
        if pad > 2 {
            return None;
        }
        if pad == 2 && (v[1] & 0x0f) != 0 {
            return None;
        }
        if pad == 1 && (v[2] & 0x03) != 0 {
            return None;
        }
        let bits = ((v[0] as u32) << 18)
            | ((v[1] as u32) << 12)
            | ((v[2] as u32) << 6)
            | (v[3] as u32);
        out.push((bits >> 16) as u8);
        if pad < 2 {
            out.push((bits >> 8) as u8);
        }
        if pad < 1 {
            out.push(bits as u8);
        }
    }
    Some(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn encode_vectors() {
        assert_eq!(encode(b""), b"");
        assert_eq!(encode(b"f"), b"Zg==");
        assert_eq!(encode(b"fo"), b"Zm8=");
        assert_eq!(encode(b"foo"), b"Zm9v");
        assert_eq!(encode(b"foob"), b"Zm9vYg==");
    }

    #[test]
    fn decode_vectors() {
        assert_eq!(decode(b""), Some(Vec::new()));
        assert_eq!(decode(b"Zg=="), Some(b"f".to_vec()));
        assert_eq!(decode(b"Zm8="), Some(b"fo".to_vec()));
        assert_eq!(decode(b"Zm9v"), Some(b"foo".to_vec()));
    }

    #[test]
    fn rejects_invalid() {
        assert_eq!(decode(b"Zg!="), None);
        assert_eq!(decode(b"Z=g="), None);
        assert_eq!(decode(b"Zh=="), None);
        assert_eq!(decode(b"Zg="), None);
        assert_eq!(decode(b"===="), None);
    }
}
