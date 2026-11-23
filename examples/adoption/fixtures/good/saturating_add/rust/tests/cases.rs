use harness::add_sat;

#[test]
fn adds_within_range() {
    assert_eq!(add_sat(1, 2), 3);
    assert_eq!(add_sat(-5, 7), 2);
}

#[test]
fn saturates_on_overflow_and_underflow() {
    assert_eq!(add_sat(i32::MAX, 1), i32::MAX);
    assert_eq!(add_sat(i32::MIN, -1), i32::MIN);
}
