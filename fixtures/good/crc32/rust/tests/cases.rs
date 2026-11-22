use harness::crc32;

#[test]
fn standard_vectors() {
    assert_eq!(crc32(b""), 0);
    assert_eq!(crc32(b"hello"), 0x3610_a686);
    assert_eq!(crc32(b"123456789"), 0xcbf4_3926);
}

#[test]
fn binary_vectors() {
    assert_eq!(crc32(&[0xff]), 0xff00_0000);
    assert_eq!(crc32(&[0, 0, 0, 0, 0, 0, 0, 0]), 0x6522_df69);
}
