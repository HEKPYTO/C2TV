#include <stdint.h>
#include <stdio.h>
#include <string.h>

#define CAP 4096

static int read_all(uint8_t *dst, size_t n) {
    size_t got = 0;
    while (got < n) {
        size_t r = fread(dst + got, 1, n - got, stdin);
        if (r == 0) return 0;
        got += r;
    }
    return 1;
}

static uint32_t rd32(const uint8_t *p) {
    return (uint32_t)p[0] | ((uint32_t)p[1] << 8) |
           ((uint32_t)p[2] << 16) | ((uint32_t)p[3] << 24);
}

static uint64_t rd64(const uint8_t *p) {
    return (uint64_t)rd32(p) | ((uint64_t)rd32(p + 4) << 32);
}

static void wr32(uint8_t *p, uint32_t v) {
    p[0] = (uint8_t)v; p[1] = (uint8_t)(v >> 8);
    p[2] = (uint8_t)(v >> 16); p[3] = (uint8_t)(v >> 24);
}

static void wr64(uint8_t *p, uint64_t v) {
    wr32(p, (uint32_t)v); wr32(p + 4, (uint32_t)(v >> 32));
}

static uint8_t gf_mul(uint8_t a, uint8_t b) {
    uint8_t p = 0;
    int k;
    for (k = 0; k < 8; k++) {
        if (b & 1u) p ^= a;
        b = (uint8_t)(b >> 1);
        {
            uint8_t hi = (uint8_t)(a & 0x80u);
            a = (uint8_t)(a << 1);
            if (hi) a ^= 0x1bu;
        }
    }
    return p;
}

static uint8_t rot8(uint8_t v) {
    return (uint8_t)((v << 1) | (v >> 7));
}

static uint8_t sbox_at(uint8_t v) {
    uint8_t inv = 0;
    uint8_t r1, r2, r3, r4;
    if (v != 0) {
        unsigned int i;
        for (i = 1; i < 256u; i++) {
            if (gf_mul(v, (uint8_t)i) == 1u) {
                inv = (uint8_t)i;
                break;
            }
        }
    }
    r1 = rot8(inv);
    r2 = rot8(r1);
    r3 = rot8(r2);
    r4 = rot8(r3);
    return (uint8_t)(inv ^ r1 ^ r2 ^ r3 ^ r4 ^ 0x63u);
}

static void make_tables(uint8_t s[256], uint8_t inv[256]) {
    unsigned int i;
    for (i = 0; i < 256u; i++) {
        s[i] = sbox_at((uint8_t)i);
    }
    for (i = 0; i < 256u; i++) {
        inv[s[i]] = (uint8_t)i;
    }
}

static void expand_key(const uint8_t key[16], uint8_t rk[176], const uint8_t s[256]) {
    unsigned int i;
    int j;
    uint8_t rcon = 1;
    memcpy(rk, key, 16);
    for (i = 16; i < 176u; i += 4) {
        uint8_t t[4];
        t[0] = rk[i - 4]; t[1] = rk[i - 3];
        t[2] = rk[i - 2]; t[3] = rk[i - 1];
        if (i % 16u == 0u) {
            uint8_t old = t[0];
            t[0] = (uint8_t)(s[t[1]] ^ rcon);
            t[1] = s[t[2]];
            t[2] = s[t[3]];
            t[3] = s[old];
            rcon = gf_mul(rcon, 2);
        }
        for (j = 0; j < 4; j++) {
            rk[i + (unsigned int)j] = (uint8_t)(rk[i - 16u + (unsigned int)j] ^ t[j]);
        }
    }
}

static void add_key(uint8_t st[16], const uint8_t rk[176], unsigned int round) {
    unsigned int i;
    for (i = 0; i < 16u; i++) st[i] ^= rk[round * 16u + i];
}

static void sub_bytes(uint8_t st[16], const uint8_t s[256]) {
    unsigned int i;
    for (i = 0; i < 16u; i++) st[i] = s[st[i]];
}

static void shift_rows(uint8_t st[16]) {
    uint8_t t[16];
    unsigned int r, c;
    for (r = 0; r < 4u; r++) {
        for (c = 0; c < 4u; c++) {
            t[r + 4u * c] = st[r + 4u * ((c + r) % 4u)];
        }
    }
    memcpy(st, t, 16);
}

static void inv_shift_rows(uint8_t st[16]) {
    uint8_t t[16];
    unsigned int r, c;
    for (r = 0; r < 4u; r++) {
        for (c = 0; c < 4u; c++) {
            t[r + 4u * c] = st[r + 4u * ((c + 4u - r) % 4u)];
        }
    }
    memcpy(st, t, 16);
}

