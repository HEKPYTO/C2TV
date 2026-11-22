use harness::merge;

#[test]
fn merge_empty_arrays() {
    assert_eq!(merge(&[], &[]), Some(Vec::new()));
}

#[test]
fn merge_all_of_one_side() {
    assert_eq!(merge(&[], &[2, 5, 9]), Some(vec![2, 5, 9]));
    assert_eq!(merge(&[2, 5, 9], &[]), Some(vec![2, 5, 9]));
}

#[test]
fn merge_rejects_unsorted() {
    assert_eq!(merge(&[1, 3, 2], &[4]), None);
    assert_eq!(merge(&[1], &[5, 4]), None);
}
