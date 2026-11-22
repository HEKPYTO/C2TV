pub fn encode(data: &[u8]) -> Vec<u8> {
    let mut out = Vec::new();
    let mut i = 0;
    while i < data.len() {
        let v = data[i];
        let mut count = 0u8;
        while i < data.len() && data[i] == v && count < 255 {
            i += 1;
            count += 1;
        }
        out.push(v);
        out.push(count);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_input() {
        assert_eq!(encode(b""), Vec::<u8>::new());
    }

    #[test]
    fn single_run() {
        assert_eq!(encode(b"aaaa"), vec![b'a', 4]);
    }

    #[test]
    fn mixed_runs() {
        assert_eq!(encode(b"aabccc"), vec![b'a', 2, b'b', 1, b'c', 3]);
    }

    #[test]
    fn alternating() {
        assert_eq!(encode(&[1, 2, 1, 2]), vec![1, 1, 2, 1, 1, 1, 2, 1]);
    }
}
