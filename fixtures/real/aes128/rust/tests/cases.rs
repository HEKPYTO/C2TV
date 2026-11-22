use harness::aes128_ecb;

fn hex(bytes: &[u8; 16]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

#[test]
fn fips197_published_ciphertext() {
    let key = [
        0x00, 0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08, 0x09, 0x0a, 0x0b, 0x0c, 0x0d,
        0x0e, 0x0f,
    ];
    let plain = [
        0x00, 0x11, 0x22, 0x33, 0x44, 0x55, 0x66, 0x77, 0x88, 0x99, 0xaa, 0xbb, 0xcc, 0xdd,
        0xee, 0xff,
    ];
    let cipher = aes128_ecb(&key, &plain, true);
    assert_eq!(hex(&cipher), "69c4e0d86a7b0430d8cdb78070b4c55a");
    assert_eq!(aes128_ecb(&key, &cipher, false), plain);
}

#[test]
fn zero_key_and_block_ciphertext() {
    let zero = [0u8; 16];
    let cipher = aes128_ecb(&zero, &zero, true);
    assert_eq!(hex(&cipher), "66e94bd4ef8a2c3b884cfa59ca342b2e");
    assert_eq!(aes128_ecb(&zero, &cipher, false), zero);
}

#[test]
fn deterministic_additional_blocks() {
    let key = [0u8; 16];
    let plain = [
        0x00, 0x11, 0x22, 0x33, 0x44, 0x55, 0x66, 0x77, 0x88, 0x99, 0xaa, 0xbb, 0xcc, 0xdd,
        0xee, 0xff,
    ];
    let cipher = aes128_ecb(&key, &plain, true);
    assert_eq!(hex(&cipher), "c8a331ff8edd3db175e1545dbefb760b");
    let ff = [0xffu8; 16];
    let cipher = aes128_ecb(&ff, &plain, true);
    assert_eq!(hex(&cipher), "0a90e5b74d2807a651f69ac0896a09f6");
    let cipher = aes128_ecb(&ff, &cipher, false);
    assert_eq!(cipher, plain);
}
