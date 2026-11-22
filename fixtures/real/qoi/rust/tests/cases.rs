use harness::{qoi_decode, qoi_encode, qoi_roundtrip};

fn bytes(hex: &str) -> Vec<u8> {
    (0..hex.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&hex[i..i + 2], 16).unwrap())
        .collect()
}

#[test]
fn decodes_rgb_rgba_run_and_index() {
    let rgb = qoi_decode(&bytes(
        "716f696600000001000000010400fe1020300000000000000001",
    ))
    .unwrap();
    assert_eq!(rgb.width, 1);
    assert_eq!(rgb.height, 1);
    assert_eq!(rgb.pixels, vec![[0x10, 0x20, 0x30, 255]]);

    let rgba = qoi_decode(&bytes(
        "716f696600000001000000010400ff102030400000000000000001",
    ))
    .unwrap();
    assert_eq!(rgba.pixels, vec![[0x10, 0x20, 0x30, 0x40]]);

    let run = qoi_decode(&bytes(
        "716f696600000003000000010400fe112233c20000000000000001",
    ))
    .unwrap();
    assert_eq!(run.pixels, vec![[0x11, 0x22, 0x33, 255]; 3]);

    let index = qoi_decode(&bytes(
        "716f696600000002000000010400ff10203040200000000000000001",
    ))
    .unwrap();
    assert_eq!(
        index.pixels,
        vec![[0x10, 0x20, 0x30, 0x40], [0x10, 0x20, 0x30, 0x40]]
    );
}

#[test]
fn decodes_luma_chunk() {
    let input = bytes("716f696600000002000000010400fe0a141ea5560000000000000001");
    let image = qoi_decode(&input).unwrap();
    assert_eq!(
        image.pixels,
        vec![[10, 20, 30, 255], [12, 25, 33, 255]]
    );
    let encoded = qoi_encode(&image).unwrap();
    assert_eq!(qoi_decode(&encoded).unwrap(), image);
    assert_eq!(encoded, input);
}

#[test]
fn roundtrips_corpus_images() {
    let cases = [
        "716f696600000001000000010400fe1020300000000000000001",
        "716f696600000001000000010400ff102030400000000000000001",
        "716f696600000003000000010400fe112233c20000000000000001",
        "716f696600000002000000010400ff10203040200000000000000001",
        "716f696600000001000000010400ff010203040000000000000001",
        "716f696600000002000000020400fe0a141e7afec86432ffc86432640000000000000001",
        "716f696600000002000000010400fe0a141ea5560000000000000001",
    ];
    for text in cases {
        let input = bytes(text);
        let image = qoi_decode(&input).unwrap();
        let encoded = qoi_encode(&image).unwrap();
        assert_eq!(qoi_decode(&encoded).unwrap().pixels, image.pixels);
        assert_eq!(qoi_roundtrip(&input).unwrap(), encoded);
    }
}

#[test]
fn roundtrip_encodes_canonical_run() {
    let input = bytes("716f696600000003000000010400fe112233c20000000000000001");
    let image = qoi_decode(&input).unwrap();
    let encoded = qoi_encode(&image).unwrap();
    assert_eq!(
        encoded,
        bytes("716f696600000003000000010400fe112233c10000000000000001")
    );
}

#[test]
fn rejects_invalid_stream() {
    let input = bytes("0000000000000001000000010400fe1020300000000000000001");
    assert!(qoi_decode(&input).is_none());
    assert!(qoi_roundtrip(&input).is_none());
    let over_cap = bytes("716f696600000003210000000104000000000000000001");
    assert!(qoi_decode(&over_cap).is_none());
    assert!(qoi_roundtrip(&over_cap).is_none());
}
