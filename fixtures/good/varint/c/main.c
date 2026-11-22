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

static int leb_decode(const uint8_t *p, uint32_t n, uint64_t *v, uint32_t *used) {
    uint64_t acc = 0;
    uint32_t i;
    if (n == 0) return 0;
    for (i = 0; i < 10 && i < n; i++) {
        uint8_t b = p[i];
        if (i == 9 && b > 1) return 0;
        acc |= (uint64_t)(b & 0x7f) << (7u * i);
        if ((b & 0x80) == 0) {
            *v = acc;
            *used = i + 1;
            return 1;
        }
    }
    return 0;
}

static uint32_t leb_encode(uint64_t v, uint8_t *out) {
    uint32_t n = 0;
    while (v >= 0x80) {
        out[n++] = (uint8_t)((v & 0x7f) | 0x80);
        v >>= 7;
    }
    out[n++] = (uint8_t)v;
    return n;
}

static void compute(const uint8_t *in, uint32_t n, uint64_t *status,
                    uint64_t *ret, uint8_t *out, uint32_t *outn) {
    *status = 0;
    *ret = 0;
    *outn = 0;
    if (n == 0) {
        *status = 1;
        return;
    }
    if (in[0] == 0) {
        uint64_t v = 0;
        uint32_t used = 0;
        if (n < 2) {
            *status = 1;
            return;
        }
        if (!leb_decode(in + 1, n - 1, &v, &used) || used != n - 1) {
            *status = 1;
            return;
        }
        *ret = v;
        *outn = leb_encode(v, out);
        return;
    }
    if (in[0] == 1) {
        uint64_t v;
        if (n != 9) {
            *status = 1;
            return;
        }
        v = rd64(in + 1);
        *ret = v;
        *outn = leb_encode(v, out);
        return;
    }
    *status = 1;
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
