pub fn crc32(data: &[u8]) -> u32 {
    let mut crc = 0xffff_ffffu32;
    for &b in data {
        let mut x = (crc ^ (b as u32)) & 0xff;
        for _ in 0..8 {
            if x & 1 != 0 {
                x = (x >> 1) ^ 0xedb8_8320;
            } else {
                x >>= 1;
            }
        }
        crc = (crc >> 8) ^ x;
    }
    crc ^ 0xffff_ffff
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn known_vectors() {
        assert_eq!(crc32(b""), 0);
        assert_eq!(crc32(b"\x00"), 0xd202_ef8d);
        assert_eq!(crc32(b"\xff"), 0xff00_0000);
        assert_eq!(crc32(b"123456789"), 0xcbf4_3926);
    }

    #[test]
    fn pattern_vectors() {
        assert_eq!(crc32(&[0; 8]), 0x6522_df69);
        assert_eq!(crc32(&[1, 2, 3, 4, 5, 6, 7, 8]), 0x3fca_88c5);
    }
}
