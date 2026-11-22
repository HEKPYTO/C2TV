fn gf_mul(mut a: u8, mut b: u8) -> u8 {
    let mut p = 0u8;
    for _ in 0..8 {
        if b & 1 != 0 {
            p ^= a;
        }
        let hi = a & 0x80 != 0;
        a <<= 1;
        if hi {
            a ^= 0x1b;
        }
        b >>= 1;
    }
    p
}

fn sbox_at(v: u8) -> u8 {
    let inv = if v == 0 {
        0
    } else {
        (1u16..=255)
            .find(|&i| gf_mul(v, i as u8) == 1)
            .map(|i| i as u8)
            .unwrap_or(0)
    };
    let r1 = inv.rotate_left(1);
    let r2 = inv.rotate_left(2);
    let r3 = inv.rotate_left(3);
    let r4 = inv.rotate_left(4);
    inv ^ r1 ^ r2 ^ r3 ^ r4 ^ 0x63
}

fn tables() -> ([u8; 256], [u8; 256]) {
    let mut s = [0u8; 256];
    let mut inv = [0u8; 256];
    for i in 0..256 {
        s[i] = sbox_at(i as u8);
    }
    for i in 0..256 {
        inv[s[i] as usize] = i as u8;
    }
    (s, inv)
}

fn expand_key(key: &[u8; 16], s: &[u8; 256]) -> [u8; 176] {
    let mut rk = [0u8; 176];
    rk[..16].copy_from_slice(key);
    let mut rcon = 1u8;
    for i in (16..176).step_by(4) {
        let mut t = [rk[i - 4], rk[i - 3], rk[i - 2], rk[i - 1]];
        if i % 16 == 0 {
            let old = t[0];
            t[0] = s[t[1] as usize] ^ rcon;
            t[1] = s[t[2] as usize];
            t[2] = s[t[3] as usize];
            t[3] = s[old as usize];
            rcon = gf_mul(rcon, 2);
        }
        for j in 0..4 {
            rk[i + j] = rk[i - 16 + j] ^ t[j];
        }
    }
    rk
}

fn add_key(st: &mut [u8; 16], rk: &[u8; 176], round: usize) {
    for i in 0..16 {
        st[i] ^= rk[round * 16 + i];
    }
}

fn sub_bytes(st: &mut [u8; 16], s: &[u8; 256]) {
    for byte in st.iter_mut() {
        *byte = s[*byte as usize];
    }
}

fn shift_rows(st: &mut [u8; 16]) {
    let old = *st;
    for r in 0..4 {
        for c in 0..4 {
            st[r + 4 * c] = old[r + 4 * ((c + r) % 4)];
        }
    }
}

fn inv_shift_rows(st: &mut [u8; 16]) {
    let old = *st;
    for r in 0..4 {
        for c in 0..4 {
            st[r + 4 * c] = old[r + 4 * ((c + 4 - r) % 4)];
        }
    }
}

fn mix_columns(st: &mut [u8; 16]) {
    for c in 0..4 {
        let o = 4 * c;
        let (a0, a1, a2, a3) = (st[o], st[o + 1], st[o + 2], st[o + 3]);
        st[o] = gf_mul(a0, 2) ^ gf_mul(a1, 3) ^ a2 ^ a3;
        st[o + 1] = a0 ^ gf_mul(a1, 2) ^ gf_mul(a2, 3) ^ a3;
        st[o + 2] = a0 ^ a1 ^ gf_mul(a2, 2) ^ gf_mul(a3, 3);
        st[o + 3] = gf_mul(a0, 3) ^ a1 ^ a2 ^ gf_mul(a3, 2);
    }
}

