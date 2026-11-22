pub fn add_sat(a: i32, b: i32) -> i32 {
    a.saturating_add(b)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ordinary_addition() {
        assert_eq!(add_sat(0, 0), 0);
        assert_eq!(add_sat(1, 2), 3);
        assert_eq!(add_sat(-1, 1), 0);
        assert_eq!(add_sat(-5, -7), -12);
        assert_eq!(add_sat(1_000_000_000, 1_000_000_000), 2_000_000_000);
    }

    #[test]
    fn saturating_addition() {
        assert_eq!(add_sat(i32::MAX, 1), i32::MAX);
        assert_eq!(add_sat(i32::MIN, -1), i32::MIN);
        assert_eq!(add_sat(i32::MAX, 0), i32::MAX);
        assert_eq!(add_sat(i32::MIN, 0), i32::MIN);
    }
}
