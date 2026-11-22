pub fn parse_sat(s: &[u8]) -> Option<i32> {
    let mut i = 0;
    let mut neg = false;
    if let Some(&c) = s.first() {
        if c == b'+' {
            i = 1;
        } else if c == b'-' {
            neg = true;
            i = 1;
        }
    }
    if i >= s.len() {
        return None;
    }
    let mut v: i64 = 0;
    for &c in &s[i..] {
        if !c.is_ascii_digit() {
            return None;
        }
        v = (v * 10 + (c - b'0') as i64).min(2_147_483_648);
    }
    if neg {
        if v >= 2_147_483_648 {
            Some(i32::MIN)
        } else {
            Some(-(v as i32))
        }
    } else if v > 2_147_483_647 {
        Some(i32::MAX)
    } else {
        Some(v as i32)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_signed_values() {
        assert_eq!(parse_sat(b"0"), Some(0));
        assert_eq!(parse_sat(b"-0"), Some(0));
        assert_eq!(parse_sat(b"+42"), Some(42));
        assert_eq!(parse_sat(b"-2147483648"), Some(i32::MIN));
    }

    #[test]
    fn saturates_at_bounds() {
        assert_eq!(parse_sat(b"2147483648"), Some(i32::MAX));
        assert_eq!(parse_sat(b"-2147483649"), Some(i32::MIN));
    }

    #[test]
    fn rejects_bad_strings() {
        assert_eq!(parse_sat(b"abc"), None);
        assert_eq!(parse_sat(b"12x"), None);
        assert_eq!(parse_sat(b""), None);
        assert_eq!(parse_sat(b"-"), None);
    }
}