fn inv_mix_columns(st: &mut [u8; 16]) {
    for c in 0..4 {
        let o = 4 * c;
        let (a0, a1, a2, a3) = (st[o], st[o + 1], st[o + 2], st[o + 3]);
        st[o] = gf_mul(a0, 14) ^ gf_mul(a1, 11) ^ gf_mul(a2, 13) ^ gf_mul(a3, 9);
        st[o + 1] = gf_mul(a0, 9) ^ gf_mul(a1, 14) ^ gf_mul(a2, 11) ^ gf_mul(a3, 13);
        st[o + 2] = gf_mul(a0, 13) ^ gf_mul(a1, 9) ^ gf_mul(a2, 14) ^ gf_mul(a3, 11);
        st[o + 3] = gf_mul(a0, 11) ^ gf_mul(a1, 13) ^ gf_mul(a2, 9) ^ gf_mul(a3, 14);
    }
}

pub fn aes128_ecb(key: &[u8; 16], block: &[u8; 16], encrypt: bool) -> [u8; 16] {
    let (s, inv) = tables();
    let rk = expand_key(key, &s);
    let mut st = *block;
    if encrypt {
        add_key(&mut st, &rk, 0);
        for round in 1..10 {
            sub_bytes(&mut st, &s);
            shift_rows(&mut st);
            mix_columns(&mut st);
            add_key(&mut st, &rk, round);
        }
        sub_bytes(&mut st, &s);
        shift_rows(&mut st);
        add_key(&mut st, &rk, 10);
    } else {
        add_key(&mut st, &rk, 10);
        for round in (1..10).rev() {
            inv_shift_rows(&mut st);
            sub_bytes(&mut st, &inv);
            add_key(&mut st, &rk, round);
            inv_mix_columns(&mut st);
        }
        inv_shift_rows(&mut st);
        sub_bytes(&mut st, &inv);
        add_key(&mut st, &rk, 0);
    }
    st
}

#[cfg(test)]
mod tests {
    use super::*;

    const FIPS_KEY: [u8; 16] = [
        0x00, 0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08, 0x09, 0x0a, 0x0b, 0x0c, 0x0d,
        0x0e, 0x0f,
    ];
    const FIPS_PLAIN: [u8; 16] = [
        0x00, 0x11, 0x22, 0x33, 0x44, 0x55, 0x66, 0x77, 0x88, 0x99, 0xaa, 0xbb, 0xcc, 0xdd,
        0xee, 0xff,
    ];
    const FIPS_CIPHER: [u8; 16] = [
        0x69, 0xc4, 0xe0, 0xd8, 0x6a, 0x7b, 0x04, 0x30, 0xd8, 0xcd, 0xb7, 0x80, 0x70, 0xb4,
        0xc5, 0x5a,
    ];

    fn hex(bytes: &[u8; 16]) -> String {
        bytes.iter().map(|b| format!("{b:02x}")).collect()
    }

    #[test]
    fn fips197_appendix_b() {
        assert_eq!(
            hex(&aes128_ecb(&FIPS_KEY, &FIPS_PLAIN, true)),
            "69c4e0d86a7b0430d8cdb78070b4c55a"
        );
        assert_eq!(aes128_ecb(&FIPS_KEY, &FIPS_CIPHER, false), FIPS_PLAIN);
    }

    #[test]
    fn zero_key_and_block() {
        let zero = [0u8; 16];
        let cipher = aes128_ecb(&zero, &zero, true);
        assert_eq!(hex(&cipher), "66e94bd4ef8a2c3b884cfa59ca342b2e");
        assert_eq!(aes128_ecb(&zero, &cipher, false), zero);
        let cipher = aes128_ecb(&zero, &FIPS_PLAIN, true);
        assert_eq!(hex(&cipher), "c8a331ff8edd3db175e1545dbefb760b");
    }

    #[test]
    fn additional_blocks() {
        let ff = [0xffu8; 16];
        let cipher = aes128_ecb(&ff, &FIPS_PLAIN, true);
        assert_eq!(hex(&cipher), "0a90e5b74d2807a651f69ac0896a09f6");
        assert_eq!(aes128_ecb(&ff, &cipher, false), FIPS_PLAIN);
        let cipher = aes128_ecb(&FIPS_KEY, &ff, true);
        assert_eq!(hex(&cipher), "3c441f32ce07822364d7a2990e50bb13");
    }
}
