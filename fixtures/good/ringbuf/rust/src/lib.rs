pub struct RingBuf {
    buf: [u8; 8],
    cap: usize,
    head: usize,
    len: usize,
}

impl RingBuf {
    pub fn new(cap: usize) -> Self {
        Self {
            buf: [0; 8],
            cap,
            head: 0,
            len: 0,
        }
    }

    pub fn push(&mut self, v: u8) -> bool {
        if self.cap == 0 || self.len == self.cap {
            return false;
        }
        let idx = (self.head + self.len) % self.cap;
        self.buf[idx] = v;
        self.len += 1;
        true
    }

    pub fn pop(&mut self) -> Option<u8> {
        if self.cap == 0 || self.len == 0 {
            return None;
        }
        let v = self.buf[self.head];
        self.head = (self.head + 1) % self.cap;
        self.len -= 1;
        Some(v)
    }

    pub fn contents(&self) -> Vec<u8> {
        (0..self.len)
            .map(|i| self.buf[(self.head + i) % self.cap])
            .collect()
    }

    pub fn len(&self) -> usize {
        self.len
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn push_pop_no_wrap() {
        let mut r = RingBuf::new(2);
        assert!(r.push(0x11));
        assert!(r.push(0x22));
        assert_eq!(r.pop(), Some(0x11));
        assert!(r.push(0x33));
        assert_eq!(r.contents(), vec![0x22, 0x33]);
        assert_eq!(r.len(), 2);
    }

    #[test]
    fn wraps_twice() {
        let mut r = RingBuf::new(2);
        assert!(r.push(1));
        assert!(r.push(2));
        assert_eq!(r.pop(), Some(1));
        assert!(r.push(3));
        assert_eq!(r.contents(), vec![2, 3]);
        assert_eq!(r.pop(), Some(2));
        assert!(r.push(4));
        assert_eq!(r.pop(), Some(3));
        assert!(r.push(5));
        assert_eq!(r.contents(), vec![4, 5]);
    }

    #[test]
    fn full_and_empty() {
        let mut r = RingBuf::new(1);
        assert_eq!(r.pop(), None);
        assert!(r.push(7));
        assert!(!r.push(8));
        assert_eq!(r.pop(), Some(7));
        let mut z = RingBuf::new(0);
        assert!(!z.push(1));
        assert_eq!(z.pop(), None);
        assert!(z.contents().is_empty());
    }
}
