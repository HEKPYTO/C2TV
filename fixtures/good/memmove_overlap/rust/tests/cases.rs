use harness::copy_within;

#[test]
fn disjoint_copy() {
    let mut buf = [1u8, 2, 3, 4, 5, 6];
    assert!(copy_within(&mut buf, 0, 3, 3));
    assert_eq!(buf, [1, 2, 3, 1, 2, 3]);
}

#[test]
fn overlapping_copy() {
    let mut buf = [1u8, 2, 3, 4, 5];
    assert!(copy_within(&mut buf, 0, 2, 3));
    assert_eq!(buf, [1, 2, 1, 2, 3]);
    assert!(!copy_within(&mut buf, 4, 0, 2));
}
