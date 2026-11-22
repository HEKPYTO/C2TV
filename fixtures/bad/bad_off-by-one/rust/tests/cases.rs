use harness::echo;

#[test]
fn echo_empty_body() {
    assert!(echo(&[]).is_empty());
}

#[test]
fn echo_full_body() {
    assert_eq!(echo(&[1, 2, 3]), vec![1, 2, 3]);
}
