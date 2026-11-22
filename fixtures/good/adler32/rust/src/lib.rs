pub fn adler32(data: &[u8]) -> u32 {
    let mut a: u32 = 1;
    let mut b: u32 = 0;
    for &x in data {
        a = (a + x as u32) % 65521;
        b = (b + a) % 65521;
    }
    (b << 16) | a
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_and_single() {
        assert_eq!(adler32(b""), 1);
        assert_eq!(adler32(b"a"), 6422626);
    }

    #[test]
    fn wikipedia() {
        assert_eq!(adler32(b"Wikipedia"), 300286872);
    }

    #[test]
    fn long_inputs() {
        assert_eq!(adler32(&[0u8; 32]), 2097153);
        let data: Vec<u8> = (0..64).map(|x| x as u8).collect();
        assert_eq!(adler32(&data), 2866808801);
    }

    #[test]
    fn hello() {
        assert_eq!(adler32(b"hello"), 103547413);
    }
}
