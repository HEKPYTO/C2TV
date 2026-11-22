pub fn maxima(data: &[u32], k: usize) -> Vec<u32> {
    if k == 0 || data.len() < k {
        return Vec::new();
    }
    data.windows(k)
        .map(|w| w.iter().copied().max().unwrap())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn computes_increasing_and_decreasing_windows() {
        assert_eq!(maxima(&[1, 2, 3, 4], 2), vec![2, 3, 4]);
        assert_eq!(maxima(&[4, 3, 2, 1], 2), vec![4, 3, 2]);
    }

    #[test]
    fn handles_duplicates_and_short_input() {
        assert_eq!(maxima(&[1, 3, 3, 2, 5], 3), vec![3, 3, 5]);
        assert_eq!(maxima(&[1, 2], 4), Vec::<u32>::new());
    }

    #[test]
    fn handles_single_width_and_zero_width() {
        assert_eq!(maxima(&[5, 1, 9], 1), vec![5, 1, 9]);
        assert_eq!(maxima(&[], 0), Vec::<u32>::new());
    }
}
