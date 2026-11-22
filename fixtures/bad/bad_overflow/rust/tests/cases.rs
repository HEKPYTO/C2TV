use harness::add_sat;

#[test]
fn non_overflowing_pairs() {
    assert_eq!(add_sat(0, 0), 0);
    assert_eq!(add_sat(1, 2), 3);
    assert_eq!(add_sat(-1, 1), 0);
    assert_eq!(add_sat(-5, -7), -12);
    assert_eq!(add_sat(1_000_000_000, 1_000_000_000), 2_000_000_000);
}

#[test]
fn overflow_saturates() {
    assert_eq!(add_sat(i32::MAX, 1), i32::MAX);
    assert_eq!(add_sat(i32::MIN, -1), i32::MIN);
}
