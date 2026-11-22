pub fn popcount(data: &[u8]) -> (u64, u8) {
    let total: u64 = data.iter().map(|&x| x.count_ones() as u64).sum();
    (total, (total & 1) as u8)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_and_zero() {
        assert_eq!(popcount(b""), (0, 0));
        assert_eq!(popcount(&[0x00]), (0, 0));
    }

    #[test]
    fn full_bytes() {
        assert_eq!(popcount(&[0xff]), (8, 0));
        assert_eq!(popcount(&[0x0f, 0x0f]), (8, 0));
    }

    #[test]
    fn ascii_and_mixed() {
        assert_eq!(popcount(b"A"), (2, 0));
        assert_eq!(popcount(&[0xff, 0x00, 0x0f, 0x01]), (13, 1));
    }
}