static void mix_columns(uint8_t st[16]) {
    unsigned int c;
    for (c = 0; c < 4u; c++) {
        unsigned int o = 4u * c;
        uint8_t a0 = st[o], a1 = st[o + 1], a2 = st[o + 2], a3 = st[o + 3];
        st[o]     = (uint8_t)(gf_mul(a0, 2) ^ gf_mul(a1, 3) ^ a2 ^ a3);
        st[o + 1] = (uint8_t)(a0 ^ gf_mul(a1, 2) ^ gf_mul(a2, 3) ^ a3);
        st[o + 2] = (uint8_t)(a0 ^ a1 ^ gf_mul(a2, 2) ^ gf_mul(a3, 3));
        st[o + 3] = (uint8_t)(gf_mul(a0, 3) ^ a1 ^ a2 ^ gf_mul(a3, 2));
    }
}

static void inv_mix_columns(uint8_t st[16]) {
    unsigned int c;
    for (c = 0; c < 4u; c++) {
        unsigned int o = 4u * c;
        uint8_t a0 = st[o], a1 = st[o + 1], a2 = st[o + 2], a3 = st[o + 3];
        st[o]     = (uint8_t)(gf_mul(a0, 14) ^ gf_mul(a1, 11) ^ gf_mul(a2, 13) ^ gf_mul(a3, 9));
        st[o + 1] = (uint8_t)(gf_mul(a0, 9) ^ gf_mul(a1, 14) ^ gf_mul(a2, 11) ^ gf_mul(a3, 13));
        st[o + 2] = (uint8_t)(gf_mul(a0, 13) ^ gf_mul(a1, 9) ^ gf_mul(a2, 14) ^ gf_mul(a3, 11));
        st[o + 3] = (uint8_t)(gf_mul(a0, 11) ^ gf_mul(a1, 13) ^ gf_mul(a2, 9) ^ gf_mul(a3, 14));
    }
}

static void aes_encrypt(uint8_t st[16], const uint8_t rk[176], const uint8_t s[256]) {
    int round;
    add_key(st, rk, 0);
    for (round = 1; round < 10; round++) {
        sub_bytes(st, s);
        shift_rows(st);
        mix_columns(st);
        add_key(st, rk, (unsigned int)round);
    }
    sub_bytes(st, s);
    shift_rows(st);
    add_key(st, rk, 10);
}

static void aes_decrypt(uint8_t st[16], const uint8_t rk[176], const uint8_t inv[256]) {
    int round;
    add_key(st, rk, 10);
    for (round = 9; round >= 1; round--) {
        inv_shift_rows(st);
        sub_bytes(st, inv);
        add_key(st, rk, (unsigned int)round);
        inv_mix_columns(st);
    }
    inv_shift_rows(st);
    sub_bytes(st, inv);
    add_key(st, rk, 0);
}

static void compute(const uint8_t *in, uint32_t n, uint64_t *status,
                    uint64_t *ret, uint8_t *out, uint32_t *outn) {
    uint8_t s[256], inv[256], rk[176], st[16];
    *status = 0;
    *ret = 0;
    *outn = 0;
    (void)ret;
    if (n != 33u || in[0] > 1u) {
        *status = 1;
        return;
    }
    make_tables(s, inv);
    expand_key(in + 1, rk, s);
    memcpy(st, in + 17, 16);
    if (in[0] == 0u) aes_encrypt(st, rk, s);
    else aes_decrypt(st, rk, inv);
    memcpy(out, st, 16);
    *outn = 16;
}

int main(void) {
    uint8_t hdr[4];
    uint8_t in[CAP];
    uint8_t out[CAP];
    uint8_t resp[20 + CAP];
    uint32_t n;
    uint64_t status = 0;
    uint64_t ret = 0;
    uint32_t outn = 0;
    if (!read_all(hdr, 4)) return 2;
    n = rd32(hdr);
    if (n > CAP) return 2;
    if (!read_all(in, n)) return 2;
    compute(in, n, &status, &ret, out, &outn);
    if (outn > CAP) return 2;
    wr64(resp, status);
    wr64(resp + 8, ret);
    wr32(resp + 16, outn);
    if (outn != 0) memcpy(resp + 20, out, outn);
    if (fwrite(resp, 1, 20 + outn, stdout) != (size_t)(20 + outn)) return 2;
    return 0;
}
