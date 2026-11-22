use harness::adler32;

#[test]
fn small_inputs() {
    assert_eq!(adler32(b""), 1);
    assert_eq!(adler32(b"a"), 6422626);
    assert_eq!(adler32(b"hello"), 103547413);
}

#[test]
fn longer_inputs() {
    assert_eq!(adler32(b"Wikipedia"), 300286872);
    assert_eq!(adler32(&[0u8; 32]), 2097153);
}
