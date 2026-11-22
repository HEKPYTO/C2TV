use harness::{canonical, Rec};

#[test]
fn canonicalizes_pattern_payload() {
    let rec = Rec { a: 0x04030201, b: 5, c: 0x0706, d: 9 };
    assert_eq!(canonical(&rec), [1, 2, 3, 4, 5, 0, 6, 7, 9, 0, 0, 0]);
}

#[test]
fn canonicalizes_zero_record() {
    let rec = Rec { a: 0, b: 0, c: 0, d: 0 };
    assert_eq!(canonical(&rec), [0; 12]);
}

#[test]
fn canonicalizes_max_fields() {
    let rec = Rec { a: u32::MAX, b: 255, c: u16::MAX, d: 255 };
    assert_eq!(canonical(&rec), [255, 255, 255, 255, 255, 0, 255, 255, 255, 0, 0, 0]);
}
