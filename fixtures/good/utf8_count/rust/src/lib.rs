pub fn scalar_count(data: &[u8]) -> Option<usize> {
    let mut i = 0;
    let mut count = 0;
    while i < data.len() {
        let b = data[i];
        if b < 0x80 {
            i += 1;
            count += 1;
            continue;
        }
        let (need, lo, hi) = match b {
            0xc2..=0xdf => (1, 0x80, 0xbf),
            0xe0 => (2, 0xa0, 0xbf),
            0xe1..=0xec => (2, 0x80, 0xbf),
            0xed => (2, 0x80, 0x9f),
            0xee..=0xef => (2, 0x80, 0xbf),
            0xf0 => (3, 0x90, 0xbf),
            0xf1..=0xf3 => (3, 0x80, 0xbf),
            0xf4 => (3, 0x80, 0x8f),
            _ => return None,
        };
        if i + need >= data.len() {
            return None;
        }
        let c = data[i + 1];
        if c < lo || c > hi {
            return None;
        }
        for k in 2..=need {
            if data[i + k] & 0xc0 != 0x80 {
                return None;
            }
        }
        i += need + 1;
        count += 1;
    }
    Some(count)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn valid_counts() {
        assert_eq!(scalar_count(b""), Some(0));
        assert_eq!(scalar_count(b"abc"), Some(3));
        assert_eq!(scalar_count("é".as_bytes()), Some(1));
        assert_eq!(scalar_count("€".as_bytes()), Some(1));
        assert_eq!(scalar_count("😀".as_bytes()), Some(1));
        assert_eq!(scalar_count("aé€😀".as_bytes()), Some(4));
    }

    #[test]
    fn invalid_sequences() {
        assert_eq!(scalar_count(&[0xc0, 0x80]), None);
        assert_eq!(scalar_count(&[0xed, 0xa0, 0x80]), None);
        assert_eq!(scalar_count(&[0xe2, 0x82]), None);
        assert_eq!(scalar_count(&[0x80]), None);
        assert_eq!(scalar_count(&[0xf4, 0x90, 0x80, 0x80]), None);
    }
}
