use harness::popcount;

#[test]
fn zeros_and_full_bytes() {
    assert_eq!(popcount(b""), (0, 0));
    assert_eq!(popcount(&[0x00]), (0, 0));
    assert_eq!(popcount(&[0xff]), (8, 0));
    assert_eq!(popcount(&[0x0f, 0x0f]), (8, 0));
}

#[test]
fn odd_parity() {
    assert_eq!(popcount(b"A"), (2, 0));
    assert_eq!(popcount(&[0xff, 0x00, 0x0f, 0x01]), (13, 1));
}
