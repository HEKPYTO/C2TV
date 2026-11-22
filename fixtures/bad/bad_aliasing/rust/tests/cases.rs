use harness::overlap_copy;

#[test]
fn copy_to_higher_index_overlapping() {
    let mut buf = [1u8, 2, 3, 4, 5];
    assert!(overlap_copy(&mut buf, 0, 2, 3));
    assert_eq!(buf, [1, 2, 1, 2, 3]);
}

#[test]
fn copy_to_lower_index_disjoint() {
    let mut buf = [1u8, 2, 3, 4, 5, 6, 7, 8];
    assert!(overlap_copy(&mut buf, 5, 0, 2));
    assert_eq!(buf, [6, 7, 3, 4, 5, 6, 7, 8]);
    assert!(!overlap_copy(&mut buf, 0, 0, 9));
}
