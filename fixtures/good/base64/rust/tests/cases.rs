use harness::{decode, encode};

#[test]
fn codec_roundtrip() {
    for data in [b"".as_slice(), b"f", b"fo", b"foo", b"foob", &[0, 255, 128]] {
        let text = encode(data);
        assert_eq!(text.len() % 4, 0);
        assert_eq!(decode(&text), Some(data.to_vec()));
    }
}

#[test]
fn strict_decode() {
    assert_eq!(decode(b"Zh=="), None);
    assert_eq!(decode(b"Zm8!"), None);
    assert_eq!(decode(b"Z=g="), None);
    assert_eq!(decode(b"Zg="), None);
}
