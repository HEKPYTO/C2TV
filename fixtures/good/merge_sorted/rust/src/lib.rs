pub fn merge(a: &[u32], b: &[u32]) -> Option<Vec<u32>> {
    if a.windows(2).any(|w| w[0] > w[1]) || b.windows(2).any(|w| w[0] > w[1]) {
        return None;
    }
    let mut out = Vec::with_capacity(a.len() + b.len());
    let mut i = 0;
    let mut j = 0;
    while i < a.len() && j < b.len() {
        if a[i] <= b[j] {
            out.push(a[i]);
            i += 1;
        } else {
            out.push(b[j]);
            j += 1;
        }
    }
    out.extend_from_slice(&a[i..]);
    out.extend_from_slice(&b[j..]);
    Some(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn merges_equal_keys() {
        assert_eq!(merge(&[2, 2, 2], &[2, 2, 2]), Some(vec![2, 2, 2, 2, 2, 2]));
    }

    #[test]
    fn merges_interleaved() {
        assert_eq!(merge(&[1, 4, 7], &[2, 2, 5, 8]), Some(vec![1, 2, 2, 4, 5, 7, 8]));
    }

    #[test]
    fn rejects_unsorted_input() {
        assert_eq!(merge(&[5, 3], &[]), None);
        assert_eq!(merge(&[], &[9, 4]), None);
    }
}
