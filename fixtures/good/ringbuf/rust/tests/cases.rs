use harness::RingBuf;

#[test]
fn contents_follow_head() {
    let mut r = RingBuf::new(4);
    for v in [0x10, 0x20, 0x30, 0x40] {
        assert!(r.push(v));
    }
    assert!(!r.push(0x50));
    assert_eq!(r.pop(), Some(0x10));
    assert!(r.push(0x50));
    assert_eq!(r.len(), 4);
    assert_eq!(r.contents(), vec![0x20, 0x30, 0x40, 0x50]);
}

#[test]
fn empty_and_full() {
    let mut r = RingBuf::new(2);
    assert_eq!(r.pop(), None);
    assert!(r.contents().is_empty());
    assert!(r.push(1));
    assert!(r.push(2));
    assert!(!r.push(3));
    assert_eq!(r.pop(), Some(1));
    assert!(r.push(3));
    assert_eq!(r.contents(), vec![2, 3]);
}
