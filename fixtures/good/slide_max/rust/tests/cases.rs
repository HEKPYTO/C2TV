use harness::maxima;

#[test]
fn computes_window_maxima() {
    assert_eq!(maxima(&[5, 1, 9], 1), vec![5, 1, 9]);
    assert_eq!(maxima(&[1, 2, 3, 4], 2), vec![2, 3, 4]);
    assert_eq!(maxima(&[4, 3, 2, 1], 2), vec![4, 3, 2]);
}

#[test]
fn handles_duplicates_and_short_input() {
    assert_eq!(maxima(&[1, 3, 3, 2, 5], 3), vec![3, 3, 5]);
    assert_eq!(maxima(&[1, 2], 4), Vec::<u32>::new());
    assert_eq!(maxima(&[], 1), Vec::<u32>::new());
}
