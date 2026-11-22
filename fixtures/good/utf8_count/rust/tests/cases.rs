use harness::scalar_count;

#[test]
fn counts_valid_utf8() {
    assert_eq!(scalar_count(b""), Some(0));
    assert_eq!(scalar_count(b"abc"), Some(3));
    assert_eq!(scalar_count(&[0xc3, 0xa9]), Some(1));
    assert_eq!(scalar_count(&[0xe2, 0x82, 0xac]), Some(1));
    assert_eq!(scalar_count(&[0xf0, 0x9f, 0x98, 0x80]), Some(1));
    assert_eq!(scalar_count(&[0x61, 0xc3, 0xa9, 0xe2, 0x82, 0xac, 0xf0, 0x9f, 0x98, 0x80]), Some(4));
}

#[test]
fn rejects_invalid_utf8() {
    assert_eq!(scalar_count(&[0xc0, 0x80]), None);
    assert_eq!(scalar_count(&[0xed, 0xa0, 0x80]), None);
    assert_eq!(scalar_count(&[0xe2, 0x82]), None);
    assert_eq!(scalar_count(&[0x80]), None);
}
