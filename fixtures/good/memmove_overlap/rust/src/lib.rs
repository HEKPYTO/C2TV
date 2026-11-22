pub fn copy_within(buf: &mut [u8], src: usize, dst: usize, len: usize) -> bool {
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
    fn noop_and_bounds() {
        let mut buf = [1u8, 2, 3];
        assert!(copy_within(&mut buf, 1, 1, 0));
        assert_eq!(buf, [1, 2, 3]);
        assert!(!copy_within(&mut buf, 0, 0, 4));
        assert!(!copy_within(&mut buf, usize::MAX, 0, 1));
    }

    #[test]
    fn overlapping_shifts() {
        let mut left = [1u8, 2, 3, 4, 5];
        assert!(copy_within(&mut left, 2, 0, 3));
        assert_eq!(left, [3, 4, 5, 4, 5]);
        let mut right = [1u8, 2, 3, 4, 5];
        assert!(copy_within(&mut right, 0, 2, 3));
        assert_eq!(right, [1, 2, 1, 2, 3]);
    }

    #[test]
    fn empty_buffer_len_zero() {
        let mut buf: [u8; 0] = [];
        assert!(copy_within(&mut buf, 0, 0, 0));
    }
}
