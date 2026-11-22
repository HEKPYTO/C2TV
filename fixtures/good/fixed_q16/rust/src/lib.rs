pub fn mul(a: i32, b: i32) -> i32 {
    let q = (a as i64) * (b as i64) + 32768;
    let mut r = q / 65536;
    if q < 0 && q % 65536 != 0 {
        r -= 1;
    }
    r as i32
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn positive_products() {
        assert_eq!(mul(0, 0), 0);
        assert_eq!(mul(65536, 65536), 65536);
        assert_eq!(mul(32768, 32768), 16384);
        assert_eq!(mul(0x4000_0000, 0x1_0000), 0x4000_0000);
    }

    #[test]
    fn negative_products() {
        assert_eq!(mul(65536, -65536), -65536);
        assert_eq!(mul(-32768, 32768), -16384);
        assert_eq!(mul(i32::MIN, -1), 32768);
        assert_eq!(mul(32768, -1), 0);
    }
}
