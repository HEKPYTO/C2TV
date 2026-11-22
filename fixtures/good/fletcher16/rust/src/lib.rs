pub fn fletcher16(data: &[u8]) -> u16 {
    let mut sum1: u32 = 0;
    let mut sum2: u32 = 0;
    for &x in data {
        sum1 = (sum1 + x as u32) % 255;
        sum2 = (sum2 + sum1) % 255;
    }
    ((sum2 << 8) | sum1) as u16
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_and_single() {
        assert_eq!(fletcher16(b""), 0);
        assert_eq!(fletcher16(b"a"), 24929);
    }

    #[test]
    fn ascii_strings() {
        assert_eq!(fletcher16(b"abcde"), 51440);
        assert_eq!(fletcher16(b"abcdef"), 8279);
    }

    #[test]
    fn binary_patterns() {
        assert_eq!(fletcher16(&[0u8; 8]), 0);
        let data: Vec<u8> = (0..16).map(|x| x as u8).collect();
        assert_eq!(fletcher16(&data), 43640);
    }
}
