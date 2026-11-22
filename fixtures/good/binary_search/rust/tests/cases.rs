use harness::lower_bound;

#[test]
fn boundary_indices() {
    assert_eq!(lower_bound(&[], 7), Some(0));
    assert_eq!(lower_bound(&[10, 20, 30], 5), Some(0));
    assert_eq!(lower_bound(&[10, 20, 30], 35), Some(3));
    assert_eq!(lower_bound(&[1, 2, 2, 2, 4], 2), Some(1));
}

#[test]
fn unsorted_input_is_none() {
    assert_eq!(lower_bound(&[3, 1, 2], 2), None);
    assert_eq!(lower_bound(&[1, 3, 5, 7], 4), Some(2));
}
