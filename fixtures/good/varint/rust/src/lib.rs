pub fn encode(mut v: u64) -> Vec<u8> {
    let mut out = Vec::with_capacity(10);
    while v >= 0x80 {
        out.push(((v & 0x7f) as u8) | 0x80);
        v >>= 7;
    }
    out.push(v as u8);
    out
}

pub fn decode(data: &[u8]) -> Option<(u64, usize)> {
    if data.is_empty() {
        return None;
    }
    let mut acc = 0u64;
    for (i, &b) in data.iter().take(10).enumerate() {
        if i == 9 && b > 1 {
            return None;
        }
        acc |= ((b & 0x7f) as u64) << (7 * i);
        if b & 0x80 == 0 {
            return Some((acc, i + 1));
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn small_values() {
        assert_eq!(encode(0), vec![0]);
        assert_eq!(encode(127), vec![0x7f]);
        assert_eq!(encode(128), vec![0x80, 0x01]);
        assert_eq!(decode(&[0]), Some((0, 1)));
        assert_eq!(decode(&[0x7f]), Some((127, 1)));
        assert_eq!(decode(&[0x80, 0x01]), Some((128, 2)));
    }

    #[test]
    fn max_value() {
        let bytes = encode(u64::MAX);
        let mut expected = vec![0xff; 9];
        expected.push(0x01);
        assert_eq!(bytes, expected);
        assert_eq!(decode(&bytes), Some((u64::MAX, 10)));
    }

    #[test]
    fn rejects_invalid() {
        assert_eq!(decode(&[]), None);
        assert_eq!(decode(&[0x80; 10]), None);
        assert_eq!(decode(&[0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0x02]), None);
    }
}
