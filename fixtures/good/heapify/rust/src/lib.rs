fn sift_down(h: &mut [u32], mut i: usize) {
    loop {
        let l = 2 * i + 1;
        let r = 2 * i + 2;
        let mut s = i;
        if l < h.len() && h[l] < h[s] {
            s = l;
        }
        if r < h.len() && h[r] < h[s] {
            s = r;
        }
        if s == i {
            break;
        }
        h.swap(i, s);
        i = s;
    }
}

pub fn heapify(a: &mut [u32]) {
    if a.is_empty() {
        return;
    }
    let start = a.len() / 2;
    for i in (0..start).rev() {
        sift_down(a, i);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_and_single() {
        let mut a: [u32; 0] = [];
        heapify(&mut a);
        assert_eq!(a, []);
        let mut b = [42u32];
        heapify(&mut b);
        assert_eq!(b, [42]);
    }

    #[test]
    fn three_elements() {
        let mut a = [3u32, 2, 1];
        heapify(&mut a);
        assert_eq!(a, [1, 2, 3]);
        let mut b = [1u32, 2, 3];
        heapify(&mut b);
        assert_eq!(b, [1, 2, 3]);
    }

    #[test]
    fn duplicates_and_mixed() {
        let mut a = [3u32, 1, 3, 1];
        heapify(&mut a);
        assert_eq!(a, [1, 1, 3, 3]);
        let mut b = [9u32, 3, 7, 1, 8, 2, 6, 4];
        heapify(&mut b);
        assert_eq!(b, [1, 3, 2, 4, 8, 7, 6, 9]);
    }
}
