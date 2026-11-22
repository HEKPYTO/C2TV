use harness::hash;

#[test]
fn hashes_empty_and_short_strings() {
    assert_eq!(hash(b""), 0);
    assert_eq!(hash(b"a"), 0x3c2569b2);
    assert_eq!(hash(b"abc"), 0xb3dd93fa);
}

#[test]
fn hashes_binary_patterns() {
    assert_eq!(hash(&[1, 2, 3, 4]), 0x3e349da5);
    assert_eq!(hash(&[1, 2, 3, 4, 5]), 0xa291b9c8);
    assert_eq!(hash(&[1, 2, 3, 4, 5, 6, 7, 8]), 0x0d4b1fbb);
    assert_eq!(hash(&[0; 16]), 0x8134cdf8);
}
