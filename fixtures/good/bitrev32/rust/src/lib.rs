pub fn reverse(x: u32) -> u32 {
    x.reverse_bits()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn zero_and_one() {
        assert_eq!(reverse(0), 0);
        assert_eq!(reverse(1), 0x80000000);
        assert_eq!(reverse(0x80000000), 1);
    }

    #[test]
    fn patterns() {
        assert_eq!(reverse(0x12345678), 0x1e6a2c48);
        assert_eq!(reverse(0xffffffff), 0xffffffff);
        assert_eq!(reverse(0x0000ffff), 0xffff0000);
    }

    #[test]
    fn involution() {
        assert_eq!(reverse(reverse(0x0f0f0f0f)), 0x0f0f0f0f);
    }
}
