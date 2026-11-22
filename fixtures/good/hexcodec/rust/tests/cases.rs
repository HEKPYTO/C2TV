use harness::{decode, encode};

#[test]
fn encodes_empty_and_binary() {
    assert_eq!(encode(&[]), Vec::<u8>::new());
    assert_eq!(encode(&[0xff]), b"ff".to_vec());
    assert_eq!(encode(&[0x00, 0x0f, 0xf0, 0xff]), b"000ff0ff".to_vec());
}

#[test]
fn decodes_valid_hex() {
    assert_eq!(decode(b""), Some(Vec::new()));
    assert_eq!(decode(b"ff"), Some(vec![0xff]));
    assert_eq!(decode(b"00FFaa55"), Some(vec![0x00, 0xff, 0xaa, 0x55]));
}

#[test]
fn rejects_invalid_hex() {
    assert_eq!(decode(b"0"), None);
    assert_eq!(decode(b"gg"), None);
    assert_eq!(decode(b"0z"), None);
}
