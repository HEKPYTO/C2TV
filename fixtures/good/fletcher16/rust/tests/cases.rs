use harness::fletcher16;

#[test]
fn small_inputs() {
    assert_eq!(fletcher16(b""), 0);
    assert_eq!(fletcher16(b"a"), 24929);
    assert_eq!(fletcher16(b"abcde"), 51440);
}

#[test]
fn other_inputs() {
    assert_eq!(fletcher16(b"abcdef"), 8279);
    assert_eq!(fletcher16(&[0u8; 8]), 0);
}
