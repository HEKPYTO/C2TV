use harness::reverse;

#[test]
fn edge_bits() {
    assert_eq!(reverse(0), 0);
    assert_eq!(reverse(1), 0x80000000);
    assert_eq!(reverse(0x80000000), 1);
    assert_eq!(reverse(u32::MAX), u32::MAX);
}

#[test]
fn mixed_patterns() {
    assert_eq!(reverse(0x12345678), 0x1e6a2c48);
    assert_eq!(reverse(0x0000ffff), 0xffff0000);
}
