use harness::{decode, encode};

#[test]
fn roundtrip_public_api() {
    for v in [0u64, 1, 127, 128, 300, u64::MAX] {
        let bytes = encode(v);
        assert_eq!(decode(&bytes), Some((v, bytes.len())));
    }
}

#[test]
fn decode_rejects_bad_input() {
    assert_eq!(decode(&[]), None);
    assert_eq!(decode(&[0x80; 11]), None);
    assert_eq!(decode(&[0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff]), None);
    assert_eq!(decode(&[0x80, 0x01, 0x7f]), Some((128, 2)));
}
