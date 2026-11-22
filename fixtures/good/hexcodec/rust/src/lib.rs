pub fn encode(data: &[u8]) -> Vec<u8> {
    const DIGITS: &[u8; 16] = b"0123456789abcdef";
    let mut out = Vec::with_capacity(data.len() * 2);
    for &b in data {
        out.push(DIGITS[(b >> 4) as usize]);
        out.push(DIGITS[(b & 15) as usize]);
    }
    out
}

pub fn decode(data: &[u8]) -> Option<Vec<u8>> {
    if data.len() % 2 != 0 {
        return None;
    }
    let mut out = Vec::with_capacity(data.len() / 2);
    for pair in data.chunks_exact(2) {
        let hi = hexval(pair[0])?;
        let lo = hexval(pair[1])?;
        out.push((hi * 16 + lo) as u8);
    }
    Some(out)
}

fn hexval(c: u8) -> Option<u8> {
    match c {
        b'0'..=b'9' => Some(c - b'0'),
        b'a'..=b'f' => Some(c - b'a' + 10),
        b'A'..=b'F' => Some(c - b'A' + 10),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn encodes_lowercase() {
        assert_eq!(encode(&[]), Vec::<u8>::new());
        assert_eq!(encode(&[0xff]), b"ff".to_vec());
        assert_eq!(encode(&[0xde, 0xad, 0xbe, 0xef]), b"deadbeef".to_vec());
    }

    #[test]
    fn decodes_all_case_variants() {
        assert_eq!(decode(b"deadbeef"), Some(vec![0xde, 0xad, 0xbe, 0xef]));
        assert_eq!(decode(b"DEADBEEF"), Some(vec![0xde, 0xad, 0xbe, 0xef]));
        assert_eq!(decode(b"dEaDbEeF"), Some(vec![0xde, 0xad, 0xbe, 0xef]));
    }

    #[test]
    fn rejects_bad_hex() {
        assert_eq!(decode(b"abc"), None);
        assert_eq!(decode(b"zz"), None);
        assert_eq!(decode(b"0g"), None);
    }
}
