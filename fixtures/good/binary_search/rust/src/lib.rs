pub fn lower_bound(a: &[u32], key: u32) -> Option<usize> {
    if a.windows(2).any(|w| w[0] > w[1]) {
        return None;
    }
    let mut lo = 0usize;
    let mut hi = a.len();
    while lo < hi {
        let mid = lo + (hi - lo) / 2;
        if a[mid] < key {
            lo = mid + 1;
        } else {
            hi = mid;
        }
    }
    Some(lo)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_and_outside() {
        assert_eq!(lower_bound(&[], 7), Some(0));
        assert_eq!(lower_bound(&[10, 20, 30], 5), Some(0));
        assert_eq!(lower_bound(&[10, 20, 30], 35), Some(3));
    }

    #[test]
    fn present_absent_and_duplicates() {
        assert_eq!(lower_bound(&[1, 3, 5, 7], 5), Some(2));
        assert_eq!(lower_bound(&[1, 3, 5, 7], 4), Some(2));
        assert_eq!(lower_bound(&[1, 2, 2, 2, 4], 2), Some(1));
    }

    #[test]
    fn unsorted_is_rejected() {
        assert_eq!(lower_bound(&[3, 1, 2], 2), None);
    }
}
