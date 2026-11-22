pub fn overlap_copy(buf: &mut [u8], src: usize, dst: usize, len: usize) -> bool {
    if src.checked_add(len).map_or(true, |end| end > buf.len()) {
        return false;
    }
    if dst.checked_add(len).map_or(true, |end| end > buf.len()) {
        return false;
    }
    buf.copy_within(src..src + len, dst);
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn disjoint_copies() {
        let mut buf = [1u8, 2, 3, 4, 5, 6];
        assert!(overlap_copy(&mut buf, 0, 3, 3));
        assert_eq!(buf, [1, 2, 3, 1, 2, 3]);
        let mut other = [1u8, 2, 3, 4, 5, 6, 7, 8];
        assert!(overlap_copy(&mut other, 5, 0, 2));
        assert_eq!(other, [6, 7, 3, 4, 5, 6, 7, 8]);
    }

    #[test]
    fn overlapping_copy_preserves_source() {
        let mut buf = [1u8, 2, 3, 4, 5];
        assert!(overlap_copy(&mut buf, 0, 2, 3));
        assert_eq!(buf, [1, 2, 1, 2, 3]);
    }

    #[test]
    fn bounds_checks() {
        let mut buf = [1u8, 2, 3];
        assert!(overlap_copy(&mut buf, 0, 0, 0));
        assert!(!overlap_copy(&mut buf, 0, 0, 4));
        assert!(!overlap_copy(&mut buf, usize::MAX, 0, 1));
    }
}
