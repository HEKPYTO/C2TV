use harness::mul;

#[test]
fn fractional_products() {
    assert_eq!(mul(0, 0), 0);
    assert_eq!(mul(65536, 65536), 65536);
    assert_eq!(mul(32768, 32768), 16384);
    assert_eq!(mul(65536, -65536), -65536);
    assert_eq!(mul(-32768, 32768), -16384);
}

#[test]
fn extreme_products() {
    assert_eq!(mul(i32::MIN, -1), 32768);
    assert_eq!(mul(0x4000_0000, 0x1_0000), 0x4000_0000);
    assert_eq!(mul(32768, -1), 0);
}
