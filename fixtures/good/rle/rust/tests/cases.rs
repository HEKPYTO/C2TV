use harness::encode;

#[test]
fn simple_runs() {
    assert_eq!(encode(b""), Vec::<u8>::new());
    assert_eq!(encode(b"aaaa"), vec![b'a', 4]);
    assert_eq!(encode(b"aabccc"), vec![b'a', 2, b'b', 1, b'c', 3]);
}

#[test]
fn alternating_bytes() {
    let out = encode(&[1, 2, 1, 2, 1, 2, 1, 2]);
    assert_eq!(out.len(), 16);
    assert_eq!(out, vec![1, 1, 2, 1, 1, 1, 2, 1, 1, 1, 2, 1, 1, 1, 2, 1]);
}
